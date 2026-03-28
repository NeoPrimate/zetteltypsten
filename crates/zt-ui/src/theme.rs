use gpui::{rgb, Hsla};

/// Height of the fake titlebar overlaid on each section (matches macOS traffic-light row).
pub const TITLEBAR_H: f32 = 36.0;

/// Convert hex u32 to GPUI Hsla.
pub fn hex(color: u32) -> Hsla {
    rgb(color).into()
}

// Catppuccin Macchiato palette
pub fn base() -> Hsla { hex(0x24273a) }
pub fn mantle() -> Hsla { hex(0x1e2030) }
pub fn crust() -> Hsla { hex(0x181926) }
pub fn surface0() -> Hsla { hex(0x363a4f) }
pub fn surface1() -> Hsla { hex(0x494d64) }
pub fn surface2() -> Hsla { hex(0x5b6078) }
pub fn overlay0() -> Hsla { hex(0x6e738d) }
pub fn overlay1() -> Hsla { hex(0x8087a2) }
pub fn overlay2() -> Hsla { hex(0x939ab7) }
pub fn text() -> Hsla { hex(0xcad3f5) }
pub fn subtext0() -> Hsla { hex(0xa5adcb) }
pub fn subtext1() -> Hsla { hex(0xb8c0e0) }
pub fn blue() -> Hsla { hex(0x8aadf4) }
pub fn lavender() -> Hsla { hex(0xb7bdf8) }
pub fn green() -> Hsla { hex(0xa6da95) }
pub fn red() -> Hsla { hex(0xed8796) }
pub fn pink() -> Hsla { hex(0xf5bde6) }
pub fn rosewater() -> Hsla { hex(0xf4dbd6) }
pub fn peach() -> Hsla { hex(0xf5a97f) }
pub fn yellow() -> Hsla { hex(0xeed49f) }
pub fn teal() -> Hsla { hex(0x8bd5ca) }
pub fn sky() -> Hsla { hex(0x91d7e3) }
pub fn sapphire() -> Hsla { hex(0x7dc4e4) }
pub fn mauve() -> Hsla { hex(0xc6a0f6) }
pub fn maroon() -> Hsla { hex(0xee99a0) }
pub fn flamingo() -> Hsla { hex(0xf0c6c6) }
