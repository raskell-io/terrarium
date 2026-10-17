//! Instinct: how an agent acts when no language model is deciding for it.
//!
//! These are fast, automatic responses rather than deliberation, built from models in
//! behavioural ecology and social psychology instead of hand-picked thresholds. Every
//! choice is weighed in one currency, the agent's chance of getting through the next
//! few days, so food, injury and harm to others are comparable. The rules, their
//! sources and the calibration constants are documented in docs/INSTINCTS.md.

use rand::RngExt;
use tracing::debug;
use uuid::Uuid;

use crate::action::{Action, ActionContext, Direction, Neighbour};
use crate::agent::{Agent, EpisodeCategory};
use crate::world::LocalFood;

// ---- Body mechanics (mirror Agent::tick_hunger and Agent::eat) ----

const HUNGER_PER_DAY: f64 = 0.1;
const HUNGER_PER_FOOD: f64 = 0.3;
/// Hunger above which starvation starts to cost health
const STARVING_AT: f64 = 0.8;
const STARVATION_DAMAGE_PER_DAY: f64 = 0.1;
/// Typical injury from a blow (engine: 0.15-0.25)
const BLOW: f64 = 0.2;
/// Share of a victim's carried food an attack seizes (engine: min(2 * damage, 1))
const SEIZED_SHARE: f64 = 0.4;

// ---- Calibration (see docs/INSTINCTS.md for where each value comes from) ----

/// How far ahead, in days, instinct weighs survival
const HORIZON_DAYS: f64 = 7.0;
/// Weight on a stranger's welfare when helping, relative to one's own
const STRANGER_WTR: f64 = 0.3;
/// Weight on harm done to a stranger, relative to the same loss to oneself
const HARM_WEIGHT: f64 = 1.0;
/// Extra welfare weight per unit of liking
const FRIENDSHIP_WTR: f64 = 0.5;
/// Extra welfare weight from a favourable balance of past gifts
const RECIPROCITY_WTR: f64 = 0.5;
/// Survival-equivalent cost, per witness, of being seen to harm someone
const REPUTATION_COST: f64 = 0.03;
/// Survival-equivalent cost of breaking the norm against harming others
const NORM_COST: f64 = 0.02;
/// Survival value at which reserves are high enough to breed (about 16 days' reserves)
const BREEDING_RESERVE: f64 = 0.9;
/// Days a harm stays a grievance
const GRUDGE_DAYS: usize = 10;
/// Survival-equivalent value of deterring someone who harmed us, per unit of harm
const DETERRENCE_VALUE: f64 = 0.05;

/// The physical state that survival depends on
#[derive(Debug, Clone, Copy)]
struct Body {
    food: u32,
    hunger: f64,
    health: f64,
}

impl Body {
    fn of(agent: &Agent) -> Self {
        Self { food: agent.physical.food, hunger: agent.physical.hunger, health: agent.physical.health }
    }

    fn of_neighbour(n: &Neighbour) -> Self {
        Self { food: n.food, hunger: n.hunger, health: n.health }
    }

    /// Days until starvation would kill: the hunger reserve before damage starts, plus the
    /// health there is to lose
    fn days_left(&self) -> f64 {
        let reserve = (STARVING_AT - self.hunger + self.food as f64 * HUNGER_PER_FOOD).max(0.0);
        reserve / HUNGER_PER_DAY + self.health.max(0.0) / STARVATION_DAMAGE_PER_DAY
    }

    /// State-dependent value: a survival-like score in [0, 1) that is concave in days left,
    /// so each extra day matters less the better off one already is
    fn value(&self) -> f64 {
        if self.health <= 0.0 {
            return 0.0;
        }
        1.0 - (-self.days_left() / HORIZON_DAYS).exp()
    }

    fn food_gain(&self, amount: u32) -> f64 {
        Body { food: self.food + amount, ..*self }.value() - self.value()
    }

    fn food_loss(&self, amount: u32) -> f64 {
        self.value() - Body { food: self.food.saturating_sub(amount), ..*self }.value()
    }

    fn injury_loss(&self, damage: f64) -> f64 {
        self.value() - Body { health: self.health - damage, ..*self }.value()
    }
}

