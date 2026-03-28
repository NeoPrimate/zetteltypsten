use crate::note::NoteId;
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// A directed link from one note to another.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Link {
    pub source: NoteId,
    pub target: NoteId,
    /// Byte range in the source file where the link appears.
    pub span: Range<usize>,
    pub display_text: Option<String>,
}

/// A link parsed from source text before the target note has been resolved.
#[derive(Clone, Debug)]
pub struct UnresolvedLink {
    /// The raw string inside `#zettel("...")`.
    pub raw_target: String,
    pub display_text: Option<String>,
    /// Byte range in source file.
    pub span: Range<usize>,
}
