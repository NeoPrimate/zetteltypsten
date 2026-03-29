use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use crate::builder::CompiledBook;
use crate::search::build_search_index;
use crate::template;

/// Export a compiled book as a static HTML site.
pub fn export(book: &CompiledBook, output_dir: &Path) -> Result<()> {
    // Create output directories
    fs::create_dir_all(output_dir.join("css"))
        .with_context(|| "Failed to create css directory")?;
    fs::create_dir_all(output_dir.join("js"))
        .with_context(|| "Failed to create js directory")?;

    // Write CSS
    fs::write(output_dir.join("css/book.css"), BOOK_CSS)?;
    fs::write(output_dir.join("css/print.css"), PRINT_CSS)?;

    // Write JS
    fs::write(output_dir.join("js/book.js"), BOOK_JS)?;
    fs::write(output_dir.join("js/search.js"), SEARCH_JS)?;

    // Write search index
    if book.config.html.search.enable {
        let index_json = build_search_index(book);
        fs::write(output_dir.join("searchindex.json"), index_json)?;
    }

    // Find the first navigable chapter for the redirect
    let nav = book.navigable_chapters();
    let first_url = nav.first().and_then(|ch| {
        ch.file.as_ref().map(|f| {
            f.with_extension("html")
                .to_string_lossy()
                .replace('\\', "/")
        })
    });

    // Write index.html (redirect to first chapter)
    let redirect = first_url.as_deref().unwrap_or("#");
    fs::write(
        output_dir.join("index.html"),
        format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta http-equiv="refresh" content="0; url={site}{redirect}">
    <title>{title}</title>
</head>
<body>
    <p>Redirecting to <a href="{site}{redirect}">{title}</a>...</p>
</body>
</html>"#,
            site = book.config.html.site_url,
            title = book.config.title,
            redirect = redirect,
        ),
    )?;

    // Write 404 page
    fs::write(
        output_dir.join("404.html"),
        template::render_404(&book.config),
    )?;

    // Write print page
    if book.config.html.print.enable {
        fs::write(
            output_dir.join("print.html"),
            template::render_print_page(&book.config, &book.chapters),
        )?;
    }

    // Write each chapter page
    for (i, chapter) in book.chapters.iter().enumerate() {
        if chapter.is_draft {
            continue;
        }
        let Some(ref file) = chapter.file else {
            continue;
        };

        let html_path = file.with_extension("html");
        let out_path = output_dir.join(&html_path);

        // Create parent dirs
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let (prev, next) = book.neighbors(i);
        let current = file.to_string_lossy();
        let page_html = template::render_chapter_page(
            &book.config,
            chapter,
            prev,
            next,
            Some(&current),
        );

        fs::write(&out_path, page_html)
            .with_context(|| format!("Failed to write {}", out_path.display()))?;
    }

    // Copy additional CSS/JS files
    let src_root = output_dir
        .parent()
        .unwrap_or(output_dir)
        .join(&book.config.src);
    for css in &book.config.html.additional_css {
        let src = src_root.join(css);
        let dst = output_dir.join(css);
        if src.exists() {
            if let Some(p) = dst.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(&src, &dst)?;
        }
    }
    for js in &book.config.html.additional_js {
        let src = src_root.join(js);
        let dst = output_dir.join(js);
        if src.exists() {
            if let Some(p) = dst.parent() {
                fs::create_dir_all(p)?;
            }
            fs::copy(&src, &dst)?;
        }
    }

    tracing::info!("Book exported to {}", output_dir.display());
    Ok(())
}

// =============================================================================
// Embedded assets
// =============================================================================

const BOOK_CSS: &str = r##"
:root {
    /* Catppuccin Mocha (dark) */
    --bg: #1e1e2e;
    --fg: #cdd6f4;
    --sidebar-bg: #181825;
    --sidebar-fg: #bac2de;
    --sidebar-active: #89b4fa;
    --border: #313244;
    --link: #89b4fa;
    --link-hover: #b4befe;
    --heading: #cdd6f4;
    --code-bg: #181825;
    --nav-bg: #11111b;
    --section-num: #6c7086;
    --draft: #585b70;
    --search-bg: #313244;
}

body.theme-light {
    --bg: #eff1f5;
    --fg: #4c4f69;
    --sidebar-bg: #e6e9ef;
    --sidebar-fg: #5c5f77;
    --sidebar-active: #1e66f5;
    --border: #ccd0da;
    --link: #1e66f5;
    --link-hover: #7287fd;
    --heading: #4c4f69;
    --code-bg: #e6e9ef;
    --nav-bg: #dce0e8;
    --section-num: #8c8fa1;
    --draft: #9ca0b0;
    --search-bg: #ccd0da;
}

* { margin: 0; padding: 0; box-sizing: border-box; }

body {
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: var(--bg);
    color: var(--fg);
    line-height: 1.6;
    display: flex;
    min-height: 100vh;
}