/// Agreeableness scales how much others' welfare and social norms weigh (0.5 to 1.5)
fn agreeable(agent: &Agent) -> f64 {
    0.5 + agent.identity.personality.agreeableness
}

/// Welfare tradeoff ratio: how much another's gain counts against one's own cost when helping.
/// Kin by relatedness, plus a baseline for anyone, plus liking and the balance of past gifts
fn help_weight(agent: &Agent, other: Uuid, relatedness: f64) -> f64 {
    let belief = agent.beliefs.social.get(&other);
    let sentiment = belief.map_or(0.0, |b| b.sentiment);
    let balance = belief.map_or(0.0, |b| {
        (b.food_received as f64 - b.food_given as f64) / (b.food_received + b.food_given + 1) as f64
    });
    (relatedness + STRANGER_WTR * agreeable(agent) + FRIENDSHIP_WTR * sentiment + RECIPROCITY_WTR * balance)
        .max(0.0)
}

/// How much a loss inflicted on another weighs: harm aversion, plus kinship and liking
fn harm_weight(agent: &Agent, other: Uuid, relatedness: f64) -> f64 {
    let sentiment = agent.beliefs.social.get(&other).map_or(0.0, |b| b.sentiment);
    (HARM_WEIGHT * agreeable(agent) + relatedness + FRIENDSHIP_WTR * sentiment).max(0.0)
}

/// Recent harm this agent remembers suffering (or seeing done) at another's hands
fn grievance(agent: &Agent, other: Uuid, epoch: usize) -> f64 {
    agent
        .memory
        .episodes_with(other)
        .iter()
        .filter(|e| e.category == EpisodeCategory::Conflict && e.valence <= -0.4)
        .filter(|e| epoch.saturating_sub(e.epoch) <= GRUDGE_DAYS)
        .map(|e| -e.valence)
        .sum()
}

/// Value of retaliating against someone: deterrence, which matters less the more the
/// relationship is worth keeping
fn deterrence(agent: &Agent, other: Uuid, relatedness: f64, epoch: usize) -> f64 {
    let relationship = help_weight(agent, other, relatedness).min(1.0);
    DETERRENCE_VALUE * grievance(agent, other, epoch) * (1.0 - relationship)
}

/// Social cost of harming someone in front of `witnesses` others
fn social_cost(agent: &Agent, witnesses: usize) -> f64 {
    REPUTATION_COST * witnesses as f64 + NORM_COST * agreeable(agent)
}

/// Whether a neighbour can be expected to share the food we would otherwise take: they would if
/// Hamilton's rule holds for them, judged by kinship and how far we trust them
fn expects_sharing(agent: &Agent, me: Body, target: &Neighbour, amount: u32) -> bool {
    let trust = agent.beliefs.social.get(&target.id).map_or(0.0, |b| b.trust);
    let their_weight = (target.relatedness + STRANGER_WTR + FRIENDSHIP_WTR * trust).max(0.0);
    their_weight * me.food_gain(amount) > Body::of_neighbour(target).food_loss(amount)
}

/// Net value of attacking a neighbour: seized food and deterrence, against the risk they hit
/// back, the harm done to them and the social cost
fn attack_value(agent: &Agent, me: Body, context: &ActionContext, target: &Neighbour, epoch: usize) -> f64 {
    let them = Body::of_neighbour(target);
    let loot = (target.food as f64 * SEIZED_SHARE).round() as u32;
    let retaliation = target.strength / (context.strength + target.strength).max(f64::EPSILON);
    let gain = me.food_gain(loot) + deterrence(agent, target.id, target.relatedness, epoch);
    let cost = retaliation * me.injury_loss(BLOW)
        + harm_weight(agent, target.id, target.relatedness) * (them.food_loss(loot) + them.injury_loss(BLOW))
        + social_cost(agent, context.neighbours.len() - 1);
    gain - cost
}

