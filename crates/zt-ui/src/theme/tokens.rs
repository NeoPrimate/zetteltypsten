//! Semantic theme tokens that map UI roles to Catppuccin Macchiato palette values.
//!
//! Components should prefer these over raw palette colors so that the mapping
//! between palette and purpose lives in one place and can be adjusted without
//! touching component code.

use super::colors;
use gpui::Hsla;

// ── Backgrounds ───────────────────────────────────────────────────────────────

/// Raised surface (list items, cards).
pub fn bg_surface()  -> Hsla { colors::surface0() }
/// Hover highlight on interactive elements.
pub fn bg_hover()    -> Hsla { colors::surface0() }

// ── Text ──────────────────────────────────────────────────────────────────────

/// Primary body text.
pub fn text_primary() -> Hsla { colors::text() }
/// Secondary / muted text (labels, placeholders).
pub fn text_muted()   -> Hsla { colors::subtext0() }

// ── Accent ────────────────────────────────────────────────────────────────────

/// Interactive accent (links, active icons, cursor).
pub fn accent() -> Hsla { colors::blue() }
