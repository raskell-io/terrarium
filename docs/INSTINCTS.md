# Instincts

When no language model is deciding for an agent (no API key, or a failed call), the agent acts
on **instinct**: fast, automatic responses that need no deliberation (`src/instinct.rs`).

Instinct is a fallback and a baseline. **Deliberate choices, including whether to fight or keep
the peace, belong to the language model.** The engine never makes those choices on an agent's
behalf. It applies the physics of an action and updates what agents perceive and believe. What
an agent does about it is up to whoever is deciding.

The instinct rules are built from models in behavioural ecology and social psychology rather
than hand-picked thresholds. Where a model gives the form of a rule but not its numbers, the
constant is listed under [Calibration](#calibration) and labelled as such.

## One currency: survival

Every instinctive choice is weighed in a single currency: an agent's chance of getting through
the next few days. That makes food gained, injuries risked and harm done to others comparable.
Using a state-dependent value as the common currency follows McNamara & Houston (1986).

From the engine's body mechanics (hunger +0.1/day, one food −0.3 hunger, starvation costs
0.1 health/day above hunger 0.8):

```
days_left = max(0, 0.8 − hunger + 0.3 · food) / 0.1  +  health / 0.1
value     = 1 − exp(−days_left / HORIZON)              (0 if dead)
```

`value` is concave: each extra day matters less the better off an agent already is. This is
diminishing marginal utility, and it drives most of what follows. Food matters far more to the
hungry than to the well-fed.

## Rules, in priority order

| # | Behaviour | Rule | Basis |
|---|-----------|------|-------|
| 1 | Keep commitments | Settle debts to creditors who are present | Indirect reciprocity: reliability is a reputation (Nowak & Sigmund 1998) |
| 2 | Answer trade offers | Accept if the exchange leaves us better off in survival terms; promises are valued at nothing. Making offers is left to deliberation | Social exchange theory (Homans 1958) |
| 3 | Homeostasis | Eat before hunger can reach starvation within two days; rest when exhausted | Drive reduction (Hull 1943) |
| 4 | Ownership contests | On someone's land with them watching: **fight** only if winning is likely and the patch is worth the injury and the harm to them, otherwise **yield** (submit). An owner **challenges** a trespasser when the patch is worth more to the owner than to the trespasser, weighted by how much the owner cares about them, so kin and friends are tolerated | Hawk–dove and the bourgeois convention (Maynard Smith & Price 1973; Maynard Smith 1982); prior-residence effect (Kokko et al. 2006); territoriality in human property (DeScioli & Wilson 2011) |
| 5 | Taking by force | Attack or raid only when (a) the victim is not expected to share anyway, and (b) the food seized, plus the value of deterring someone who has harmed us, outweighs: the risk of being hit back (scaled by relative strength), the harm done to the victim, and the social cost of witnesses and norms. It must also beat foraging | Resource-holding potential (Parker 1974); tolerated theft (Blurton Jones 1984); harm aversion (Crockett et al. 2014); revenge as deterrence (McCullough, Kurzban & Tabak 2013); scarcity and mistrust predict violence (Ember & Ember 1992) |
| 6 | Sharing | Give the amount *k* that maximises `w · B(k) − C(k)` if positive, where `B` is the recipient's survival gain and `C` our own loss. An agent able to breed and courting keeps back what mating costs, and forages until it has it | Hamilton's rule (Hamilton 1964), generalised as a welfare tradeoff ratio (Tooby et al. 2008; Delton & Robertson 2016) |
| 7 | Reproduction | Mate when in good condition (survival value ≥ `BREEDING_RESERVE`), able to pay the mating cost, and a courted partner is here; never with kin. This comes before provisioning, because breeding is what survival is for | Capital breeding (Jönsson 1997); state-dependent life histories (McNamara & Houston 1996); Westermarck effect (Lieberman, Tooby & Cosmides 2007) |
| 8 | Provisioning | Work a patch while it yields, then move to the best visible patch (skipping land whose owner is watching), then remembered food, then hunt or search. Cache surplus in storage when there is one. Conscientious agents build a home, then a storehouse | Marginal value theorem (Charnov 1976); storage in seasonal environments (Testart 1982; Binford 1980); conscientiousness and planning (Roberts et al. 2014) |
| 9 | Social time | Court the familiar and liked. Warn people you care about of anyone who has done harm. Teach kin and close friends | propinquity (Festinger, Schachter & Back 1950); prosocial gossip (Feinberg et al. 2012); teaching among kin (Kline 2015; Hewlett & Roulette 2016) |
| 10 | Otherwise | Gather materials or explore | Random search when no patch is known |

