//! Reusable GPUI component functions.
//!
//! Each module provides one or more free functions that return GPUI elements
//! (typically `AnyElement` or `Div`). These are the building blocks shared
//! across the workspace, file tree, and right-panel views.

pub mod empty_state;
pub mod sidebar_item;
pub mod tag_badge;

pub use empty_state::empty_state;
pub use sidebar_item::sidebar_item;
pub use tag_badge::{tag_badge, tag_color};
