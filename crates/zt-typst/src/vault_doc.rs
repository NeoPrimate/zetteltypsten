//! Unified vault compilation.
//!
//! [`VaultDocument`] compiles ALL `.typ` files in the vault as a single Typst
//! document. A virtual master source `#include`s every note, separated by
//! boundary markers. Cross-note `@ref` works natively — no preamble injection.
//!
//! Each note gets isolated counters (headings, figures, footnotes restart at 1).
//! The introspector maps boundary markers to Y-offsets so individual notes can
//! be rendered as vertical slices of the single compiled page.

use crate::world::ZettelWorld;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use typst::layout::{Frame, PagedDocument};

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// Vertical extent of a single note within the compiled master document.
#[derive(Debug, Clone, Copy)]
pub struct NoteBoundary {
    /// Y offset (in Typst points) where this note's content begins.
    pub y_start_pt: f32,
    /// Y offset (in Typst points) where the next note begins (or document end).
    pub y_end_pt: f32,
}

/// A single compiled document containing ALL vault notes.
///
/// The vault stays as separate `.typ` files on disk. This struct assembles
/// them into one virtual document in memory, compiles it, and tracks where
/// each note's content lands in the output.
pub struct VaultDocument {
    vault_root: PathBuf,
    world: Arc<Mutex<ZettelWorld>>,
    /// The most recent successful compilation.
    doc: Option<PagedDocument>,
    /// Ordered list of note rel_paths (alphabetical, deterministic for comemo).
    note_order: Vec<String>,
    /// Map: rel_path → vertical extent in the compiled output.
    note_boundaries: HashMap<String, NoteBoundary>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Implementation
// ─────────────────────────────────────────────────────────────────────────────

impl VaultDocument {
    /// Create a new VaultDocument by scanning the vault for `.typ` files,
    /// building a virtual master document, and compiling it.
    pub fn new(vault_root: PathBuf) -> Self {
        let note_order = scan_typ_files(&vault_root);
        let master = build_master_source(&note_order);

        let mut world = ZettelWorld::new(vault_root.clone(), "__vault_master__.typ");
        world.set_source("__vault_master__.typ", master);
        let world = Arc::new(Mutex::new(world));

        let mut vd = Self {
            vault_root,
            world,
            doc: None,
            note_order,
            note_boundaries: HashMap::new(),
        };
        vd.recompile();
        vd
    }

    /// Recompile the master document. Returns `true` if compilation succeeded.
    pub fn recompile(&mut self) -> bool {
        let world = self.world.lock().unwrap();
        let result = typst::compile::<PagedDocument>(&*world);
        drop(world);

        match result.output {
            Ok(doc) => {
                self.note_boundaries = extract_boundaries(&doc, &self.note_order);
                self.doc = Some(doc);
                true
            }
            Err(errs) => {
                for e in errs.iter().take(5) {
                    tracing::error!("VaultDocument compile error: {e:?}");
                }
                false
            }
        }
    }

    /// Invalidate a single note's cached source (call after saving to disk).
    /// The next `recompile()` will re-read this file from disk.
    pub fn invalidate_note(&mut self, rel_path: &str) {
        let mut world = self.world.lock().unwrap();
        world.clear_source(rel_path);
    }

    /// Notify that a note's source has been updated in memory (before disk write).
    /// This sets the source directly in the World cache so compilation uses it
    /// without waiting for a disk write.
    pub fn update_note_source(&mut self, rel_path: &str, source: String) {
        let mut world = self.world.lock().unwrap();
        world.set_source(rel_path, source);
    }

    /// Add a new note to the vault document. Rebuilds the master source.
    pub fn add_note(&mut self, rel_path: &str) {
        if !self.note_order.contains(&rel_path.to_string()) {
            self.note_order.push(rel_path.to_string());
            self.note_order.sort();
            self.rebuild_master();
        }
    }

    /// Remove a note from the vault document. Rebuilds the master source.
    pub fn remove_note(&mut self, rel_path: &str) {
        if let Some(pos) = self.note_order.iter().position(|p| p == rel_path) {
            self.note_order.remove(pos);
            self.rebuild_master();
        }
    }

    /// Rename a note (remove old, add new). Rebuilds the master source.
    pub fn rename_note(&mut self, old_rel: &str, new_rel: &str) {
        self.remove_note(old_rel);
        self.add_note(new_rel);
    }

    /// Rebuild the virtual master source and update the World.
    fn rebuild_master(&mut self) {
        let master = build_master_source(&self.note_order);
        let mut world = self.world.lock().unwrap();
        world.set_source("__vault_master__.typ", master);
    }

    // ── Accessors ────────────────────────────────────────────────────────────

    /// The compiled document (if compilation succeeded).
    pub fn document(&self) -> Option<&PagedDocument> {
        self.doc.as_ref()
    }

    /// Get all page frames from the compiled document.
    /// With `height: auto`, this is typically a single tall page.
    pub fn frames(&self) -> Vec<Frame> {
        self.doc.as_ref()
            .map(|doc| doc.pages.iter().map(|p| p.frame.clone()).collect())
            .unwrap_or_default()
    }

