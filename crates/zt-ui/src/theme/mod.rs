//! UI theme: Catppuccin Macchiato palette + semantic design tokens.
//!
//! # Structure
//! - [`colors`] — raw palette swatches (named after Catppuccin color names)
//! - [`tokens`] — semantic mappings (role → palette color)
//!
//! Most UI code should import via `crate::theme` and call either the raw color
//! functions (e.g. `theme::blue()`) or the semantic token functions (e.g.
//! `theme::accent()`).  Both are re-exported from this module for convenience.

pub mod colors;
pub mod tokens;

// Re-export everything so existing `use crate::theme` call sites continue to work
// without modification.
pub use colors::*;
pub use tokens::*;

/// Height of the fake titlebar row in pixels.
///
/// This aligns with the macOS traffic-light buttons. Each column (activity bar,
/// sidebar, content, right panel) reserves this height at the top so that the
/// draggable window region has a consistent appearance.
pub const TITLEBAR_H: f32 = 36.0;
