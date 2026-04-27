<h1 align="center">
  <br>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/icons/network-light.svg">
    <source media="(prefers-color-scheme: light)" srcset="assets/icons/network-dark.svg">
    <img src="assets/icons/network-light.svg" width="80">
  </picture>
  <br>
  Zetteltypsten
  <br>
</h1>

<p align="center"><strong>Notes. Books. PDFs. One Typst-native workspace.</strong></p>

<p align="center">
  A GPU-accelerated, privacy-first desktop app for writing linked notes, publishing web books, and crafting beautiful PDFs — all in pure Rust, all in <a href="https://typst.app">Typst</a>.
</p>

<p align="center">
  <em>Free. Open Source. Local-only. No telemetry. No webview.</em>
</p>

---

## Why Zetteltypsten?

Most note apps render Markdown in a browser. Most PDF tools live behind a paywall. Most reference managers don't talk to your notes.

Zetteltypsten unifies them — on top of Typst, the modern successor to LaTeX — and renders everything natively on the GPU.

## Features

### 🔗 Linked Notes

A Zettelkasten built on real Typst syntax. Use `@ref` and `<label>` to link notes, `#metadata()` to tag them, and watch the knowledge graph emerge.

- Native cross-note references — no custom DSL
- Live force-directed graph view
- Backlinks, tag index, full-text search

### 📚 Book Mode

Assemble notes into a published book — table of contents, chapters, parts. Inspired by [mdBook](https://rust-lang.github.io/mdBook/), powered by Typst.

### 📄 PDF Authoring

Compose PDFs from the same notes you already write. Side-by-side preview with sub-frame incremental recompilation — typing feels instant.

### ✏️ PDF Annotation

Drop papers into your vault, highlight, annotate, extract. Inspired by [Zotero](https://www.zotero.org).

- DOI lookup → automatic BibTeX
- Annotation overlay with color-coded highlights
- One-click export to Markdown
- Search across the whole library

## Architecture

100% Rust. No JavaScript. No WebView. No HTML rendering.

| Layer        | Technology                                                     |
| ------------ | -------------------------------------------------------------- |
| UI           | [GPUI](https://www.gpui.rs/) (Metal / Vulkan)                  |
| Components   | [gpui-component](https://longbridge.github.io/gpui-component/) |
| Compiler     | [`typst`](https://github.com/typst/typst) 0.14                 |
| Renderer     | Custom Frame Painter — Typst `Frame` → GPUI Canvas             |
| Database     | SQLite (`rusqlite`) with FTS5                                  |
| File watcher | `notify` + `walkdir`                                           |

### The Frame Painter

The heart of the app. Typst compiles your source to a `Frame` tree; we walk it and paint each `FrameItem` directly to the GPU — glyphs, shapes, images, links — with zero rasterization round-trips.

Two modes share one renderer: a continuous note view and a paginated PDF preview.

### Incremental Everything

A persistent `ZettelWorld` lets [`comemo`](https://github.com/typst/comemo) memoize parsing, evaluation, and layout across keystrokes. Single-character edits recompile in **1–5 ms**.

## Getting Started

```bash
git clone https://github.com/your-org/zetteltypsten
cd zetteltypsten
cargo run --package zt-app -- /path/to/your/vault
```

A vault is just a directory of `.typ` files. Zetteltypsten creates a `.zetteltypsten/` folder for its index — everything else is yours.

## Keyboard

| Shortcut | Action               |
| -------- | -------------------- |
| `Cmd+B`  | Toggle left sidebar  |
| `Cmd+R`  | Toggle right sidebar |
| `Cmd+E`  | Edit / view mode     |
| `Cmd+N`  | New note             |
| `Cmd+S`  | Save                 |
| `Cmd+W`  | Close tab            |

## Project Layout

```
crates/
  zt-app/      Binary entry point
  zt-ui/       Workspace, Frame Painter, views
  zt-typst/    Typst World, compiler, font cache
  zt-core/     Domain types
  zt-editor/   Text buffer, cursor, history
  zt-index/    Vault indexer, link graph
  zt-db/       SQLite layer
  zt-fs/       Scanner + watcher
  zt-book/     Book builder
```

## Principles

- **Pure Rust.** No JS, no WASM, no WebView.
- **Native Typst.** `@ref`, `<label>`, `#metadata()` — never a custom DSL.
- **Local first.** Your vault is plain files. Always.
- **GPU first.** Everything renders through GPUI's canvas.
- **Use the toolkit.** Lean on `gpui-component` instead of rebuilding UI from scratch.

## License

Open source. See `LICENSE`.
