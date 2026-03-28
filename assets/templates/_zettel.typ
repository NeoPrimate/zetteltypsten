// _zettel.typ — Zetteltypsten vault support library
// This file is auto-generated. You can customize the rendering,
// but the function signatures must remain unchanged for link extraction.

/// Create a link to another note in this vault.
/// - target: the note ID (vault-relative path without .typ extension)
/// - display: optional display text (defaults to target)
#let zettel(target, display: none) = {
  let label = if display != none { display } else { target }
  link("zettel:" + target, text(fill: rgb("#89b4fa"), label))
}

/// Declare tags for this note.
/// Usage: #tags("rust", "project/active")
#let tags(..items) = {
  let t = items.pos()
  for tag in t {
    box(inset: (x: 4pt, y: 2pt), radius: 3pt, fill: luma(230))[
      \##tag
    ]
    h(4pt)
  }
}
