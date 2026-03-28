# CLAUDE.md — Zetteltypsten

## Project Overview

Zetteltypsten is a privacy-first, open-source desktop application for Typst note-taking (like Obsidian/Logseq), web book publishing (like mdBook), and PDF document creation (like the Typst web app). Built entirely in Rust with GPUI (Zed's GPU-accelerated UI framework).

## Architecture

### Stack

| Layer               | Technology                                       |
| ------------------- | ------------------------------------------------ |
| UI Framework        | GPUI 0.2 (GPU-accelerated, Metal/Vulkan)         |
| UI Components       | gpui-component 0.5 (57 components)               |
| Typst Compiler      | typst 0.14 (native Rust, incremental via comemo) |
| Document Renderer   | Custom Frame Painter (typst Frame → GPUI Canvas) |
| Code Editor         | gpui-component Editor                            |
| Graph Visualization | GPUI Canvas + custom Rust force simulation       |
| Database            | SQLite via rusqlite (labels, tags, links, FTS)   |
| File System         | notify (watcher) + walkdir (scanner)             |

### No JavaScript, No WebView

The app is 100% Rust. No JS, no WASM, no WebView, no HTML/CSS rendering. Typst documents are compiled natively and rendered directly to GPUI's GPU-accelerated canvas via the Frame Painter.

### Core Rendering: Frame Painter

The Frame Painter is the heart of the app. It walks Typst's compiled `Frame` tree and paints directly to GPUI Canvas:

```
Source text → typst::compile() → PagedDocument → Frame tree
                                                      ↓
                                          FrameItem enum (exhaustive match)
                                                      ↓
                                          GPUI Canvas paint calls
```

**FrameItem mapping** (compile-time checked via exhaustive enum match):

| FrameItem variant          | GPUI paint call                         |
| -------------------------- | --------------------------------------- |
| `Group(GroupItem)`         | Push transform + clip, recurse children |
| `Text(TextItem)`           | `window.paint_glyph()` per shaped glyph |
| `Shape(Shape, Span)`       | `window.paint_path()` with fill/stroke  |
| `Image(Image, Size, Span)` | `window.paint_image()`                  |
| `Link(Destination, Size)`  | Register click region in hit-test map   |
| `Tag(Tag)`                 | Skip (metadata)                         |

**Two modes, one renderer:**

- **Note view**: Render frames continuously without page boundaries
- **PDF preview**: Render frames with page gaps, white backgrounds, shadows

**Incremental rendering:**

1. Persist `ZettelWorld` across compilations → comemo caches parsing + evaluation + layout
2. Compare page `Frame` hashes → only repaint changed pages
3. Result: ~1-5ms recompile for single-character edits

## Crate Structure

```
zetteltypsten/
├── crates/
│   ├── zt-app/          # Binary entry point, GPUI Application setup
│   ├── zt-ui/           # GPUI UI layer (workspace, views, components)
│   │   ├── workspace.rs   # Root 3-column layout
│   │   ├── sidebar.rs     # File tree (gpui-component Tree + Sidebar)
│   │   ├── tab_bar.rs     # Tabs (gpui-component Tabs)
│   │   ├── editor.rs      # Code editor (gpui-component Editor)
│   │   ├── typst_canvas.rs # Frame Painter (Frame → GPUI Canvas)
│   │   └── theme.rs       # Catppuccin Mocha color palette
│   ├── zt-typst/        # Typst compilation (World, compiler, font cache)
│   │   ├── compiler.rs    # compile_to_png_pages, compile_to_body_html_with_refs
│   │   └── world.rs       # ZettelWorld (implements typst::World)
│   ├── zt-core/         # Domain types (NoteId, Tag, Link, Config)
│   ├── zt-editor/       # Text buffer (Rope), cursor, history, block parser
│   ├── zt-index/        # Vault indexer, link graph, tag index, extractor
│   ├── zt-db/           # SQLite database (notes, labels, tags, links, FTS)
│   ├── zt-fs/           # File system scanner + watcher
│   └── zt-book/         # Book builder (book.toml, export to HTML)
```

## gpui-component Usage

Use gpui-component for ALL UI elements. Reference: https://longbridge.github.io/gpui-component/docs/components/

| UI Element       | gpui-component               | Notes                            |
| ---------------- | ---------------------------- | -------------------------------- |
| File explorer    | `Tree`                       | Hierarchical file/folder display |
| Sidebar          | `Sidebar` + `Resizable`      | Left/right collapsible panels    |
| Tabs             | `Tabs`                       | Note tabs in titlebar area       |
| Code editor      | `Editor`                     | Typst syntax highlighting        |
| Title bar        | `TitleBar`                   | macOS transparent titlebar       |
| Buttons          | `Button` + `DropdownButton`  | Activity bar, toolbar            |
| Inputs           | `Input`                      | Rename, search, new file         |
| Modals           | `Dialog` + `AlertDialog`     | Delete confirmation, settings    |
| Context menus    | `Menu`                       | Right-click on files, tabs       |
| Notifications    | `Notification`               | Save confirmation, errors        |
| Tooltips         | `Tooltip`                    | Hover hints on icons             |
| Icons            | `Icon`                       | Activity bar, file tree icons    |
| Keyboard hints   | `Kbd`                        | Shortcut display                 |
| Selects          | `Select`                     | Tag filter, view mode            |
| Scrolling        | `Scrollable` + `VirtualList` | File tree, note content          |
| Settings         | `Settings`                   | App preferences                  |
| Tags             | `Tag` + `Badge`              | Note tags display                |
| Progress         | `Progress` + `Spinner`       | Compilation, indexing            |
| Charts           | `Chart`                      | Book analytics (optional)        |
| Color picker     | `ColorPicker`                | Theme customization (optional)   |
| Resizable panels | `Resizable`                  | Sidebar width, preview split     |
| Checkboxes       | `Checkbox` + `Switch`        | Settings toggles                 |
| Lists            | `List` + `VirtualList`       | Backlinks, search results        |
| Popovers         | `Popover` + `HoverCard`      | Quick preview on hover           |

## Typst Integration

### Cross-note References

Uses native Typst `@ref` and `<label>` syntax:

```typst
// In note-a.typ:
= My Section <my-section>

// In note-b.typ:
See @my-section for details.

#ref(<my-section>, form: "page")),
#ref(<my-section>, form: "normal")),
```

The compiler injects a `#show ref` preamble that transforms cross-note `@ref`s into `#link("zt-open:note-id#label")`. The Frame Painter intercepts `FrameItem::Link(Destination::Url("zt-open:..."))` for navigation.

### Tags

Uses native Typst `#metadata()`:

```typst
#metadata("important") <tag-important>
```

### Incremental Compilation

Persist `ZettelWorld` instance across compilations. The `comemo` crate automatically memoizes:

- Parsing (span-stable incremental parser)
- Evaluation (module + closure caching)
- Layout (element-level caching)

### Font Cache

`zt_typst::world::warm_font_cache()` pre-scans system fonts on a background thread at startup. Shared globally via `OnceLock<FontCache>`.

## Database

SQLite at `.zetteltypsten/state.db`:

| Table    | Purpose                         |
| -------- | ------------------------------- |
| `notes`  | id, title, content, modified_at |
| `labels` | name, note_id, display_text     |
| `tags`   | note_id, tag                    |
| `links`  | source_id, target_id, context   |
| `refs`   | source_id, label_name           |

Incremental sync: on save, `DELETE FROM links/tags/labels/refs WHERE source_id = ?` then re-insert. Graph rebuilds from DB.

## Book Mode

Config at `.zetteltypsten/book.toml`:

```toml
[book]
title = "My Book"
authors = ["Author"]

[[part]]
title = "Getting Started"

  [[part.chapter]]
  title = "Introduction"
  file = "notes/intro.typ"
```

Book chapters can reference vault notes directly via `#include` or use book-only files in `book/`.

## Key Commands

| Shortcut    | Action                  |
| ----------- | ----------------------- |
| Cmd+B       | Toggle left sidebar     |
| Cmd+R       | Toggle right sidebar    |
| Cmd+E       | Toggle edit/view mode   |
| Cmd+N       | New note                |
| Cmd+S       | Save                    |
| Cmd+W       | Close tab               |
| Cmd+O       | Quick switcher (TODO)   |
| Cmd+P       | Command palette (TODO)  |
| Cmd+Shift+F | Full-text search (TODO) |

## Build & Run

```bash
cargo run --package zt-app -- /path/to/vault
```

## Development Guidelines

- **Pure Rust only** — no JavaScript, no WASM, no WebView
- **Use gpui-component** for every UI element — don't build custom components when one exists
- **Exhaustive enum matches** — the Frame Painter must match all `FrameItem` variants; the compiler enforces correctness on Typst upgrades
- **Background threads** for heavy work — compilation, indexing, font scanning. Never block the GPUI render loop
- **Catppuccin Mocha** theme — all colors from `theme.rs`
- **Native Typst syntax** — use `@ref`, `<label>`, `#metadata()` for links/tags, not custom functions
- **Incremental everything** — persist `ZettelWorld` for comemo caching, sync only changed files to SQLite