/// Net value of raiding the storehouse here
fn raid_value(agent: &Agent, me: Body, context: &ActionContext, owner: Uuid, stored: u32, epoch: usize) -> f64 {
    let loot = stored.min(6);
    let present = context.neighbours.iter().find(|n| n.id == owner);
    let relatedness = present.map_or(0.0, |n| n.relatedness);
    // The owner's loss comes out of their store; an absent owner is assumed modestly fed
    let owner_body = present
        .map(|n| Body { food: n.food + stored, ..Body::of_neighbour(n) })
        .unwrap_or(Body { food: stored, hunger: 0.5, health: 1.0 });
    let retaliation = present.map_or(0.0, |n| n.strength / (context.strength + n.strength).max(f64::EPSILON));
    let witnesses = context.neighbours.iter().filter(|n| n.id != owner).count();
    me.food_gain(loot) + deterrence(agent, owner, relatedness, epoch)
        - retaliation * me.injury_loss(BLOW)
        - harm_weight(agent, owner, relatedness) * owner_body.food_loss(loot)
        - social_cost(agent, witnesses)
}

/// Decide what to do by instinct
#[allow(clippy::too_many_arguments)]
pub fn decide(
    agent: &Agent,
    local_food: &LocalFood,
    context: &ActionContext,
    pending_trades: &[(usize, Uuid, &str, String, String, Option<usize>)],
    debts_owed: &[(Uuid, &str, String, Option<usize>)],
    mate_proposals: &[(Uuid, &str)],
    epoch: usize,
) -> Action {
    let mut rng = rand::rng();
    let me = Body::of(agent);
    let pos = (agent.physical.x, agent.physical.y);
    let toward = |target: (usize, usize)| Direction::toward(pos, target).map(Action::Move);
    let neighbours = &context.neighbours;
    let nearby = |id: &Uuid| neighbours.iter().any(|n| n.id == *id);

    // 1. Keep commitments to creditors who are here (reputation for reliability)
    for (creditor, name, service, _) in debts_owed {
        if !nearby(creditor) {
            continue;
        }
        if let Some(skill) = service.strip_prefix("teach ") {
            if agent.skills.level(skill) >= 0.3 {
                debug!("Instinct: teaching {} {} to settle a debt", name, skill);
                return Action::Teach { target: *creditor, skill: skill.to_string() };
            }
        }
        if service.contains("food given") && agent.physical.food > 3 {
            return Action::Give { target: *creditor, amount: (agent.physical.food / 2).max(1) };
        }
    }

    // 2. Answer trade offers: accept exchanges that leave us better off, decline the rest.
    // Making offers is left to deliberation
    if let Some((idx, _, name, offering, requesting, _)) = pending_trades.first() {
        return if trade_is_favourable(agent, me, offering, requesting) {
            debug!("Instinct: accepting trade from {}", name);
            Action::TradeAccept { proposal_index: *idx }
        } else {
            Action::TradeDecline { proposal_index: *idx }
        };
    }

    // 3. Homeostasis: eat before hunger reaches starvation within two days, rest when spent
    if agent.physical.hunger >= STARVING_AT - 2.0 * HUNGER_PER_DAY && agent.physical.food > 0 {
        return Action::Eat;
    }
    if agent.physical.energy < 0.2 {
        return Action::Rest;
    }

    // 4. Ownership contests. On someone's land with them watching, the intruder yields
    // unless the patch is worth the fight
    if let Some(owner) = context.territory_owner_nearby.and_then(|id| neighbours.iter().find(|n| n.id == id)) {
        let patch = local_food.here.min(5);
        // Engine: both take a blow, the one left healthier wins
        let wins = agent.physical.health - BLOW * 1.2 > owner.health - BLOW;
        let fight = if wins { me.food_gain(patch) } else { 0.0 }
            - me.injury_loss(BLOW * 1.2)
            - harm_weight(agent, owner.id, owner.relatedness) * Body::of_neighbour(owner).injury_loss(BLOW)
            - social_cost(agent, neighbours.len() - 1);
        return if fight > 0.0 { Action::Fight } else { Action::Submit };
    }
    // An owner challenges a trespasser when the patch is worth more to them than its use is to
    // the trespasser, weighted by how much the owner cares about them (kin and friends are tolerated)
    for (id, _) in &context.trespassers {
        if let Some(t) = neighbours.iter().find(|n| n.id == *id) {
            let patch = local_food.here.min(5);
            if me.food_loss(patch) > help_weight(agent, t.id, t.relatedness) * Body::of_neighbour(t).food_gain(patch) {
                return Action::Challenge { target: t.id };
            }
        }
    }

    // 5. Take food by force when that is worth more than foraging for it
    let forage_value = if local_food.here > 0 {
        me.food_gain(local_food.here.min(5))
    } else {
        local_food.adjacent.iter().map(|(_, _, f)| me.food_gain((*f).min(5)) / 2.0).fold(0.0, f64::max)
    };
    let best_attack = neighbours
        .iter()
        .filter(|n| !expects_sharing(agent, me, n, (n.food as f64 * SEIZED_SHARE).round() as u32))
        .map(|n| (n, attack_value(agent, me, context, n, epoch)))
        .max_by(|a, b| a.1.total_cmp(&b.1));
    let raid = context
        .raid_target
        .as_ref()
        .filter(|(x, y, ..)| (*x, *y) == pos)
        .map(|(_, _, owner, _, stored)| raid_value(agent, me, context, *owner, *stored, epoch));
    match (best_attack, raid) {
        (_, Some(r)) if r > forage_value && best_attack.is_none_or(|(_, a)| r >= a) => {
            debug!("Instinct: raiding (value {:.3} vs forage {:.3})", r, forage_value);
            return Action::Raid;
        }
        (Some((target, a)), _) if a > forage_value && a > 0.0 => {
            debug!("Instinct: attacking {} (value {:.3} vs forage {:.3})", target.name, a, forage_value);
            return Action::Attack { target: target.id };
        }
        _ => {}
    }

    // 6. Share food by Hamilton's rule, w * B > C, in survival terms: give the amount that best
    // trades our loss against the recipient's gain, weighted by how much we care about them.
    // Survival isn't everything: an agent able to breed and courting keeps back what mating costs
    let spare = agent.physical.food.saturating_sub(breeding_reserve(agent, context));
    let best_gift = neighbours
        .iter()
        .flat_map(|n| {
            let w = help_weight(agent, n.id, n.relatedness);
            let them = Body::of_neighbour(n);
            (1..=spare).map(move |k| (n.id, k, w * them.food_gain(k) - me.food_loss(k)))
        })
        .max_by(|a, b| a.2.total_cmp(&b.2));
    if let Some((target, amount, net)) = best_gift {
        if net > 0.0 {
            return Action::Give { target, amount };
        }
    }

    // 7. Reproduce when ready and a partner is here: breeding is what the rest is for
    if let Some(action) = mate(agent, context, mate_proposals) {
        return action;
    }

    // 8. Provision: forage, store surplus where it keeps, build
    if let Some(action) = provision(agent, local_food, context, &mut rng, &toward) {
        return action;
    }

    // 9. Social time, once needs are met
    if let Some(action) = socialise(agent, context, epoch, &mut rng) {
        return action;
    }

    // 10. Nothing pressing: gather materials or explore
    if rng.random::<f64>() < 0.3 {
        Action::GatherMaterials
    } else {
        random_move(&mut rng)
    }
}