### Welfare weights

How much another agent's welfare counts, relative to one's own:

```
help weight w  = relatedness + STRANGER_WTR · A' + FRIENDSHIP_WTR · sentiment + RECIPROCITY_WTR · balance
harm weight    = HARM_WEIGHT · A' + relatedness + FRIENDSHIP_WTR · sentiment
A'             = 0.5 + agreeableness                 (0.5 to 1.5)
balance        = (food received − food given) / (total + 1)   from the gift ledger
```

- **Relatedness** is 0.5 for parents, children and full siblings, and 0.25 for half siblings,
  as far as the agent knows.
- **Agreeableness** scales concern for others' welfare and for norms. It is the Big Five trait
  most associated with constructive rather than destructive conflict tactics
  (Jensen-Campbell & Graziano 2001), and with helping (Graziano et al. 2007).
- **The reciprocity balance** makes giving contingent on past receiving, as Gurven (2004)
  documents among Ache and Hiwi foragers.

### What follows from these rules

These outcomes come out of the rules; none of them is written in as a rule of its own:

- **Sharing pre-empts conflict.** A surplus is worth little to its holder and a lot to someone
  hungry, so food flows to the hungry before force pays.
- **Force appears under scarcity.** Attacks and land disputes show up when food is short and
  trust is low.
- **Kin are spared.** Relatives are neither robbed nor driven off.

In the 100-day First Winter scenario the instinct layer produces no violence. In a harsher
variant (15% fertile land, 3 starting food) it produces starvation, occasional attacks and
three times as many territorial disputes.

## What the engine does and does not do

The engine applies the outcome of a choice: damage, food seized, a territory changing hands. It
updates perception, memory and beliefs: victims, owners and bystanders remember what they saw,
and their trust in the actor drops. It does not:

- **Defend anyone automatically.** There is no automatic raid defence and no automatic ally
  intervention. Helping, retaliating or staying out of it is each agent's choice.
- **Enforce territory.** Claims are beliefs, not walls. Anyone can gather on claimed land, and
  an owner who is nearby sees it happen.
- **Grant access automatically.** Group membership no longer gives agents access to each
  other's land.

## Calibration

These constants set magnitudes that the cited models leave open. Change them in
`src/instinct.rs`.

| Constant | Value | Meaning | Anchor |
|----------|-------|---------|--------|
| `HORIZON_DAYS` | 7 | How far ahead instinct weighs survival | Calibration |
| `STRANGER_WTR` | 0.3 | Weight on a stranger's gain when helping | Dictators give about 28% of their endowment on average (Engel 2011) |
| `HARM_WEIGHT` | 1.0 | Weight on a stranger's loss when harming them | People weigh harm to others at least as heavily as harm to themselves (Crockett et al. 2014) |
| `FRIENDSHIP_WTR` | 0.5 | Extra weight per unit of liking | Calibration |
| `RECIPROCITY_WTR` | 0.5 | Extra weight from a favourable gift balance | Calibration |
| `REPUTATION_COST` | 0.03 | Survival-equivalent cost per witness of a harmful act | Calibration |
| `NORM_COST` | 0.02 | Cost of breaking the norm against harm | Internalised norms (Gintis 2003); calibration |
| `BREEDING_RESERVE` | 0.9 | Condition needed to mate (about 16 days of reserves) | Capital breeding (Jönsson 1997); calibration |
| `GRUDGE_DAYS` | 10 | How long a harm remains a grievance | Calibration |
| `DETERRENCE_VALUE` | 0.05 | Value of retaliating, per unit of harm suffered | Calibration |

The probabilities used for social time (how often agents court, gossip or teach) are also
calibration. They are scaled by extraversion, the trait tied to social engagement.

## Limitations

- **One currency is a simplification.** Agents have no social goals beyond survival, kin and
  the relationships they value, and no status-seeking or costly signalling
  (Hawkes & Bliege Bird 2002).
- **Trade evaluation is coarse.** Offers arrive as text and are only roughly valued.
- **The triune brain is not literal.** It is a metaphor for this layer, not neuroscience. The
  models here are behavioural-ecology models of evolved strategies.

## References

