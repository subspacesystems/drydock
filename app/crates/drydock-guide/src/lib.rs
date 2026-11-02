// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! User guide generator: Markdown pages to static HTML with an offline search index.
//!
//! Behaviour: `docs/specs/m8-guide-build.md`. Pure functions first ([`parse_summary`],
//! [`render_markdown`], [`sidebar_html`], [`index_entries`], [`index_json`]), then [`build`]
//! ties them to the file system.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};

/// One line of `SUMMARY.md`: a page in the contents, `level` 0 at the top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryEntry {
    pub title: String,
    /// Source path relative to the guide folder, `/`-separated, e.g. `drawing/lines.md`.
    pub path: String,
    pub level: usize,
}

/// A heading found in a page (levels 1 to 6), with its unique anchor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub id: String,
}

/// The text under one heading of level 1 to 3 (deeper headings stay inside it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub heading: String,
    pub id: String,
    /// Plain text, whitespace collapsed to single spaces.
    pub text: String,
}

/// One Markdown page, rendered.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rendered {
    /// First level-1 heading's text; empty when the page has none.
    pub title: String,
    /// The page's HTML body: headings carry `id`, links to `.md` files point to `.html`.
    pub body_html: String,
    pub headings: Vec<Heading>,
    pub sections: Vec<Section>,
    /// Every link target as written in the Markdown (`a.md#x`, `https://...`, `#top`).
    pub links: Vec<String>,
    /// Every image target as written.
    pub images: Vec<String>,
}

/// One search record: a section of a page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexEntry {
    pub title: String,
    pub heading: String,
    pub url: String,
    pub text: String,
}

/// A page in a [`BuildReport`]: the contents entry and the file written for it.
///
/// It compares equal to either name, so callers may talk about the Markdown source or the
/// HTML output without converting between them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PagePath {
    /// Source path relative to the guide folder, e.g. `drawing/lines.md`.
    pub source: String,
    /// Output path relative to the output folder, e.g. `drawing/lines.html`.
    pub output: String,
}

impl PagePath {
    /// The Markdown source path.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The HTML output path.
    pub fn output(&self) -> &str {
        &self.output
    }
}

impl PartialEq<&str> for PagePath {
    fn eq(&self, other: &&str) -> bool {
        self.source == *other || self.output == *other
    }
}

impl PartialEq<String> for PagePath {
    fn eq(&self, other: &String) -> bool {
        self.source == *other || self.output == *other
    }
}

impl PartialEq<str> for PagePath {
    fn eq(&self, other: &str) -> bool {
        self.source == other || self.output == other
    }
}

/// What [`build`] wrote.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildReport {
    /// Pages in contents order; each compares equal to its source or output path.
    pub pages: Vec<PagePath>,
    /// `.md` files in the source tree that SUMMARY.md does not list.
    pub orphans: Vec<String>,
    /// Source files copied unchanged (images and so on), relative paths.
    pub assets: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuideError {
    /// SUMMARY.md is missing or a line is malformed.
    Summary {
        line: usize,
        message: String,
    },
    /// A contents entry names a file that does not exist.
    MissingPage {
        path: String,
    },
    /// A relative link points to a file that does not exist or leaves the guide folder.
    BrokenLink {
        page: String,
        target: String,
    },
    /// A link to a page or to `#x` names an anchor the page does not have.
    BrokenAnchor {
        page: String,
        target: String,
    },
    /// An image file does not exist.
    MissingImage {
        page: String,
        target: String,
    },
    /// An image is not a relative path (a URL or an absolute path): the guide fetches nothing.
    ExternalImage {
        page: String,
        target: String,
    },
    Io(String),
}

impl fmt::Display for GuideError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GuideError::Summary { line, message } => {
                write!(f, "SUMMARY.md line {line}: {message}")
            }
            GuideError::MissingPage { path } => write!(f, "missing page: {path}"),
            GuideError::BrokenLink { page, target } => {
                write!(f, "{page}: broken link: {target}")
            }
            GuideError::BrokenAnchor { page, target } => {
                write!(f, "{page}: broken anchor: {target}")
            }
            GuideError::MissingImage { page, target } => {
                write!(f, "{page}: missing image: {target}")
            }
            GuideError::ExternalImage { page, target } => {
                write!(f, "{page}: external image: {target}")
            }
            GuideError::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for GuideError {}

