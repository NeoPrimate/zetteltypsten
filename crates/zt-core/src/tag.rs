use serde::{Deserialize, Serialize};

/// A tag for organizing notes, e.g. `"rust"` or `"project/active"`.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct Tag(pub String);

impl Tag {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the tag text as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consume the tag and return its inner `String`.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for Tag {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Tag {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl From<String> for Tag {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for Tag {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<Tag> for String {
    fn from(t: Tag) -> String {
        t.0
    }
}

impl std::fmt::Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
