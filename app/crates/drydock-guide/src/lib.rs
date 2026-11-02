// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! User guide generator: Markdown pages to static HTML with an offline search index.
//!
//! Behaviour: `docs/specs/m8-guide-build.md`. Pure functions first ([`parse_summary`],
//! [`render_markdown`], [`sidebar_html`], [`index_entries`], [`index_json`]), then [`build`]
//! ties them to the file system.

use std::fmt;
use std::path::Path;

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

/// What [`build`] wrote.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildReport {
    /// Output paths of the pages, relative to the output folder, in contents order.
    pub pages: Vec<String>,
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
        let _ = f;
        unimplemented!("m8-guide-build")
    }
}

impl std::error::Error for GuideError {}

/// The anchor for a heading text, before duplicates are numbered: lower case, letters and
/// digits kept, every other run of characters becomes one `-`, no leading or trailing `-`;
/// `section` when nothing is left.
pub fn heading_id(text: &str) -> String {
    let _ = text;
    unimplemented!("m8-guide-build")
}

/// Parses `SUMMARY.md` (format in the spec).
pub fn parse_summary(text: &str) -> Result<Vec<SummaryEntry>, GuideError> {
    let _ = text;
    unimplemented!("m8-guide-build")
}

/// Renders one Markdown page (rules in the spec).
pub fn render_markdown(markdown: &str) -> Rendered {
    let _ = markdown;
    unimplemented!("m8-guide-build")
}

/// The contents sidebar as a `<nav class="toc">` element for the page at source path
/// `current`. Links are relative to that page's folder.
pub fn sidebar_html(entries: &[SummaryEntry], current: &str) -> String {
    let _ = (entries, current);
    unimplemented!("m8-guide-build")
}

/// Search records for one page. `page_url` is the page's output path (`drawing/lines.html`).
pub fn index_entries(page_url: &str, page: &Rendered) -> Vec<IndexEntry> {
    let _ = (page_url, page);
    unimplemented!("m8-guide-build")
}

/// The search index as a JSON array of `{"title","heading","url","text"}` objects.
pub fn index_json(entries: &[IndexEntry]) -> String {
    let _ = entries;
    unimplemented!("m8-guide-build")
}

/// Builds the guide in `src` into `out` (layout and checks in the spec).
pub fn build(src: &Path, out: &Path) -> Result<BuildReport, GuideError> {
    let _ = (src, out);
    unimplemented!("m8-guide-build")
}