- Binford, L. R. (1980). Willow smoke and dogs' tails. *American Antiquity*, 45(1), 4–20.
- Blurton Jones, N. G. (1984). A selfish origin for human food sharing: tolerated theft. *Ethology and Sociobiology*, 5, 1–3.
- Charnov, E. L. (1976). Optimal foraging, the marginal value theorem. *Theoretical Population Biology*, 9, 129–136.
- Crockett, M. J., et al. (2014). Harm to others outweighs harm to self in moral decision making. *PNAS*, 111(48), 17320–17325.
- Delton, A. W., & Robertson, T. E. (2016). How the mind makes welfare tradeoffs. *Current Opinion in Psychology*, 7, 12–16.
- DeScioli, P., & Wilson, B. J. (2011). The territorial foundations of human property. *Evolution and Human Behavior*, 32, 297–304.
- Ember, C. R., & Ember, M. (1992). Resource unpredictability, mistrust, and war. *Journal of Conflict Resolution*, 36(2), 242–262.
- Engel, C. (2011). Dictator games: a meta study. *Experimental Economics*, 14, 583–610.
- Feinberg, M., Willer, R., Stellar, J., & Keltner, D. (2012). The virtues of gossip. *Journal of Personality and Social Psychology*, 102(5), 1015–1030.
- Festinger, L., Schachter, S., & Back, K. (1950). *Social Pressures in Informal Groups*. Harper.
- Gintis, H. (2003). The hitchhiker's guide to altruism. *Journal of Theoretical Biology*, 220, 407–418.
- Graziano, W. G., Habashi, M. M., Sheese, B. E., & Tobin, R. M. (2007). Agreeableness, empathy, and helping. *Journal of Personality and Social Psychology*, 93(4), 583–599.
- Gurven, M. (2004). To give and to give not: the behavioral ecology of human food transfers. *Behavioral and Brain Sciences*, 27, 543–583.
- Hamilton, W. D. (1964). The genetical evolution of social behaviour. *Journal of Theoretical Biology*, 7, 1–52.
- Hawkes, K., & Bliege Bird, R. (2002). Showing off, handicap signaling, and the evolution of men's work. *Evolutionary Anthropology*, 11, 58–67.
- Hewlett, B. S., & Roulette, C. J. (2016). Teaching in hunter-gatherer infancy. *Royal Society Open Science*, 3, 150403.
- Homans, G. C. (1958). Social behavior as exchange. *American Journal of Sociology*, 63(6), 597–606.
- Hull, C. L. (1943). *Principles of Behavior*. Appleton-Century.
- Jensen-Campbell, L. A., & Graziano, W. G. (2001). Agreeableness as a moderator of interpersonal conflict. *Journal of Personality*, 69(2), 323–362.
- Jönsson, K. I. (1997). Capital and income breeding as alternative tactics of resource use in reproduction. *Oikos*, 78, 57–66.
- Kline, M. A. (2015). How to learn about teaching. *Behavioral and Brain Sciences*, 38, e31.
- Kokko, H., López-Sepulcre, A., & Morrell, L. J. (2006). From hawks and doves to self-consistent games of territorial behavior. *American Naturalist*, 167, 901–912.
- Lieberman, D., Tooby, J., & Cosmides, L. (2007). The architecture of human kin detection. *Nature*, 445, 727–731.
- Maynard Smith, J. (1982). *Evolution and the Theory of Games*. Cambridge University Press.
- Maynard Smith, J., & Price, G. R. (1973). The logic of animal conflict. *Nature*, 246, 15–18.
- McCullough, M. E., Kurzban, R., & Tabak, B. A. (2013). Cognitive systems for revenge and forgiveness. *Behavioral and Brain Sciences*, 36, 1–15.
- McNamara, J. M., & Houston, A. I. (1986). The common currency for behavioral decisions. *American Naturalist*, 127, 358–378.
- McNamara, J. M., & Houston, A. I. (1996). State-dependent life histories. *Nature*, 380, 215–221.
- Nowak, M. A., & Sigmund, K. (1998). Evolution of indirect reciprocity by image scoring. *Nature*, 393, 573–577.
- Parker, G. A. (1974). Assessment strategy and the evolution of fighting behaviour. *Journal of Theoretical Biology*, 47, 223–243.
- Roberts, B. W., Lejuez, C., Krueger, R. F., Richards, J. M., & Hill, P. L. (2014). What is conscientiousness and how can it be assessed? *Developmental Psychology*, 50(5), 1315–1330.
- Testart, A. (1982). The significance of food storage among hunter-gatherers. *Current Anthropology*, 23(5), 523–537.
- Tooby, J., Cosmides, L., Sell, A., Lieberman, D., & Sznycer, D. (2008). Internal regulatory variables and the design of human motivation. In A. Elliot (Ed.), *Handbook of Approach and Avoidance Motivation*. Erlbaum.