/// The anchor for a heading text, before duplicates are numbered: lower case, letters and
/// digits kept, every other run of characters becomes one `-`, no leading or trailing `-`;
/// `section` when nothing is left.
pub fn heading_id(text: &str) -> String {
    let mut out = String::new();
    let mut separator = false;
    for c in text.chars() {
        if c.is_alphanumeric() {
            for lower in c.to_lowercase() {
                out.push(lower);
            }
            separator = false;
        } else if !out.is_empty() && !separator {
            out.push('-');
            separator = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "section".to_string()
    } else {
        out
    }
}

/// Parses `SUMMARY.md` (format in the spec).
pub fn parse_summary(text: &str) -> Result<Vec<SummaryEntry>, GuideError> {
    let mut entries: Vec<SummaryEntry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut previous_level: Option<usize> = None;

    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }

        let mut indent = 0;
        let mut rest = line;
        while let Some(tail) = rest.strip_prefix(' ') {
            indent += 1;
            rest = tail;
        }
        if rest.starts_with('\t') {
            return Err(summary_error(
                number,
                "tabs are not allowed for indentation",
            ));
        }
        if indent % 2 != 0 {
            return Err(summary_error(
                number,
                "indentation must be a multiple of two spaces",
            ));
        }
        let level = indent / 2;
        if let Some(previous) = previous_level {
            if level > previous + 1 {
                return Err(summary_error(
                    number,
                    "entry is more than one level too deep",
                ));
            }
        } else if level != 0 {
            return Err(summary_error(number, "the first entry must be at level 0"));
        }

        let body = rest
            .strip_prefix("- ")
            .ok_or_else(|| summary_error(number, "expected '- [Title](path.md)'"))?;
        if !body.starts_with('[') {
            return Err(summary_error(number, "expected a title in brackets"));
        }
        let close = body
            .find(']')
            .ok_or_else(|| summary_error(number, "expected a title in brackets"))?;
        let title = &body[1..close];
        let after_title = &body[close + 1..];
        let target = after_title
            .strip_prefix('(')
            .and_then(|tail| tail.strip_suffix(')'))
            .ok_or_else(|| summary_error(number, "expected a link to a .md page"))?;

        if target.is_empty() || !target.ends_with(".md") {
            return Err(summary_error(number, "the link must name a .md file"));
        }
        if target.starts_with('/') {
            return Err(summary_error(number, "the path must be relative"));
        }
        if has_scheme(target) {
            return Err(summary_error(number, "the path must not have a scheme"));
        }
        if target.split('/').any(|part| part == "..") {
            return Err(summary_error(number, "the path must not contain '..'"));
        }
        if !seen.insert(target.to_string()) {
            return Err(summary_error(number, "the page is listed twice"));
        }

        entries.push(SummaryEntry {
            title: title.to_string(),
            path: target.to_string(),
            level,
        });
        previous_level = Some(level);
    }

    Ok(entries)
}

fn summary_error(line: usize, message: &str) -> GuideError {
    GuideError::Summary {
        line,
        message: message.to_string(),
    }
}

