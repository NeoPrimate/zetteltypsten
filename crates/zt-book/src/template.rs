use crate::builder::CompiledChapter;
use crate::config::{BookConfig, Chapter};

/// Generate the full HTML page for a single chapter.
pub fn render_chapter_page(
    config: &BookConfig,
    chapter: &CompiledChapter,
    prev: Option<&CompiledChapter>,
    next: Option<&CompiledChapter>,
    current_path: Option<&str>,
) -> String {
    let toc_html = render_toc(config, current_path);
    let nav_html = render_page_nav(prev, next, &config.html.site_url);
    let additional_css = render_additional_css(config);
    let additional_js = render_additional_js(config);

    let edit_button = if let (Some(template), Some(path)) =
        (&config.html.edit_url_template, chapter.file.as_ref())
    {
        let url = template.replace("{path}", &path.to_string_lossy());
        format!(r#"<a class="edit-button" href="{url}" target="_blank">Suggest an edit</a>"#)
    } else {
        String::new()
    };

    let git_link = config
        .html
        .git_repo_url
        .as_ref()
        .map(|url| format!(r#"<a class="git-link" href="{url}" target="_blank">⟨/⟩</a>"#))
        .unwrap_or_default();

    let section_prefix = chapter
        .number
        .as_ref()
        .map(|nums| {
            let s: Vec<String> = nums.iter().map(|n| n.to_string()).collect();
            format!("{}. ", s.join("."))
        })
        .unwrap_or_default();

    format!(
        r##"<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{section}{title} - {book_title}</title>
    <meta name="description" content="{description}">
    <link rel="stylesheet" href="{site}css/book.css">
    {additional_css}
</head>
<body class="theme-{theme}">
    <nav class="sidebar" id="sidebar">
        <div class="sidebar-header">
            <h1 class="book-title">{book_title}</h1>
            {git_link}
        </div>
        <div class="sidebar-search">
            <input type="search" id="search-input" placeholder="Search..." autocomplete="off">
            <div id="search-results" class="search-results"></div>
        </div>
        <div class="sidebar-scrollbox">
            {toc}
        </div>
    </nav>

    <div class="page-wrapper">
        <div class="page-header">
            <button id="sidebar-toggle" class="sidebar-toggle" aria-label="Toggle sidebar">
                <svg viewBox="0 0 20 20" width="20" height="20">
                    <path d="M3 5h14M3 10h14M3 15h14" stroke="currentColor" stroke-width="2" fill="none"/>
                </svg>
            </button>
            {edit_button}
        </div>

        <main id="content">
            <article>
                {content}
            </article>
        </main>

        {nav}
    </div>

    <script src="{site}js/book.js"></script>
    <script src="{site}js/search.js"></script>
    {additional_js}
</body>
</html>"##,
        lang = config.language,
        section = section_prefix,
        title = chapter.title,
        book_title = config.title,
        description = config.description,
        site = config.html.site_url,
        theme = config.html.default_theme,
        toc = toc_html,
        content = chapter.html,
        nav = nav_html,
        additional_css = additional_css,
        additional_js = additional_js,
        edit_button = edit_button,
        git_link = git_link,
    )
}

fn render_toc(config: &BookConfig, current_path: Option<&str>) -> String {
    let mut html = String::from("<ol class=\"chapter-list\">\n");
    let site = &config.html.site_url;
    let no_label = config.html.no_section_label;

    for ch in &config.chapters {
        render_chapter_toc(&mut html, ch, current_path, site, no_label);
    }

    html.push_str("</ol>\n");
    html
}

fn render_chapter_toc(
    html: &mut String,
    ch: &Chapter,
    current_path: Option<&str>,
    site_url: &str,
    no_section_label: bool,
) {
    let is_current = current_path
        .zip(ch.file.as_ref())
        .map(|(c, f)| c == f.to_string_lossy().as_ref())
        .unwrap_or(false);

    let class = if is_current { " class=\"active\"" } else { "" };

    let section = if !no_section_label {
        ch.number
            .as_ref()
            .map(|nums| {
                let s: Vec<String> = nums.iter().map(|n| n.to_string()).collect();
                format!("<span class=\"section-num\">{}.</span> ", s.join("."))
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    if let Some(file) = ch.file.as_ref() {
        let url = format!(
            "{}{}",
            site_url,
            file.with_extension("html")
                .to_string_lossy()
                .replace('\\', "/")
        );
        html.push_str(&format!(
            "<li{class}><a href=\"{url}\">{section}{}</a>",
            ch.title
        ));
    } else {
        html.push_str(&format!(
            "<li class=\"draft\"><span>{section}{}</span>",
            ch.title
        ));
    }

    if !ch.children.is_empty() {
        html.push_str("\n<ol>\n");
        for child in &ch.children {
            render_chapter_toc(html, child, current_path, site_url, no_section_label);
        }
        html.push_str("</ol>\n");
    }

    html.push_str("</li>\n");
}

fn render_page_nav(
    prev: Option<&CompiledChapter>,
    next: Option<&CompiledChapter>,
    site_url: &str,
) -> String {
    let mut html = String::from("<nav class=\"page-nav\">\n");

    if let Some(p) = prev {
        if let Some(path) = p.file.as_ref() {
            let url = format!(
                "{}{}",
                site_url,
                path.with_extension("html")
                    .to_string_lossy()
                    .replace('\\', "/")
            );
            html.push_str(&format!(
                "<a class=\"nav-prev\" href=\"{url}\">← {}</a>\n",
                p.title
            ));
        }
    } else {
        html.push_str("<span></span>\n");
    }

    if let Some(n) = next {
        if let Some(path) = n.file.as_ref() {
            let url = format!(
                "{}{}",
                site_url,
                path.with_extension("html")
                    .to_string_lossy()
                    .replace('\\', "/")
            );
            html.push_str(&format!(
                "<a class=\"nav-next\" href=\"{url}\">{} →</a>\n",
                n.title
            ));
        }
    } else {
        html.push_str("<span></span>\n");
    }

    html.push_str("</nav>\n");
    html
}

fn render_additional_css(config: &BookConfig) -> String {
    config
        .html
        .additional_css
        .iter()
        .map(|p| {
            format!(
                r#"<link rel="stylesheet" href="{}{}">"#,
                config.html.site_url,
                p.display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_additional_js(config: &BookConfig) -> String {
    config
        .html
        .additional_js
        .iter()
        .map(|p| {
            format!(
                r#"<script src="{}{}"></script>"#,
                config.html.site_url,
                p.display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Render the print page (all chapters combined).
pub fn render_print_page(config: &BookConfig, chapters: &[CompiledChapter]) -> String {
    let mut all_content = String::new();
    for ch in chapters.iter().filter(|c| !c.is_draft) {
        let section = ch
            .number
            .as_ref()
            .map(|nums| {
                let s: Vec<String> = nums.iter().map(|n| n.to_string()).collect();
                format!("{}. ", s.join("."))
            })
            .unwrap_or_default();
        all_content.push_str(&format!(
            "<div class=\"chapter-print\">\n<h1>{}{}</h1>\n{}\n</div>\n",
            section, ch.title, ch.html
        ));
    }

    format!(
        r##"<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="utf-8">
    <title>{title} - Print</title>
    <link rel="stylesheet" href="{site}css/book.css">
    <link rel="stylesheet" href="{site}css/print.css">
</head>
<body class="print-page">
    <main>
        <h1 class="book-title">{title}</h1>
        {content}
    </main>
</body>
</html>"##,
        lang = config.language,
        title = config.title,
        site = config.html.site_url,
        content = all_content,
    )
}

/// Render the 404 page.
pub fn render_404(config: &BookConfig) -> String {
    format!(
        r##"<!DOCTYPE html>
<html lang="{lang}">
<head>
    <meta charset="utf-8">
    <title>Page Not Found - {title}</title>
    <link rel="stylesheet" href="{site}css/book.css">
</head>
<body class="theme-{theme}">
    <main style="text-align:center; padding:4rem;">
        <h1>404</h1>
        <p>Page not found.</p>
        <a href="{site}">Return to book</a>
    </main>
</body>
</html>"##,
        lang = config.language,
        title = config.title,
        site = config.html.site_url,
        theme = config.html.default_theme,
    )
}
