//! Compile-based note information extraction.
//!
//! [`extract_from_compiled`] walks the Typst introspector after compilation
//! and returns a [`NoteInfo`] with all semantic data needed by the right panel
//! and the vault index.

use typst::foundations::{Content, Element, Selector, StyleChain, Value};
use typst::introspection::{Introspector, MetadataElem};
use typst::layout::PagedDocument;
use typst::model::{HeadingElem, LinkElem, RefElem};
use typst::text::TextElem;

/// Semantic information extracted from a compiled Typst document.
#[derive(Clone, Default, Debug)]
pub struct NoteInfo {
    /// Headings as `(1-based level, plain text)`.
    pub headings: Vec<(usize, String)>,
    /// Tag strings (from `#metadata("tag") <tag-*>` or `#metadata(("a","b")) <tags>`).
    pub tags: Vec<String>,
    /// Labels declared in the document as `(name, display_text)`.
    pub labels: Vec<(String, String)>,
    /// Outgoing link targets (zt-open: rel paths and cross-note refs).
    pub outlinks: Vec<String>,
    /// `@ref` target label names.
    pub refs: Vec<String>,
}

/// Extract [`NoteInfo`] from a fully compiled [`PagedDocument`].
pub fn extract_from_compiled(doc: &PagedDocument) -> NoteInfo {
    let intro = &doc.introspector;
    let headings = extract_headings(intro);
    let tags = extract_tags(intro);
    let labels = extract_labels(intro);
    let outlinks = extract_outlinks(intro);
    let refs = extract_refs(intro);
    NoteInfo { headings, tags, labels, outlinks, refs }
}

fn extract_headings(intro: &Introspector) -> Vec<(usize, String)> {
    intro
        .query(&Selector::Elem(Element::of::<HeadingElem>(), None))
        .iter()
        .filter_map(|c| {
            let h = c.to_packed::<HeadingElem>()?;
            let level = h.resolve_level(StyleChain::default()).get();
            let text = h.body.plain_text().to_string();
            Some((level, text))
        })
        .collect()
}

fn extract_tags(intro: &Introspector) -> Vec<String> {
    let mut tags = Vec::new();
    for c in intro.query(&Selector::Elem(Element::of::<MetadataElem>(), None)).iter() {
        let Some(meta) = c.to_packed::<MetadataElem>() else { continue };
        // Only consider metadata that has a tag/tags label
        let label_name = c.label().map(|l| l.resolve().as_str().to_string()).unwrap_or_default();
        if label_name != "tags" && label_name != "tag" && !label_name.starts_with("tag-") {
            continue;
        }
        match meta.value.clone() {
            Value::Str(s) => tags.push(s.as_str().to_string()),
            Value::Array(arr) => {
                for v in arr.iter() {
                    if let Value::Str(s) = v {
                        tags.push(s.as_str().to_string());
                    }
                }
            }
            _ => {}
        }
    }
    tags
}

fn extract_labels(intro: &Introspector) -> Vec<(String, String)> {
    let mut labels = Vec::new();
    for c in intro.all() {
        let Some(label) = c.label() else { continue };
        let name = label.resolve().as_str().to_string();
        // Skip tag/metadata sentinels
        if name == "tags" || name == "tag" || name.starts_with("tag-") {
            continue;
        }
        let display = plain_text_of(c);
        labels.push((name, display));
    }
    labels
}

fn extract_outlinks(intro: &Introspector) -> Vec<String> {
    use typst::model::{Destination, LinkTarget};
    let mut links = Vec::new();
    for c in intro.query(&Selector::Elem(Element::of::<LinkElem>(), None)).iter() {
        let Some(link) = c.to_packed::<LinkElem>() else { continue };
        match &link.dest {
            LinkTarget::Dest(Destination::Url(url)) => {
                let s = url.to_string();
                // Only keep zt-open: internal links, skip http/mailto/etc.
                if s.starts_with("zt-open:") {
                    let target = s.trim_start_matches("zt-open:").to_string();
                    links.push(target);
                }
            }
            LinkTarget::Label(lbl) => {
                links.push(lbl.resolve().as_str().to_string());
            }
            _ => {}
        }
    }
    links
}

fn extract_refs(intro: &Introspector) -> Vec<String> {
    let mut refs = Vec::new();
    for c in intro.query(&Selector::Elem(Element::of::<RefElem>(), None)).iter() {
        let Some(r) = c.to_packed::<RefElem>() else { continue };
        refs.push(r.target.resolve().as_str().to_string());
    }
    refs
}

/// Extract plain text from a content node by checking for a TextElem child,
/// or falling back to `Content::plain_text()`.
fn plain_text_of(c: &Content) -> String {
    if let Some(t) = c.to_packed::<TextElem>() {
        return t.text.to_string();
    }
    c.plain_text().to_string()
}