/// Renders one Markdown page (rules in the spec).
pub fn render_markdown(markdown: &str) -> Rendered {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let events: Vec<Event<'_>> = Parser::new_ext(markdown, options).collect();

    let mut title = String::new();
    let mut headings: Vec<Heading> = Vec::new();
    let mut sections: Vec<Section> = Vec::new();
    let mut links: Vec<String> = Vec::new();
    let mut images: Vec<String> = Vec::new();
    let mut output: Vec<Event<'static>> = Vec::new();
    let mut id_counts: HashMap<String, usize> = HashMap::new();
    let mut current: Option<(String, String, String)> = None;
    let mut in_image = 0usize;

    let mut i = 0;
    while i < events.len() {
        match events[i].clone() {
            Event::Start(Tag::Heading { level, .. }) => {
                let mut text = String::new();
                let mut j = i + 1;
                while j < events.len() {
                    match events[j].clone() {
                        Event::End(TagEnd::Heading(_)) => break,
                        Event::Text(value) | Event::Code(value) => text.push_str(&value),
                        Event::SoftBreak | Event::HardBreak => text.push(' '),
                        _ => {}
                    }
                    j += 1;
                }

                let base = heading_id(&text);
                let count = id_counts.entry(base.clone()).or_insert(0);
                let id = if *count == 0 {
                    base.clone()
                } else {
                    format!("{base}-{count}")
                };
                *count += 1;

                let heading_level = level as u8;
                headings.push(Heading {
                    level: heading_level,
                    text: text.clone(),
                    id: id.clone(),
                });
                if heading_level <= 3 {
                    if let Some((heading, section_id, section_text)) = current.take() {
                        sections.push(finish_section(heading, section_id, section_text));
                    }
                    if heading_level == 1 && title.is_empty() {
                        title = text.clone();
                    }
                    current = Some((text.clone(), id.clone(), String::new()));
                } else if let Some((_, _, section_text)) = current.as_mut() {
                    section_text.push(' ');
                    section_text.push_str(&text);
                    section_text.push(' ');
                }

                output.push(Event::Start(Tag::Heading {
                    level,
                    id: Some(CowStr::from(id)),
                    classes: Vec::new(),
                    attrs: Vec::new(),
                }));
                for event in events.iter().take(j).skip(i + 1) {
                    output.push(rewrite_event(event.clone(), &mut links, &mut images));
                }
                output.push(Event::End(TagEnd::Heading(level)));
                i = j + 1;
            }
            event => {
                match &event {
                    Event::Start(Tag::Image { .. }) => in_image += 1,
                    Event::End(TagEnd::Image) => in_image = in_image.saturating_sub(1),
                    Event::Text(value) | Event::Code(value) => {
                        append_section_text(&mut current, in_image, value);
                    }
                    Event::SoftBreak | Event::HardBreak => {
                        append_section_space(&mut current, in_image);
                    }
                    Event::Start(Tag::Paragraph)
                    | Event::Start(Tag::Item)
                    | Event::Start(Tag::CodeBlock(_))
                    | Event::Start(Tag::TableHead)
                    | Event::Start(Tag::TableRow)
                    | Event::Start(Tag::TableCell)
                    | Event::Start(Tag::BlockQuote(_))
                    | Event::Start(Tag::List(_))
                    | Event::End(TagEnd::Paragraph)
                    | Event::End(TagEnd::Item)
                    | Event::End(TagEnd::CodeBlock)
                    | Event::End(TagEnd::TableHead)
                    | Event::End(TagEnd::TableRow)
                    | Event::End(TagEnd::TableCell)
                    | Event::End(TagEnd::BlockQuote(_))
                    | Event::End(TagEnd::List(_)) => {
                        append_section_space(&mut current, in_image);
                    }
                    _ => {}
                }
                output.push(rewrite_event(event, &mut links, &mut images));
                i += 1;
            }
        }
    }

    if let Some((heading, id, section_text)) = current.take() {
        sections.push(finish_section(heading, id, section_text));
    }

    let mut body_html = String::new();
    html::push_html(&mut body_html, output.into_iter());

    Rendered {
        title,
        body_html,
        headings,
        sections,
        links,
        images,
    }
}

fn append_section_text(
    current: &mut Option<(String, String, String)>,
    in_image: usize,
    value: &str,
) {
    if in_image == 0
        && let Some((_, _, section_text)) = current.as_mut()
    {
        section_text.push_str(value);
    }
}

fn append_section_space(current: &mut Option<(String, String, String)>, in_image: usize) {
    if in_image == 0
        && let Some((_, _, section_text)) = current.as_mut()
    {
        section_text.push(' ');
    }
}

fn finish_section(heading: String, id: String, section_text: String) -> Section {
    let text = section_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    Section { heading, id, text }
}

fn rewrite_event(
    event: Event<'_>,
    links: &mut Vec<String>,
    images: &mut Vec<String>,
) -> Event<'static> {
    match event {
        Event::Html(value) | Event::InlineHtml(value) => Event::Text(value.into_static()),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            links.push(dest_url.to_string());
            Event::Start(Tag::Link {
                link_type,
                dest_url: CowStr::from(rewrite_md_target(&dest_url)),
                title: title.into_static(),
                id: id.into_static(),
            })
        }
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            images.push(dest_url.to_string());
            Event::Start(
                Tag::Image {
                    link_type,
                    dest_url,
                    title,
                    id,
                }
                .into_static(),
            )
        }
        other => other.into_static(),
    }
}