/* Sidebar */
.sidebar {
    width: 280px;
    min-width: 280px;
    background: var(--sidebar-bg);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    position: fixed;
    top: 0;
    left: 0;
    bottom: 0;
    z-index: 10;
    transition: transform 0.2s;
}

.sidebar.hidden { transform: translateX(-100%); }

.sidebar-header {
    padding: 16px 20px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
}

.book-title {
    font-size: 16px;
    font-weight: 700;
    color: var(--heading);
}

.git-link {
    color: var(--sidebar-fg);
    text-decoration: none;
    font-size: 14px;
}

.sidebar-search {
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
}

.sidebar-search input {
    width: 100%;
    padding: 6px 10px;
    background: var(--search-bg);
    border: 1px solid var(--border);
    border-radius: 4px;
    color: var(--fg);
    font-size: 13px;
    outline: none;
}

.search-results {
    max-height: 300px;
    overflow-y: auto;
}

.search-results a {
    display: block;
    padding: 4px 10px;
    color: var(--link);
    text-decoration: none;
    font-size: 13px;
}

.search-results a:hover { background: var(--search-bg); }

.sidebar-scrollbox {
    flex: 1;
    overflow-y: auto;
    padding: 12px 0;
}

.chapter-list {
    list-style: none;
    padding: 0;
}

.chapter-list li { margin: 0; }

.chapter-list li a {
    display: block;
    padding: 4px 20px;
    color: var(--sidebar-fg);
    text-decoration: none;
    font-size: 14px;
    transition: background 0.1s;
}

.chapter-list li a:hover { background: var(--search-bg); }
.chapter-list li.active > a { color: var(--sidebar-active); font-weight: 600; }
.chapter-list li.draft > span { color: var(--draft); padding: 4px 20px; display: block; font-size: 14px; font-style: italic; }
.chapter-list li.part-title { padding: 12px 20px 4px; font-size: 12px; text-transform: uppercase; letter-spacing: 1px; color: var(--section-num); }
.chapter-list li.separator { padding: 0 20px; }
.chapter-list li.separator hr { border: none; border-top: 1px solid var(--border); margin: 8px 0; }

.chapter-list ol {
    list-style: none;
    padding-left: 16px;
}

.section-num { color: var(--section-num); margin-right: 4px; }

/* Page wrapper */
.page-wrapper {
    flex: 1;
    margin-left: 280px;
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    transition: margin-left 0.2s;
}

.page-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    min-height: 44px;
}

.sidebar-toggle {
    background: none;
    border: none;
    color: var(--fg);
    cursor: pointer;
    padding: 4px;
}

.edit-button {
    color: var(--link);
    text-decoration: none;
    font-size: 13px;
}

/* Main content */
main {
    flex: 1;
    max-width: 800px;
    margin: 0 auto;
    padding: 32px 24px;
    width: 100%;
}

main h1, main h2, main h3, main h4 { color: var(--heading); margin: 1.5em 0 0.5em; }
main h1 { font-size: 1.8em; }
main h2 { font-size: 1.4em; }
main h3 { font-size: 1.2em; }

main p { margin: 0.8em 0; }

main a { color: var(--link); }
main a:hover { color: var(--link-hover); }

main code {
    background: var(--code-bg);
    padding: 2px 6px;
    border-radius: 3px;
    font-size: 0.9em;
}

main pre {
    background: var(--code-bg);
    padding: 16px;
    border-radius: 6px;
    overflow-x: auto;
    margin: 1em 0;
}

main pre code { background: none; padding: 0; }

main ul, main ol { padding-left: 2em; margin: 0.5em 0; }

main blockquote {
    border-left: 3px solid var(--link);
    padding-left: 16px;
    margin: 1em 0;
    color: var(--section-num);
}

main table {
    border-collapse: collapse;
    width: 100%;
    margin: 1em 0;
}

main th, main td {
    border: 1px solid var(--border);
    padding: 8px 12px;
    text-align: left;
}

main th { background: var(--sidebar-bg); }

/* Page navigation */
.page-nav {
    display: flex;
    justify-content: space-between;
    padding: 24px;
    border-top: 1px solid var(--border);
    margin-top: auto;
}

.page-nav a {
    color: var(--link);
    text-decoration: none;
    font-size: 14px;
}

.page-nav a:hover { color: var(--link-hover); }

/* Responsive */
@media (max-width: 768px) {
    .sidebar { transform: translateX(-100%); }
    .sidebar.visible { transform: translateX(0); }
    .page-wrapper { margin-left: 0; }
}
"##;

const PRINT_CSS: &str = r##"
@media print {
    .sidebar, .page-header, .page-nav { display: none !important; }
    .page-wrapper { margin-left: 0 !important; }
    main { max-width: none; }
    .chapter-print { page-break-before: always; }
    .chapter-print:first-child { page-break-before: auto; }
}