/// Foraging, storage and building
fn provision(
    agent: &Agent,
    local_food: &LocalFood,
    context: &ActionContext,
    rng: &mut impl RngExt,
    toward: &dyn Fn((usize, usize)) -> Option<Action>,
) -> Option<Action> {
    let pos = (agent.physical.x, agent.physical.y);

    // Running low: draw on stores when nothing is within reach, otherwise forage
    if agent.physical.food < 3 {
        if let Some((x, y, stored)) = context.storage {
            if stored > 0 && local_food.here == 0 && local_food.adjacent.is_empty() {
                return Some(if (x, y) == pos { Action::WithdrawFood { amount: 5 } } else { toward((x, y))? });
            }
        }
        return Some(seek_food(agent, local_food, context, rng));
    }

    // Courting: build up what mating will cost
    if agent.physical.food < breeding_reserve(agent, context) {
        return Some(seek_food(agent, local_food, context, rng));
    }

    if context.has_shelter && !context.is_sheltered {
        return Some(Action::EnterShelter);
    }
    // With a storehouse, cache surplus food before it spoils, keeping a few days' worth in hand
    if let Some((x, y, stored)) = context.storage {
        if agent.physical.food > 4 && (x, y) == pos {
            return Some(Action::DepositFood { amount: agent.physical.food - 4 });
        }
        if agent.physical.food >= 10 && rng.random::<f64>() < 0.6 {
            if let Some(step) = toward((x, y)) {
                return Some(step);
            }
        }
        if agent.physical.food < 10 && local_food.here > 0 && rng.random::<f64>() < 0.4 {
            return Some(Action::Gather);
        }
        let far = x.abs_diff(pos.0).max(y.abs_diff(pos.1)) > 2;
        if stored >= 6 && far && rng.random::<f64>() < 0.25 {
            if let Some(step) = toward((x, y)) {
                return Some(step);
            }
        }
    }
    // Finish what we started
    if let Some((x, y, structure_type, _)) = context.unfinished_project {
        if (x, y) == pos {
            if agent.physical.energy > 0.3 {
                return Some(Action::Build { structure_type });
            }
        } else if agent.physical.hunger < 0.5 && agent.physical.energy > 0.4 && rng.random::<f64>() < 0.8 {
            if let Some(step) = toward((x, y)) {
                return Some(step);
            }
        }
    }
    // Help kin and friends with their building (the same welfare weight that governs sharing)
    if let Some((owner, name)) = &context.unfinished_here_owner {
        let relatedness = context.neighbours.iter().find(|n| n.id == *owner).map_or(0.0, |n| n.relatedness);
        if agent.physical.energy > 0.4 && help_weight(agent, *owner, relatedness) > 0.5 && rng.random::<f64>() < 0.5 {
            debug!("Instinct: helping {} build", name);
            return Some(Action::Build { structure_type: context.buildable_structures[0] });
        }
    }
    // Settle: claim the land around home
    if let Some(home) = context.home {
        let near_home = home.0.abs_diff(pos.0) <= 1 && home.1.abs_diff(pos.1) <= 1;
        if near_home && context.can_mark_territory && !context.here_barren && rng.random::<f64>() < 0.3 {
            return Some(Action::Mark);
        }
    }
    // Conscientious agents plan ahead: a home, then a storehouse
    if context.unfinished_project.is_none()
        && agent.identity.personality.conscientiousness > 0.5
        && rng.random::<f64>() < 0.5
    {
        use crate::structures::StructureType;
        let goal = if context.home.is_none() {
            Some([StructureType::Shelter, StructureType::LeanTo])
        } else if context.storage.is_none() {
            Some([StructureType::Storage, StructureType::Storage])
        } else {
            None
        };
        if let Some(goal) = goal {
            if let Some(structure_type) = goal.iter().find(|s| context.buildable_structures.contains(s)) {
                return Some(Action::Build { structure_type: *structure_type });
            }
            return Some(gather_materials_for(agent, goal[1], context, rng));
        }
    }
    // Make the tools that open up new food sources
    if rng.random::<f64>() < 0.3 {
        use crate::crafting::ToolType;
        let preferred = [ToolType::WoodenSpear, ToolType::StoneAxe, ToolType::Rope];
        let wanted = preferred
            .iter()
            .chain(context.craftable_tools.iter())
            .find(|t| context.craftable_tools.contains(t) && !agent.physical.has_tool(**t));
        if let Some(tool) = wanted {
            return Some(Action::Craft { tool: *tool });
        }
    }
    None
}

