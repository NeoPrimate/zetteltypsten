# Zetteltypsten

One place to do all your Typst note taking (Logseq, Obsidian), publish web books (MdBook) and create PDF documents (Typst App).

Free, Open Source, Privacy First.

## Goals

- Stay as close to native Typst syntax as possible
- Privacy first — all data stays local, no telemetry
- One app for notes, books, and documents

## 🚀 Features

### <picture><source media="(prefers-color-scheme: dark)" srcset="assets/icons/network-light.svg"><source media="(prefers-color-scheme: light)" srcset="assets/icons/network-dark.svg"><img src="assets/icons/network-light.svg" width="28"></picture> Zettelsten

Inspired by [Logseq](https://logseq.com) and [Obsidian](https://obsidian.md)

Write notes, create links between your notes and visualize them as a graph.

### <picture><source media="(prefers-color-scheme: dark)" srcset="assets/icons/book-open-svgrepo-com-light.svg"><source media="(prefers-color-scheme: light)" srcset="assets/icons/book-open-svgrepo-com-dark.svg"><img src="assets/icons/book-open-svgrepo-com-light.svg" width="28"></picture> Book

Inspired by [mdBook](https://rust-lang.github.io/mdBook/)

Assemble your book from your notes. Config at `.zetteltypsten/book.toml`.

### <picture><source media="(prefers-color-scheme: dark)" srcset="assets/icons/file-svgrepo-com-light.svg"><source media="(prefers-color-scheme: light)" srcset="assets/icons/file-svgrepo-com-dark.svg"><img src="assets/icons/file-svgrepo-com-light.svg" width="28"></picture> PDF

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
