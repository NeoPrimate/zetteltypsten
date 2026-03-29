//! Semantic theme tokens that map UI roles to Catppuccin Macchiato palette values.
//!
//! Components should prefer these over raw palette colors so that the mapping
//! between palette and purpose lives in one place and can be adjusted without
//! touching component code.

use super::colors;
use gpui::Hsla;

// ── Backgrounds ───────────────────────────────────────────────────────────────

/// Primary app background (content area).
pub fn bg_primary()  -> Hsla { colors::base() }
/// Secondary / sidebar background.
pub fn bg_sidebar()  -> Hsla { colors::crust() }
/// Popover / panel background.
pub fn bg_panel()    -> Hsla { colors::mantle() }
/// Raised surface (list items, cards).
pub fn bg_surface()  -> Hsla { colors::surface0() }
/// Hover highlight on interactive elements.
pub fn bg_hover()    -> Hsla { colors::surface0() }

// ── Text ──────────────────────────────────────────────────────────────────────

/// Primary body text.
pub fn text_primary() -> Hsla { colors::text() }
/// Secondary / muted text (labels, placeholders).
pub fn text_muted()   -> Hsla { colors::subtext0() }

// ── Borders ───────────────────────────────────────────────────────────────────

/// Default border between panes/panels.
pub fn border_default() -> Hsla { colors::surface0() }

// ── Accent ────────────────────────────────────────────────────────────────────

/// Interactive accent (links, active icons, cursor).
pub fn accent() -> Hsla { colors::blue() }
/// Accent in a slightly lighter shade (hover states on accent).
pub fn accent_hover() -> Hsla { colors::lavender() }