/// Food an agent able to breed and courting keeps for mating (capital breeders build reserves first)
fn breeding_reserve(agent: &Agent, context: &ActionContext) -> u32 {
    let courting = agent.reproduction.courtship_progress.values().any(|&p| p >= 0.3);
    if can_reproduce(agent, context) && courting { context.mating_food_cost } else { 0 }
}

fn can_reproduce(agent: &Agent, context: &ActionContext) -> bool {
    context.fertile
        && agent.reproduction.mating_cooldown == 0
        && agent.reproduction.gestation.is_none()
        && agent.physical.health > 0.5
}

/// Breed when reserves allow (capital breeding): in good condition, able to pay for it, and with
/// a partner who is here, never kin (the Westermarck effect)
fn mate(agent: &Agent, context: &ActionContext, mate_proposals: &[(Uuid, &str)]) -> Option<Action> {
    let ready = can_reproduce(agent, context)
        && Body::of(agent).value() >= BREEDING_RESERVE
        && agent.physical.food >= context.mating_food_cost;
    if !ready {
        return None;
    }
    let sentiment = |id: &Uuid| agent.beliefs.social.get(id).map_or(0.0, |b| b.sentiment);
    let unrelated = |id: &Uuid| context.neighbours.iter().any(|n| n.id == *id && n.relatedness == 0.0);
    let rested = agent.physical.energy > 0.4;

    if let Some((proposer, name)) = mate_proposals.iter().find(|(id, _)| sentiment(id) >= 0.0 && unrelated(id)) {
        debug!("Instinct: accepting mating proposal from {}", name);
        return Some(if rested { Action::Mate { target: *proposer } } else { Action::Rest });
    }
    agent
        .reproduction
        .courtship_progress
        .iter()
        .filter(|(id, score)| **score >= 0.7 && unrelated(id))
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(id, _)| if rested { Action::Mate { target: *id } } else { Action::Rest })
}