fn rewrite_md_target(target: &str) -> String {
    if has_scheme(target) || target.starts_with('#') || target.starts_with('/') {
        return target.to_string();
    }
    let (path, fragment) = split_fragment(target);
    if let Some(stem) = path.strip_suffix(".md") {
        match fragment {
            Some(fragment) => format!("{stem}.html#{fragment}"),
            None => format!("{stem}.html"),
        }
    } else {
        target.to_string()
    }
}

/// The contents sidebar as a `<nav class="toc">` element for the page at source path
/// `current`. Links are relative to that page's folder.
pub fn sidebar_html(entries: &[SummaryEntry], current: &str) -> String {
    let mut html = String::from("<nav class=\"toc\">");
    if entries.is_empty() {
        html.push_str("</nav>");
        return html;
    }

    html.push_str("<ul>");
    let mut previous_level = entries[0].level;
    for (index, entry) in entries.iter().enumerate() {
        if index > 0 {
            if entry.level > previous_level {
                for _ in previous_level..entry.level {
                    html.push_str("<ul>");
                }
            } else {
                html.push_str("</li>");
                if entry.level < previous_level {
                    for _ in entry.level..previous_level {
                        html.push_str("</ul></li>");
                    }
                }
            }
        }

        let href = relative_href(current, &html_path(&entry.path));
        let title = escape_html(&entry.title);
        if entry.path == current {
            html.push_str(&format!(
                "<li><a class=\"current\" href=\"{href}\">{title}</a>"
            ));
        } else {
            html.push_str(&format!("<li><a href=\"{href}\">{title}</a>"));
        }
        previous_level = entry.level;
    }

    html.push_str("</li>");
    for _ in 0..previous_level {
        html.push_str("</ul></li>");
    }
    html.push_str("</ul></nav>");
    html
}

fn relative_href(current: &str, target: &str) -> String {
    let current_parts: Vec<&str> = current.split('/').collect();
    let current_dir = &current_parts[..current_parts.len().saturating_sub(1)];
    let target_parts: Vec<&str> = target.split('/').collect();
    let target_dir = &target_parts[..target_parts.len().saturating_sub(1)];

    let mut common = 0;
    while common < current_dir.len()
        && common < target_dir.len()
        && current_dir[common] == target_dir[common]
    {
        common += 1;
    }

    let mut href = String::new();
    for _ in common..current_dir.len() {
        href.push_str("../");
    }
    href.push_str(&target_parts[common..].join("/"));
    href
}

/// Search records for one page. `page_url` is the page's output path (`drawing/lines.html`).
pub fn index_entries(page_url: &str, page: &Rendered) -> Vec<IndexEntry> {
    let first_id = page.headings.iter().find(|h| h.level == 1).map(|h| &h.id);
    page.sections
        .iter()
        .map(|section| {
            let url = if first_id == Some(&section.id) {
                page_url.to_string()
            } else {
                format!("{page_url}#{}", section.id)
            };
            IndexEntry {
                title: page.title.clone(),
                heading: section.heading.clone(),
                url,
                text: section.text.clone(),
            }
        })
        .collect()
}

/// The search index as a JSON array of `{"title","heading","url","text"}` objects.
pub fn index_json(entries: &[IndexEntry]) -> String {
    let objects: Vec<String> = entries
        .iter()
        .map(|entry| {
            format!(
                "{{\"title\":\"{}\",\"heading\":\"{}\",\"url\":\"{}\",\"text\":\"{}\"}}",
                json_escape(&entry.title),
                json_escape(&entry.heading),
                json_escape(&entry.url),
                json_escape(&entry.text),
            )
        })
        .collect();
    format!("[{}]", objects.join(","))
}

fn json_escape(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn escape_html(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            c => out.push(c),
        }
    }
    out
}