body.print-page {
    background: white;
    color: #333;
}

body.print-page main {
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
}

body.print-page .chapter-print {
    margin-bottom: 3rem;
    padding-bottom: 2rem;
    border-bottom: 1px solid #ddd;
}
"##;

const BOOK_JS: &str = r##"
(function() {
    // Sidebar toggle
    var toggle = document.getElementById('sidebar-toggle');
    var sidebar = document.getElementById('sidebar');
    if (toggle && sidebar) {
        toggle.addEventListener('click', function() {
            sidebar.classList.toggle('hidden');
        });
    }

    // Theme switcher (Ctrl+T)
    document.addEventListener('keydown', function(e) {
        if ((e.ctrlKey || e.metaKey) && e.key === 't') {
            e.preventDefault();
            var body = document.body;
            if (body.classList.contains('theme-dark')) {
                body.className = body.className.replace('theme-dark', 'theme-light');
                localStorage.setItem('zt-book-theme', 'light');
            } else {
                body.className = body.className.replace('theme-light', 'theme-dark');
                localStorage.setItem('zt-book-theme', 'dark');
            }
        }
    });

    // Restore theme
    var saved = localStorage.getItem('zt-book-theme');
    if (saved) {
        var body = document.body;
        body.className = body.className.replace(/theme-\w+/, 'theme-' + saved);
    }

    // Keyboard navigation (left/right arrows)
    document.addEventListener('keydown', function(e) {
        if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
        var prev = document.querySelector('.nav-prev');
        var next = document.querySelector('.nav-next');
        if (e.key === 'ArrowLeft' && prev) { window.location.href = prev.href; }
        if (e.key === 'ArrowRight' && next) { window.location.href = next.href; }
    });
})();
"##;

const SEARCH_JS: &str = r##"
(function() {
    var input = document.getElementById('search-input');
    var results = document.getElementById('search-results');
    var index = null;

    if (!input || !results) return;

    // Load search index
    fetch(document.querySelector('link[rel=stylesheet]').href.replace(/css\/book\.css.*/, 'searchindex.json'))
        .then(function(r) { return r.json(); })
        .then(function(data) { index = data.entries || []; })
        .catch(function() {});

    input.addEventListener('input', function() {
        if (!index) return;
        var q = input.value.toLowerCase().trim();
        results.innerHTML = '';
        if (q.length < 2) return;

        var matches = index.filter(function(e) {
            return e.title.toLowerCase().includes(q) || e.body.toLowerCase().includes(q);
        }).slice(0, 20);

        matches.forEach(function(m) {
            var a = document.createElement('a');
            a.href = m.url;
            a.textContent = m.title;
            results.appendChild(a);
        });
    });

    // Focus search with /
    document.addEventListener('keydown', function(e) {
        if (e.key === '/' && e.target.tagName !== 'INPUT') {
            e.preventDefault();
            input.focus();
        }
    });
})();
"##;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::build_book;

    #[test]
    fn build_and_export_test_book() {
        let tmp = std::env::temp_dir().join("zt-book-export-test");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("gs")).unwrap();
        std::fs::create_dir_all(tmp.join(".zetteltypsten")).unwrap();

        std::fs::write(
            tmp.join(".zetteltypsten/book.toml"),
            r#"
[book]
title = "Test Book"

[[chapter]]
title = "Intro"
file = "intro.typ"

[[chapter]]
title = "Chapter 1"
file = "ch1.typ"

  [[chapter.sub]]
  title = "Sub 1.1"
  file = "gs/sub.typ"

[output.html]
default-theme = "dark"
"#,
        )
        .unwrap();

        std::fs::write(tmp.join("intro.typ"), "= Intro\nWelcome.\n").unwrap();
        std::fs::write(tmp.join("ch1.typ"), "= Chapter One\nContent.\n").unwrap();
        std::fs::write(tmp.join("gs/sub.typ"), "= Sub Section\nDetails.\n").unwrap();

        let book = build_book(&tmp).expect("build failed");
        assert_eq!(book.chapters.len(), 3);

        let out = tmp.join("output");
        export(&book, &out).expect("export failed");

        assert!(out.join("index.html").exists());
        assert!(out.join("404.html").exists());
        assert!(out.join("print.html").exists());
        assert!(out.join("css/book.css").exists());
        assert!(out.join("js/book.js").exists());
        assert!(out.join("intro.html").exists());
        assert!(out.join("ch1.html").exists());
        assert!(out.join("gs/sub.html").exists());
        assert!(out.join("searchindex.json").exists());

        let ch1 = std::fs::read_to_string(out.join("ch1.html")).unwrap();
        assert!(ch1.contains("Chapter One"));
        assert!(ch1.contains("Test Book"));
        assert!(ch1.contains("chapter-list"));

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