/// Courtship, teaching and gossip
fn socialise(agent: &Agent, context: &ActionContext, epoch: usize, rng: &mut impl RngExt) -> Option<Action> {
    let neighbours = &context.neighbours;
    let sentiment = |id: &Uuid| agent.beliefs.social.get(id).map_or(0.0, |b| b.sentiment);
    // Kin are never mates (the Westermarck effect)
    let unrelated = |id: &Uuid| neighbours.iter().any(|n| n.id == *id && n.relatedness == 0.0);

    let can_reproduce = can_reproduce(agent, context);

    // Courtship follows familiarity and liking (propinquity): keep courting a current partner,
    // the sociable sometimes start with whoever nearby they like best
    if can_reproduce {
        let current = agent
            .reproduction
            .courtship_progress
            .iter()
            .filter(|(id, _)| unrelated(id) && sentiment(id) >= 0.0)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(id, _)| *id);
        if let Some(target) = current {
            if rng.random::<f64>() < 0.6 {
                return Some(Action::Court { target });
            }
        } else if rng.random::<f64>() < 0.4 * agent.identity.personality.extraversion {
            let familiar = neighbours
                .iter()
                .filter(|n| n.relatedness == 0.0 && sentiment(&n.id) >= 0.0)
                .max_by(|a, b| {
                    let score = |n: &Neighbour| {
                        sentiment(&n.id) + agent.beliefs.social.get(&n.id).map_or(0.0, |b| b.interaction_count as f64 * 0.05)
                    };
                    score(a).total_cmp(&score(b))
                });
            if let Some(n) = familiar {
                return Some(Action::Court { target: n.id });
            }
        }
    }

    // Prosocial gossip: warn someone we care about of a person who has done harm
    if rng.random::<f64>() < 0.3 * agent.identity.personality.extraversion {
        let offender = agent
            .beliefs
            .social
            .iter()
            .filter(|(id, b)| b.trust <= -0.3 && grievance(agent, **id, epoch) > 0.0)
            .min_by(|a, b| a.1.trust.total_cmp(&b.1.trust))
            .map(|(id, _)| *id);
        let listener = neighbours
            .iter()
            .filter(|n| Some(n.id) != offender)
            .max_by(|a, b| help_weight(agent, a.id, a.relatedness).total_cmp(&help_weight(agent, b.id, b.relatedness)));
        if let (Some(about), Some(target)) = (offender, listener) {
            return Some(Action::Gossip { target: target.id, about });
        }
    }

    // Teach kin and close friends what we know
    let teachable = agent.skills.teachable_skills();
    if !teachable.is_empty() && agent.physical.energy > 0.3 && rng.random::<f64>() < 0.2 {
        if let Some(n) = neighbours.iter().find(|n| help_weight(agent, n.id, n.relatedness) > 0.5) {
            let skill = teachable[rng.random_range(0..teachable.len())].clone();
            return Some(Action::Teach { target: n.id, skill });
        }
    }
    None
}