/// Builds the guide in `src` into `out` (layout and checks in the spec).
pub fn build(src: &Path, out: &Path) -> Result<BuildReport, GuideError> {
    let summary = match fs::read_to_string(src.join("SUMMARY.md")) {
        Ok(text) => text,
        Err(error) => {
            return Err(GuideError::Summary {
                line: 0,
                message: format!("cannot read SUMMARY.md: {error}"),
            });
        }
    };
    let entries = parse_summary(&summary)?;

    for entry in &entries {
        if !src.join(&entry.path).exists() {
            return Err(GuideError::MissingPage {
                path: entry.path.clone(),
            });
        }
    }

    let mut pages: Vec<Rendered> = Vec::new();
    for entry in &entries {
        let text = fs::read_to_string(src.join(&entry.path))
            .map_err(|error| GuideError::Io(error.to_string()))?;
        pages.push(render_markdown(&text));
    }

    let mut files: Vec<(String, PathBuf)> = Vec::new();
    collect_files(src, src, &mut files)?;
    let listed: HashSet<String> = entries.iter().map(|entry| entry.path.clone()).collect();
    let mut assets: Vec<String> = Vec::new();
    let mut orphans: Vec<String> = Vec::new();
    for (relative, _) in &files {
        if relative == "SUMMARY.md" {
            continue;
        }
        if relative.ends_with(".md") {
            if !listed.contains(relative) {
                orphans.push(relative.clone());
            }
        } else {
            assets.push(relative.clone());
        }
    }
    assets.sort();
    orphans.sort();

    let mut anchors: HashMap<String, HashSet<String>> = HashMap::new();
    for (entry, page) in entries.iter().zip(&pages).rev() {
        anchors.insert(
            entry.path.clone(),
            page.headings.iter().map(|h| h.id.clone()).collect(),
        );
        for target in &page.links {
            check_link(src, &entry.path, target, &anchors, &listed)?;
        }
        for target in &page.images {
            check_image(src, &entry.path, target)?;
        }
    }

    let mut outputs: Vec<(String, Vec<u8>)> = Vec::new();

    for (relative, path) in &files {
        if relative == "SUMMARY.md" || relative.ends_with(".md") {
            continue;
        }
        let bytes = fs::read(path).map_err(|error| GuideError::Io(error.to_string()))?;
        outputs.push((relative.clone(), bytes));
    }

    let mut page_urls: Vec<String> = Vec::new();
    for (entry, page) in entries.iter().zip(&pages) {
        let output_path = html_path(&entry.path);
        let root = "../".repeat(output_path.matches('/').count());
        let title = if page.title.is_empty() {
            &entry.title
        } else {
            &page.title
        };
        let sidebar = sidebar_html(&entries, &entry.path);
        let html = format!(
            "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{title} - Drydock User Guide</title>\n\
<link rel=\"stylesheet\" href=\"{root}style.css\">\n\
<script src=\"{root}search-index.js\"></script>\n\
<script src=\"{root}search.js\"></script>\n\
</head>\n\
<body data-root=\"{root}\">\n\
<header><input type=\"search\" id=\"search\" placeholder=\"Search the guide\" aria-label=\"Search the guide\"><ul id=\"search-results\"></ul></header>\n\
{sidebar}\n\
<main>\n\
{body}</main>\n\
</body>\n\
</html>\n",
            title = escape_html(title),
            root = root,
            sidebar = sidebar,
            body = page.body_html,
        );
        outputs.push((output_path.clone(), html.into_bytes()));
        page_urls.push(output_path);
    }

    let index_html = if let (Some(first), Some(entry)) = (page_urls.first(), entries.first()) {
        format!(
            "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<meta http-equiv=\"refresh\" content=\"0; url={first}\">\n\
<title>Drydock User Guide</title>\n\
</head>\n\
<body>\n\
<a href=\"{first}\">{title}</a>\n\
</body>\n\
</html>\n",
            first = first,
            title = escape_html(&entry.title),
        )
    } else {
        "<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<title>Drydock User Guide</title>\n\
</head>\n\
<body>\n\
<p>The guide is empty.</p>\n\
</body>\n\
</html>\n"
            .to_string()
    };
    outputs.push(("index.html".to_string(), index_html.into_bytes()));

    let mut records: Vec<IndexEntry> = Vec::new();
    for (entry, page) in entries.iter().zip(&pages) {
        records.extend(index_entries(&html_path(&entry.path), page));
    }
    let json = index_json(&records);
    outputs.push((
        "style.css".to_string(),
        include_str!("../assets/style.css").as_bytes().to_vec(),
    ));
    outputs.push((
        "search.js".to_string(),
        include_str!("../assets/search.js").as_bytes().to_vec(),
    ));
    outputs.push(("search-index.json".to_string(), json.clone().into_bytes()));
    outputs.push((
        "search-index.js".to_string(),
        format!("window.DRYDOCK_SEARCH = {json};\n").into_bytes(),
    ));

    for (relative, bytes) in outputs {
        let path = out.join(&relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| GuideError::Io(error.to_string()))?;
        }
        fs::write(path, bytes).map_err(|error| GuideError::Io(error.to_string()))?;
    }

    Ok(BuildReport {
        pages: entries
            .iter()
            .map(|entry| PagePath {
                source: entry.path.clone(),
                output: html_path(&entry.path),
            })
            .collect(),
        orphans,
        assets,
    })
}

