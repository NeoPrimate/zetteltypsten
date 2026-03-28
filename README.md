# Zetteltypsten

One place to do all your Typst note taking (Logseq, Obsidian), publish web books (MdBook) and create PDF documents (Typst App).

Free, Open Source, Privacy First.

## Stack

|               | Technology                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------- |
| Language      | Rust                                                                                        |
| UI            | [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) (GPU-accelerated)       |
| UI Components | [gpui-component](https://longbridge.github.io/gpui-component/)                              |
| Typst         | [typst](https://typst.app) 0.14 (native compilation)                                        |
| Renderer      | Custom Frame Painter (Typst Frame → GPUI Canvas)                                            |
| Code Editor   | [gpui-component Editor](https://longbridge.github.io/gpui-component/docs/components/editor) |
| Graph         | GPUI Canvas + custom Rust force simulation                                                  |
| Database      | SQLite                                                                                      |

No JavaScript. No WebView. No Electron. Pure Rust, GPU-rendered.

## Goals

- Stay as close to native Typst syntax as possible
- Privacy first — all data stays local, no telemetry
- One app for notes, books, and documents

## Modes

### Zettelsten

Inspired by [Logseq](https://logseq.com) and [Obsidian](https://obsidian.md)

Write notes, create links between your notes and visualize them as a graph.

#### Links

[Typst References](https://typst.app/docs/reference/model/ref/)

```typst
= My Section <my-section>

@my-section

#ref(<my-section>, form: "page")),
#ref(<my-section>, form: "normal")),
```

#### Tags

```typst
#metadata("mytag") <tag-mytag>
```

### Book

Inspired by [mdBook](https://rust-lang.github.io/mdBook/)

Assemble your book from your notes. Config at `.zetteltypsten/book.toml`.

### PDF

Inspired by [Typst WebApp](https://typst.app)

Full Typst document editing with live preview. The preview renders Typst's compiled Frame tree directly to GPUI Canvas — pixel-perfect, GPU-accelerated, incremental.

## Use

Some files will be exclusively used in Book, others in PDF and others shared across all of them. Annotate these with tags so you can filter them easily in the graph and file navigation pane.

## Build

```bash
cargo run --package zt-app -- /path/to/vault
```

## Architecture

See [CLAUDE.md](CLAUDE.md) for detailed architecture documentation.
