//! The viewer's colours: a soft palette on a deep slate background, easier on the eyes than
//! the terminal's stock colours. Each stock colour has a counterpart here, so the meaning of
//! every colour in the panels stays the same.

use ratatui::style::Color;

/// Background behind everything
pub const BG: Color = Color::Rgb(0x1f, 0x23, 0x29);
/// Panel borders
pub const BORDER: Color = Color::Rgb(0x4a, 0x51, 0x60);
/// Ordinary text
pub const FG: Color = Color::Rgb(0xd8, 0xd4, 0xcc);
/// Secondary text (was dark grey)
pub const MUTED: Color = Color::Rgb(0x7f, 0x86, 0x91);
/// Status bar background
pub const BAR: Color = Color::Rgb(0x2c, 0x31, 0x3a);

pub const RED: Color = Color::Rgb(0xe0, 0x84, 0x7d);
pub const GREEN: Color = Color::Rgb(0x9f, 0xc5, 0x8b);
pub const YELLOW: Color = Color::Rgb(0xe6, 0xc6, 0x83);
pub const BLUE: Color = Color::Rgb(0x82, 0xa6, 0xd6);
pub const MAGENTA: Color = Color::Rgb(0xc6, 0x9f, 0xd6);
pub const CYAN: Color = Color::Rgb(0x80, 0xc2, 0xba);
pub const WHITE: Color = Color::Rgb(0xee, 0xec, 0xe6);

pub const LIGHT_RED: Color = Color::Rgb(0xee, 0xa7, 0xa1);
pub const LIGHT_GREEN: Color = Color::Rgb(0xbd, 0xda, 0xab);
pub const LIGHT_BLUE: Color = Color::Rgb(0xa9, 0xc5, 0xea);
pub const LIGHT_MAGENTA: Color = Color::Rgb(0xda, 0xbe, 0xe6);
pub const LIGHT_CYAN: Color = Color::Rgb(0xa6, 0xd8, 0xd1);

/// Map tints for claimed land: the selected person's, someone else's, and unknown
pub const LAND_OWN: Color = Color::Rgb(0x2b, 0x3a, 0x30);
pub const LAND_FOREIGN: Color = Color::Rgb(0x3d, 0x2d, 0x30);
pub const LAND_NEUTRAL: Color = Color::Rgb(0x2e, 0x33, 0x3f);
/// Bare ground on the map
pub const GROUND: Color = Color::Rgb(0x4b, 0x50, 0x59);