fn check_link(
    src: &Path,
    page: &str,
    target: &str,
    anchors: &HashMap<String, HashSet<String>>,
    listed: &HashSet<String>,
) -> Result<(), GuideError> {
    let broken_link = || GuideError::BrokenLink {
        page: page.to_string(),
        target: target.to_string(),
    };
    let broken_anchor = || GuideError::BrokenAnchor {
        page: page.to_string(),
        target: target.to_string(),
    };

    if has_scheme(target) {
        return Ok(());
    }

    if let Some(fragment) = target.strip_prefix('#') {
        let page_anchors = anchors.get(page).ok_or_else(broken_link)?;
        if page_anchors.contains(fragment) {
            return Ok(());
        }
        return Err(broken_anchor());
    }

    let (path_part, fragment) = split_fragment(target);
    if path_part.starts_with('/') || path_part.is_empty() {
        return Err(broken_link());
    }
    let resolved = resolve_path(page, path_part).ok_or_else(broken_link)?;
    if !src.join(&resolved).exists() {
        return Err(broken_link());
    }
    if path_part.ends_with(".md") {
        if !listed.contains(&resolved) {
            return Err(broken_link());
        }
        if let Some(fragment) = fragment
            && !fragment.is_empty()
            && let Some(page_anchors) = anchors.get(&resolved)
            && !page_anchors.contains(fragment)
        {
            return Err(broken_anchor());
        }
    }
    Ok(())
}

fn check_image(src: &Path, page: &str, target: &str) -> Result<(), GuideError> {
    if has_scheme(target) || target.starts_with('/') {
        return Err(GuideError::ExternalImage {
            page: page.to_string(),
            target: target.to_string(),
        });
    }
    let (path_part, _) = split_fragment(target);
    let resolved = resolve_path(page, path_part).ok_or_else(|| GuideError::MissingImage {
        page: page.to_string(),
        target: target.to_string(),
    })?;
    if !src.join(&resolved).exists() {
        return Err(GuideError::MissingImage {
            page: page.to_string(),
            target: target.to_string(),
        });
    }
    Ok(())
}

fn collect_files(
    base: &Path,
    dir: &Path,
    files: &mut Vec<(String, PathBuf)>,
) -> Result<(), GuideError> {
    let read_dir = fs::read_dir(dir).map_err(|error| GuideError::Io(error.to_string()))?;
    let mut paths: Vec<PathBuf> = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|error| GuideError::Io(error.to_string()))?;
        paths.push(entry.path());
    }
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_files(base, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(base)
                .map_err(|error| GuideError::Io(error.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            files.push((relative, path));
        }
    }
    Ok(())
}

fn resolve_path(page: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = page.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

fn split_fragment(target: &str) -> (&str, Option<&str>) {
    match target.find('#') {
        Some(index) => (&target[..index], Some(&target[index + 1..])),
        None => (target, None),
    }
}

fn has_scheme(target: &str) -> bool {
    let bytes = target.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_alphabetic() {
        return false;
    }
    let mut index = 1;
    while index < bytes.len()
        && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'+' | b'-' | b'.'))
    {
        index += 1;
    }
    index < bytes.len() && bytes[index] == b':'
}

fn html_path(path: &str) -> String {
    if let Some(stem) = path.strip_suffix(".md") {
        format!("{stem}.html")
    } else {
        path.to_string()
    }
}
