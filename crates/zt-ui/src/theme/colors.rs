//! Catppuccin Macchiato raw palette — one function per named color swatch.
//!
//! All values are taken directly from the official Catppuccin Macchiato palette.
//! Components should generally prefer the semantic tokens in [`super::tokens`]
//! rather than reaching for these raw palette colors.

use gpui::{rgb, Hsla};

/// Convert a 24-bit hex color to GPUI [`Hsla`].
#[inline]
pub fn hex(color: u32) -> Hsla {
    rgb(color).into()
}

// ── Base layers ───────────────────────────────────────────────────────────────
pub fn base()     -> Hsla { hex(0x24273a) }
pub fn mantle()   -> Hsla { hex(0x1e2030) }
pub fn crust()    -> Hsla { hex(0x181926) }

// ── Surfaces ──────────────────────────────────────────────────────────────────
pub fn surface0() -> Hsla { hex(0x363a4f) }
pub fn surface1() -> Hsla { hex(0x494d64) }
// ── Overlays ──────────────────────────────────────────────────────────────────
pub fn overlay0() -> Hsla { hex(0x6e738d) }

// ── Text ──────────────────────────────────────────────────────────────────────
pub fn text()     -> Hsla { hex(0xcad3f5) }
pub fn subtext0() -> Hsla { hex(0xa5adcb) }

// ── Accent colors ─────────────────────────────────────────────────────────────
pub fn blue()      -> Hsla { hex(0x8aadf4) }
pub fn green()     -> Hsla { hex(0xa6da95) }
pub fn red()       -> Hsla { hex(0xed8796) }
pub fn peach()     -> Hsla { hex(0xf5a97f) }
pub fn yellow()    -> Hsla { hex(0xeed49f) }
pub fn teal()      -> Hsla { hex(0x8bd5ca) }
pub fn sky()       -> Hsla { hex(0x91d7e3) }
pub fn sapphire()  -> Hsla { hex(0x7dc4e4) }
pub fn mauve()     -> Hsla { hex(0xc6a0f6) }