/// Whether an offered exchange leaves us better off: food is valued by what it adds to our
/// survival, materials and tools by whether we lack them. Coarse, since offers arrive as text
fn trade_is_favourable(agent: &Agent, me: Body, offering: &str, requesting: &str) -> bool {
    // Only food in hand counts; promises of future food are discounted to nothing
    let food_in = |s: &str| {
        s.split(", ").filter_map(|part| part.strip_suffix(" food")?.trim().parse::<u32>().ok()).sum::<u32>()
    };
    let gain = me.food_gain(food_in(offering));
    let loss = me.food_loss(food_in(requesting));
    let tools = ["axe", "bow", "spear", "knife", "pole", "rope", "basket"];
    let tool_offered = tools.iter().any(|t| offering.contains(t)) && agent.physical.tools.is_empty();
    let tool_requested = tools.iter().any(|t| requesting.contains(t));
    let materials_offered = ["wood", "stone", "fiber", "flint"].iter().any(|m| offering.contains(m));
    let total_materials: u32 = agent.physical.materials.values().sum();
    let materials_requested = ["wood", "stone", "fiber", "flint"].iter().any(|m| requesting.contains(m));

    if tool_requested && agent.physical.tools.len() <= 1 {
        return false;
    }
    if materials_requested && total_materials < 4 {
        return false;
    }
    gain > loss || tool_offered || (materials_offered && total_materials < 5 && loss < 0.01)
}

/// Marginal value foraging: work the patch here while it yields, otherwise move to the
/// richest visible patch (respecting other owners' claims while they are watching), then to
/// remembered food, then hunt if armed, else search
fn seek_food(agent: &Agent, local_food: &LocalFood, context: &ActionContext, rng: &mut impl RngExt) -> Action {
    let pos = (agent.physical.x, agent.physical.y);
    let guarded = |x: usize, y: usize| {
        context
            .claimed_cells
            .iter()
            .any(|&(cx, cy, owner)| (cx, cy) == (x, y) && context.neighbours.iter().any(|n| n.id == owner))
    };

    if local_food.here > 0 && !guarded(pos.0, pos.1) {
        return Action::Gather;
    }

    let target = local_food
        .adjacent
        .iter()
        .filter(|(x, y, _)| !guarded(*x, *y))
        .max_by_key(|(_, _, amount)| *amount)
        .map(|(x, y, _)| (*x, *y))
        .or_else(|| {
            agent
                .beliefs
                .world
                .food_locations
                .iter()
                .filter(|loc| loc.belief.amount > 0 && (loc.x, loc.y) != pos)
                .max_by_key(|loc| loc.belief.last_seen_epoch)
                .map(|loc| (loc.x, loc.y))
        });

    if let Some(dir) = target.and_then(|t| Direction::toward(pos, t)) {
        return Action::Move(dir);
    }

    if agent.physical.has_tool(crate::crafting::ToolType::WoodenSpear) && rng.random::<f64>() < 0.5 {
        return Action::Hunt;
    }
    random_move(rng)
}

/// Gather materials for a structure, walking to the right terrain for whatever is still missing
fn gather_materials_for(
    agent: &Agent,
    structure_type: crate::structures::StructureType,
    context: &ActionContext,
    rng: &mut impl RngExt,
) -> Action {
    use crate::crafting::MaterialType;
    let registry = crate::structures::StructureRecipeRegistry::new();
    let missing: Vec<MaterialType> = registry
        .get(structure_type)
        .map(|r| {
            r.materials
                .iter()
                .filter(|(m, n)| agent.physical.materials.get(m).copied().unwrap_or(0) < **n)
                .map(|(m, _)| *m)
                .collect()
        })
        .unwrap_or_default();
    if missing.is_empty() {
        // We have everything; this cell just isn't free to build on
        return random_move(rng);
    }
    let needs_stone = missing.iter().any(|m| matches!(m, MaterialType::Stone | MaterialType::Flint));
    let needs_plants = missing.iter().any(|m| matches!(m, MaterialType::Wood | MaterialType::Fiber));
    let pos = (agent.physical.x, agent.physical.y);
    let (wanted_here, elsewhere) = if needs_plants && (!needs_stone || !context.here_barren) {
        (!context.here_barren, context.adjacent_fertile)
    } else {
        (context.here_barren, context.adjacent_barren)
    };
    if wanted_here {
        return Action::GatherMaterials;
    }
    match elsewhere.and_then(|target| Direction::toward(pos, target)) {
        Some(dir) => Action::Move(dir),
        None => random_move(rng),
    }
}

