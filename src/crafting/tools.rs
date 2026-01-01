//! Tool types and tool instances.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Types of tools that can be crafted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolType {
    // Basic tools
    StoneAxe,
    StoneKnife,
    WoodenSpear,
    Rope,
    Basket,
    // Advanced tools (require tools to craft)
    FlintAxe,
    FlintKnife,
    Bow,
    FishingPole,
}

/// Quality affects effectiveness and durability
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolQuality {
    Poor,
    Standard,
    Good,
    Excellent,
}

/// A specific tool instance with durability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub id: Uuid,
    pub tool_type: ToolType,
    pub durability: u32,
    pub max_durability: u32,
    pub quality: ToolQuality,
    pub crafted_by: Option<Uuid>,
    pub crafted_epoch: usize,
}

impl ToolType {
    /// Base durability for this tool type
    pub fn base_durability(&self) -> u32 {
        match self {
            ToolType::StoneAxe => 20,
            ToolType::StoneKnife => 15,
            ToolType::WoodenSpear => 10,
            ToolType::Rope => 100,
            ToolType::Basket => 50,
            ToolType::FlintAxe => 35,
            ToolType::FlintKnife => 25,
            ToolType::Bow => 40,
            ToolType::FishingPole => 30,
        }
    }

    /// Which skill does this tool boost?
    pub fn primary_skill(&self) -> &'static str {
        match self {
            ToolType::StoneAxe | ToolType::FlintAxe | ToolType::Basket => "foraging",
            ToolType::StoneKnife | ToolType::FlintKnife | ToolType::Rope => "crafting",
            ToolType::WoodenSpear | ToolType::Bow => "hunting",
            ToolType::FishingPole => "foraging",
        }
    }

    /// Skill bonus when equipped (0.0 to 0.5)
    pub fn skill_bonus(&self) -> f64 {
        match self {
            ToolType::StoneAxe => 0.15,
            ToolType::StoneKnife => 0.10,
            ToolType::WoodenSpear => 0.20,
            ToolType::Rope => 0.05,
            ToolType::Basket => 0.10,
            ToolType::FlintAxe => 0.25,
            ToolType::FlintKnife => 0.20,
            ToolType::Bow => 0.35,
            ToolType::FishingPole => 0.15,
        }
    }

    /// Actions unlocked by this tool
    pub fn unlocked_actions(&self) -> &'static [&'static str] {
        match self {
            ToolType::StoneAxe | ToolType::FlintAxe => &["CHOP"],
            ToolType::StoneKnife | ToolType::FlintKnife => &["PROCESS"],
            ToolType::WoodenSpear | ToolType::Bow => &["HUNT"],
            ToolType::FishingPole => &["FISH"],
            ToolType::Rope | ToolType::Basket => &[],
        }
    }

    /// Display name
    pub fn display_name(&self) -> &'static str {
        match self {
            ToolType::StoneAxe => "stone axe",
            ToolType::StoneKnife => "stone knife",
            ToolType::WoodenSpear => "wooden spear",
            ToolType::Rope => "rope",
            ToolType::Basket => "basket",
            ToolType::FlintAxe => "flint axe",
            ToolType::FlintKnife => "flint knife",
            ToolType::Bow => "bow",
            ToolType::FishingPole => "fishing pole",
        }
    }

    /// Parse tool type from string
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.to_lowercase().replace(' ', "_").replace('-', "_");
        match s.as_str() {
            "stone_axe" | "stoneaxe" | "axe" => Some(ToolType::StoneAxe),
            "stone_knife" | "stoneknife" | "knife" => Some(ToolType::StoneKnife),
            "wooden_spear" | "woodenspear" | "spear" => Some(ToolType::WoodenSpear),
            "rope" => Some(ToolType::Rope),
            "basket" => Some(ToolType::Basket),
            "flint_axe" | "flintaxe" => Some(ToolType::FlintAxe),
            "flint_knife" | "flintknife" => Some(ToolType::FlintKnife),
            "bow" => Some(ToolType::Bow),
            "fishing_pole" | "fishingpole" | "pole" => Some(ToolType::FishingPole),
            _ => None,
        }
    }
}

impl ToolQuality {
    /// Effectiveness multiplier
    pub fn effectiveness_modifier(&self) -> f64 {
        match self {
            ToolQuality::Poor => 0.7,
            ToolQuality::Standard => 1.0,
            ToolQuality::Good => 1.15,
            ToolQuality::Excellent => 1.3,
        }
    }

    /// Durability multiplier
    pub fn durability_modifier(&self) -> f64 {
        match self {
            ToolQuality::Poor => 0.5,
            ToolQuality::Standard => 1.0,
            ToolQuality::Good => 1.25,
            ToolQuality::Excellent => 1.5,
        }
    }

    /// Display name
    pub fn display_name(&self) -> &'static str {
        match self {
            ToolQuality::Poor => "poor",
            ToolQuality::Standard => "standard",
            ToolQuality::Good => "good",
            ToolQuality::Excellent => "excellent",
        }
    }

    /// Alias for display_name
    pub fn name(&self) -> &'static str {
        self.display_name()
    }

    /// Determine quality from crafting skill level
    pub fn from_skill(skill: f64) -> Self {
        if skill >= 0.8 {
            ToolQuality::Excellent
        } else if skill >= 0.5 {
            ToolQuality::Good
        } else if skill >= 0.2 {
            ToolQuality::Standard
        } else {
            ToolQuality::Poor
        }
    }
}

impl Tool {
    /// Create a new tool
    pub fn new(
        tool_type: ToolType,
        quality: ToolQuality,
        crafter: Option<Uuid>,
        epoch: usize,
    ) -> Self {
        let base_dur = tool_type.base_durability();
        let max_durability = (base_dur as f64 * quality.durability_modifier()) as u32;
        Self {
            id: Uuid::new_v4(),
            tool_type,
            durability: max_durability,
            max_durability,
            quality,
            crafted_by: crafter,
            crafted_epoch: epoch,
        }
    }

    /// Use the tool (decrements durability)
    pub fn use_tool(&mut self) -> bool {
        if self.durability > 0 {
            self.durability -= 1;
            true
        } else {
            false
        }
    }

    /// Use the tool once (alias for use_tool)
    pub fn use_once(&mut self) {
        self.use_tool();
    }

    /// Check if tool is broken
    pub fn is_broken(&self) -> bool {
        self.durability == 0
    }

    /// Get the display name including quality
    pub fn display_name(&self) -> String {
        format!("{} {}", self.quality.display_name(), self.tool_type.display_name())
    }

    /// Get the effective skill bonus (quality-adjusted)
    pub fn effective_bonus(&self) -> f64 {
        self.tool_type.skill_bonus() * self.quality.effectiveness_modifier()
    }

    /// Durability as percentage
    pub fn durability_percent(&self) -> f64 {
        if self.max_durability == 0 {
            0.0
        } else {
            self.durability as f64 / self.max_durability as f64
        }
    }
}