    /// Extract a new Frame containing only items from a specific note.
    /// Items are Y-shifted so the note starts at Y=0.
    pub fn note_frame(&self, rel_path: &str) -> Option<Frame> {
        let boundary = self.note_boundaries.get(rel_path)?;
        let doc = self.doc.as_ref()?;
        let page = doc.pages.first()?; // height:auto → single page

        let y_start = typst::layout::Abs::pt(boundary.y_start_pt as f64);
        let y_end = typst::layout::Abs::pt(boundary.y_end_pt as f64);
        let note_height = y_end - y_start;
        let note_width = page.frame.width();

        let mut frame = Frame::soft(typst::layout::Size::new(note_width, note_height));

        for (pos, item) in page.frame.items() {
            let item_y = pos.y;
            if item_y >= y_start && item_y < y_end {
                let shifted_pos = typst::layout::Point::new(pos.x, item_y - y_start);
                frame.push(shifted_pos, item.clone());
            }
        }

        Some(frame)
    }

    /// Get the boundary for a specific note.
    pub fn note_boundary(&self, rel_path: &str) -> Option<&NoteBoundary> {
        self.note_boundaries.get(rel_path)
    }

    /// All note boundaries.
    pub fn all_boundaries(&self) -> &HashMap<String, NoteBoundary> {
        &self.note_boundaries
    }

    /// Extract NoteInfo for a specific note (filtered by its Y-range).
    pub fn note_info(&self, rel_path: &str) -> zt_index::NoteInfo {
        let Some(doc) = &self.doc else { return zt_index::NoteInfo::default() };
        let Some(boundary) = self.note_boundaries.get(rel_path) else {
            return zt_index::NoteInfo::default();
        };
        zt_index::compiled::extract_for_note(doc, boundary.y_start_pt, boundary.y_end_pt)
    }

    /// Find which note contains a given Y offset (in Typst points).
    pub fn note_at_y(&self, y_pt: f32) -> Option<&str> {
        for (path, boundary) in &self.note_boundaries {
            if y_pt >= boundary.y_start_pt && y_pt < boundary.y_end_pt {
                return Some(path.as_str());
            }
        }
        None
    }

    /// The ordered list of notes in the vault.
    pub fn note_order(&self) -> &[String] {
        &self.note_order
    }

    /// The vault root path.
    pub fn vault_root(&self) -> &Path {
        &self.vault_root
    }

    /// Access the world (for introspector queries after compile).
    pub fn world(&self) -> &Arc<Mutex<ZettelWorld>> {
        &self.world
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Scan the vault for all `.typ` files, returning sorted rel_paths.
fn scan_typ_files(vault_root: &Path) -> Vec<String> {
    let mut paths = Vec::new();
    for entry in walkdir::WalkDir::new(vault_root)
        .max_depth(10)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        // Skip hidden directories and .zetteltypsten
        if entry.path().components().any(|c| {
            c.as_os_str().to_str().map_or(false, |s| s.starts_with('.'))
        }) {
            continue;
        }
        if entry.path().extension().is_some_and(|e| e == "typ") {
            if let Ok(rel) = entry.path().strip_prefix(vault_root) {
                paths.push(rel.to_string_lossy().into_owned());
            }
        }
    }
    paths.sort();
    paths
}

/// Build the virtual master Typst source that includes all notes.
fn build_master_source(note_order: &[String]) -> String {
    let mut src = String::from(
        "#set page(height: auto)\n\
         #set heading(numbering: \"1.\")\n\
         #show heading: it => { it.body }\n\
         #show ref: it => {\n\
         \x20 let el = it.element\n\
         \x20 if el != none and el.func() == heading {\n\
         \x20   link(it.target, el.body)\n\
         \x20 } else {\n\
         \x20   it\n\
         \x20 }\n\
         }\n\n"
    );
    for (i, rel_path) in note_order.iter().enumerate() {
        let _ = writeln!(src, "#metadata(\"{rel_path}\") <__zt_{i}>");
        let _ = writeln!(src, "#include \"{rel_path}\"");
        src.push('\n');
    }
    src
}

/// Extract note boundary Y-offsets from the compiled document's introspector.
fn extract_boundaries(
    doc: &PagedDocument,
    note_order: &[String],
) -> HashMap<String, NoteBoundary> {
    use typst::foundations::{Label, Selector};
    use typst::utils::PicoStr;

    let mut boundaries = HashMap::new();
    let total_height = doc.pages.iter()
        .map(|p| p.frame.height().to_pt() as f32)
        .sum::<f32>();

    // Collect Y-offsets for each boundary marker
    let mut offsets: Vec<(usize, f32)> = Vec::new();
    for (i, _rel_path) in note_order.iter().enumerate() {
        let label_name = format!("__zt_{i}");
        let pico = PicoStr::intern(&label_name);
        let Some(label) = Label::new(pico) else { continue };
        let results = doc.introspector.query(&Selector::Label(label));
        if let Some(elem) = results.first() {
            if let Some(loc) = elem.location() {
                let pos = doc.introspector.position(loc);
                // With height: auto there's one page, so pos.point.y is the absolute Y
                offsets.push((i, pos.point.y.to_pt() as f32));
            }
        }
    }

    // Sort by index (should already be in order, but be safe)
    offsets.sort_by_key(|(idx, _)| *idx);

    // Build boundaries: each note starts at its marker and ends at the next
    for (j, &(idx, y_start)) in offsets.iter().enumerate() {
        let y_end = if j + 1 < offsets.len() {
            offsets[j + 1].1
        } else {
            total_height
        };
        if idx < note_order.len() {
            boundaries.insert(
                note_order[idx].clone(),
                NoteBoundary { y_start_pt: y_start, y_end_pt: y_end },
            );
        }
    }

    boundaries
}