fn random_move(rng: &mut impl RngExt) -> Action {
    let directions = [
        Direction::North, Direction::South, Direction::East, Direction::West,
        Direction::NorthEast, Direction::NorthWest, Direction::SouthEast, Direction::SouthWest,
    ];
    Action::Move(directions[rng.random_range(0..8)])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(food: u32, hunger: f64, health: f64) -> Body {
        Body { food, hunger, health }
    }

    fn starving_next_to(relatedness: f64, trust: f64) -> Action {
        let mut agent = Agent::new("Hungry".into(), 5, 5, 0);
        agent.identity.personality.agreeableness = 0.5;
        agent.physical.hunger = 0.75;
        agent.physical.health = 0.6;
        let rich = Neighbour {
            id: Uuid::new_v4(), name: "Rich".into(), x: 5, y: 6,
            health: 0.9, hunger: 0.2, food: 12, strength: 0.9, relatedness,
        };
        agent.beliefs.update_trust(rich.id, "Rich", trust, 0);
        let context = ActionContext { strength: 0.6, neighbours: vec![rich], ..Default::default() };
        decide(&agent, &LocalFood::default(), &context, &[], &[], &[], 1)
    }

    #[test]
    fn test_hunger_turns_to_force_only_against_those_not_expected_to_share() {
        assert!(matches!(starving_next_to(0.0, -0.8), Action::Attack { .. }));
        assert!(!matches!(starving_next_to(0.0, 0.0), Action::Attack { .. }));
        assert!(!matches!(starving_next_to(0.5, -0.8), Action::Attack { .. }));
    }

    #[test]
    fn test_surplus_is_shared_with_the_hungry() {
        let mut agent = Agent::new("Rich".into(), 5, 5, 12);
        agent.physical.hunger = 0.2;
        let hungry = Neighbour {
            id: Uuid::new_v4(), name: "Hungry".into(), x: 5, y: 6,
            health: 0.6, hunger: 0.75, food: 0, strength: 0.6, relatedness: 0.0,
        };
        let context = ActionContext { strength: 1.0, neighbours: vec![hungry], ..Default::default() };
        let action = decide(&agent, &LocalFood::default(), &context, &[], &[], &[], 1);
        assert!(matches!(action, Action::Give { amount, .. } if amount >= 1), "got {:?}", action);
    }

    #[test]
    fn test_ready_pair_mates_before_building() {
        use crate::structures::StructureType;
        let mut agent = Agent::new("Ready".into(), 5, 5, 6);
        agent.physical.hunger = 0.3;
        let partner = Neighbour {
            id: Uuid::new_v4(), name: "Partner".into(), x: 5, y: 5,
            health: 1.0, hunger: 0.3, food: 6, strength: 1.0, relatedness: 0.0,
        };
        agent.reproduction.courtship_progress.insert(partner.id, 0.8);
        let context = ActionContext {
            strength: 1.0,
            mating_food_cost: 5,
            fertile: true,
            unfinished_project: Some((5, 5, StructureType::LeanTo, 50)),
            neighbours: vec![partner.clone()],
            ..Default::default()
        };
        let action = decide(&agent, &LocalFood::default(), &context, &[], &[], &[], 1);
        assert!(matches!(action, Action::Mate { target } if target == partner.id), "got {:?}", action);

        // ...but never with kin
        let kin = Neighbour { relatedness: 0.5, ..partner };
        let context = ActionContext { neighbours: vec![kin], ..context };
        let action = decide(&agent, &LocalFood::default(), &context, &[], &[], &[], 1);
        assert!(!matches!(action, Action::Mate { .. }), "got {:?}", action);
    }

    #[test]
    fn test_food_is_worth_more_to_the_needy() {
        let starving = body(0, 0.8, 0.5);
        let comfortable = body(10, 0.2, 1.0);
        assert!(starving.food_gain(3) > 4.0 * comfortable.food_gain(3));
        assert!(comfortable.food_loss(3) < starving.food_gain(3));
        assert_eq!(body(5, 0.5, 0.0).value(), 0.0);
    }
}
