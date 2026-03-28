use serde::Serialize;

use crate::builder::CompiledBook;

#[derive(Serialize)]
pub struct SearchIndex {
    pub entries: Vec<SearchEntry>,
}

#[derive(Serialize)]
pub struct SearchEntry {
    pub title: String,
    pub url: String,
    pub body: String,
}

/// Build a search index JSON from a compiled book.
pub fn build_search_index(book: &CompiledBook) -> String {
    let entries: Vec<SearchEntry> = book
        .chapters
        .iter()
        .filter(|ch| !ch.is_draft && ch.file.is_some())
        .map(|ch| {
            let url = chapter_url(ch.file.as_ref().unwrap());
            SearchEntry {
                title: ch.title.clone(),
                url,
                body: ch.plain_text.clone(),
            }
        })
        .collect();

    let index = SearchIndex { entries };
    serde_json::to_string(&index).unwrap_or_else(|_| "{}".into())
}

fn chapter_url(path: &std::path::Path) -> String {
    let s = path.with_extension("html").to_string_lossy().to_string();
    s.replace('\\', "/")
}
