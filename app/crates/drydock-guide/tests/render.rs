// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The pure parts: contents file, Markdown rendering, anchors, sidebar, search records.

mod common;

use common::{LINES, WELCOME, toc_hrefs};
use drydock_guide::{
    GuideError, IndexEntry, SummaryEntry, heading_id, index_entries, index_json, parse_summary,
    render_markdown, sidebar_html,
};

fn entry(title: &str, path: &str, level: usize) -> SummaryEntry {
    SummaryEntry {
        title: title.into(),
        path: path.into(),
        level,
    }
}

// ---- heading_id ----

#[test]
fn heading_id_lowercases_and_joins_with_dashes() {
    assert_eq!(heading_id("Drawing Arcs"), "drawing-arcs");
    assert_eq!(heading_id("  Hello, World! "), "hello-world");
    assert_eq!(heading_id("C++ & Rust"), "c-rust");
    assert_eq!(heading_id("Step 2: Push/Pull"), "step-2-push-pull");
}

#[test]
fn heading_id_keeps_unicode_letters_and_never_empty() {
    assert_eq!(heading_id("Ünïcode 3D"), "ünïcode-3d");
    assert_eq!(heading_id("!!!"), "section");
    assert_eq!(heading_id(""), "section");
}

// ---- parse_summary ----

#[test]
fn summary_entries_in_order_with_levels() {
    let text = "# User Guide\n\n- [Welcome](welcome.md)\n  - [Lines](drawing/lines.md)\n    - [Deep](drawing/deep.md)\n  - [Arcs](drawing/arcs.md)\n- [Tips](tips.md)\n";
    let got = parse_summary(text).unwrap();
    assert_eq!(
        got,
        vec![
            entry("Welcome", "welcome.md", 0),
            entry("Lines", "drawing/lines.md", 1),
            entry("Deep", "drawing/deep.md", 2),
            entry("Arcs", "drawing/arcs.md", 1),
            entry("Tips", "tips.md", 0),
        ]
    );
}

#[test]
fn summary_ignores_blank_and_heading_lines() {
    let got = parse_summary("# T\n\n\n## Part\n- [A](a.md)\n\n- [B](b.md)\n").unwrap();
    assert_eq!(got.len(), 2);
}

#[test]
fn summary_empty_is_ok_and_empty() {
    assert_eq!(parse_summary("# Only a title\n").unwrap(), vec![]);
}

#[test]
fn summary_rejects_malformed_lines_with_their_number() {
    let cases = [
        ("- [A](a.md)\nsome prose\n", 2),
        ("- [A](a.md)\n* [B](b.md)\n", 2),
        ("- [A]\n", 1),
        ("- [A](a.txt)\n", 1),
        ("- [A](/abs/a.md)\n", 1),
        ("- [A](../a.md)\n", 1),
        ("- [A](x/../a.md)\n", 1),
        ("- [A](https://example.com/a.md)\n", 1),
        ("- [A](a.md)\n - [B](b.md)\n", 2),
        ("- [A](a.md)\n\t- [B](b.md)\n", 2),
        ("  - [A](a.md)\n", 1),
        ("- [A](a.md)\n    - [B](b.md)\n", 2),
    ];
    for (text, line) in cases {
        match parse_summary(text) {
            Err(GuideError::Summary { line: l, .. }) => assert_eq!(l, line, "{text:?}"),
            other => panic!("{text:?}: {other:?}"),
        }
    }
}

#[test]
fn summary_rejects_a_page_listed_twice() {
    let r = parse_summary("- [A](a.md)\n- [B](a.md)\n");
    assert!(
        matches!(r, Err(GuideError::Summary { line: 2, .. })),
        "{r:?}"
    );
}

// ---- render_markdown ----

#[test]
fn title_is_the_first_level_one_heading() {
    assert_eq!(render_markdown(WELCOME).title, "Welcome to Drydock");
    assert_eq!(render_markdown("Just text.\n\n## Only h2\n").title, "");
}

#[test]
fn headings_get_unique_anchors() {
    let r = render_markdown(WELCOME);
    let got: Vec<(u8, &str, &str)> = r
        .headings
        .iter()
        .map(|h| (h.level, h.text.as_str(), h.id.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            (1, "Welcome to Drydock", "welcome-to-drydock"),
            (2, "Getting started", "getting-started"),
            (2, "Getting started", "getting-started-1"),
        ]
    );
    assert!(
        r.body_html
            .contains("<h1 id=\"welcome-to-drydock\">Welcome to Drydock</h1>")
    );
    assert!(
        r.body_html
            .contains("<h2 id=\"getting-started\">Getting started</h2>")
    );
    assert!(
        r.body_html
            .contains("<h2 id=\"getting-started-1\">Getting started</h2>")
    );
}

#[test]
fn heading_text_ignores_markup() {
    let r = render_markdown("# The `Line` *tool*\n");
    assert_eq!(r.headings[0].text, "The Line tool");
    assert_eq!(r.headings[0].id, "the-line-tool");
    assert_eq!(r.title, "The Line tool");
}

#[test]
fn links_and_images_are_listed_as_written() {
    let r = render_markdown(WELCOME);
    assert_eq!(
        r.links,
        vec!["drawing/lines.md", "drawing/lines.md#drawing-arcs"]
    );
    assert_eq!(r.images, vec!["images/window.png"]);
}

#[test]
fn links_to_markdown_pages_point_to_html() {
    let r = render_markdown(WELCOME);
    assert!(
        r.body_html.contains("href=\"drawing/lines.html\""),
        "{}",
        r.body_html
    );
    assert!(
        r.body_html
            .contains("href=\"drawing/lines.html#drawing-arcs\"")
    );
    assert!(!r.body_html.contains(".md"));
    assert!(r.body_html.contains("src=\"images/window.png\""));
    assert!(r.body_html.contains("<strong>3D printing</strong>"));
}

#[test]
fn external_links_are_left_alone() {
    let r = render_markdown("# T\n\n[x](https://example.com/a.md) [m](mailto:a@b.c) [t](#top)\n");
    assert!(r.body_html.contains("href=\"https://example.com/a.md\""));
    assert!(r.body_html.contains("href=\"mailto:a@b.c\""));
    assert!(r.body_html.contains("href=\"#top\""));
    assert_eq!(r.links.len(), 3);
}

#[test]
fn raw_html_is_shown_as_text() {
    let r = render_markdown("# T\n\n<script>alert(1)</script>\n\nA <b>bold</b> try.\n");
    assert!(!r.body_html.contains("<script"), "{}", r.body_html);
    assert!(!r.body_html.contains("<b>"));
    assert!(r.body_html.contains("&lt;script&gt;"));
}

#[test]
fn tables_and_code_blocks_render() {
    let r = render_markdown("# T\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```\nline 1\nline 2\n```\n");
    assert!(r.body_html.contains("<table>"));
    assert!(r.body_html.contains("<pre>"));
}

#[test]
fn sections_split_at_headings_up_to_level_three() {
    let r = render_markdown(LINES);
    let got: Vec<(&str, &str, &str)> = r
        .sections
        .iter()
        .map(|s| (s.heading.as_str(), s.id.as_str(), s.text.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("Lines", "lines", "Press L to draw a line. Back to Welcome."),
            (
                "Drawing arcs",
                "drawing-arcs",
                "Click twice for a chord, then the bulge. A zebra crossing is not a tool."
            ),
            (
                "Typing lengths",
                "typing-lengths",
                "Type a number and press Enter."
            ),
        ]
    );
}

#[test]
fn deeper_headings_and_code_stay_in_their_section() {
    let r = render_markdown(
        "# A\n\ntext\n\n#### Deep\n\nmore\n\n```\ncode  1\ncode 2\n```\n\n- one\n- two\n",
    );
    assert_eq!(r.sections.len(), 1);
    assert_eq!(r.sections[0].text, "text Deep more code 1 code 2 one two");
    assert_eq!(r.headings.len(), 2);
}

#[test]
fn text_before_the_first_heading_is_not_a_section() {
    let r = render_markdown("intro\n\n# A\n\nbody\n");
    assert_eq!(r.sections.len(), 1);
    assert_eq!(r.sections[0].heading, "A");
}

// ---- sidebar ----

fn three() -> Vec<SummaryEntry> {
    vec![
        entry("Welcome", "welcome.md", 0),
        entry("Lines", "drawing/lines.md", 1),
        entry("Tips & Tricks", "tips.md", 0),
    ]
}

#[test]
fn sidebar_lists_every_page_in_contents_order_relative_to_the_page() {
    let root = sidebar_html(&three(), "welcome.md");
    assert!(root.starts_with("<nav class=\"toc\""), "{root}");
    assert!(root.ends_with("</nav>"));
    assert_eq!(
        toc_hrefs(&root),
        vec!["welcome.html", "drawing/lines.html", "tips.html"]
    );
    let deep = sidebar_html(&three(), "drawing/lines.md");
    assert_eq!(
        toc_hrefs(&deep),
        vec!["../welcome.html", "lines.html", "../tips.html"]
    );
}

#[test]
fn sidebar_marks_the_current_page_and_escapes_titles() {
    let s = sidebar_html(&three(), "drawing/lines.md");
    assert!(
        s.contains("<a class=\"current\" href=\"lines.html\">Lines</a>"),
        "{s}"
    );
    assert!(s.contains("<a href=\"../welcome.html\">Welcome</a>"));
    assert!(s.contains(">Tips &amp; Tricks</a>"));
    assert_eq!(s.matches("class=\"current\"").count(), 1);
}

#[test]
fn sidebar_nests_children_in_lists() {
    let s = sidebar_html(&three(), "welcome.md");
    assert_eq!(s.matches("<ul").count(), s.matches("</ul>").count());
    assert_eq!(s.matches("<li").count(), s.matches("</li>").count());
    let lines = s.find("lines.html").unwrap();
    let welcome = s.find("welcome.html").unwrap();
    let tips = s.find("tips.html").unwrap();
    assert!(
        s[welcome..lines].contains("<ul"),
        "child opens a nested list"
    );
    assert!(
        s[lines..tips].contains("</ul>"),
        "nested list closes before the next top entry"
    );
    // Top-level list plus one nested list.
    assert_eq!(s.matches("<ul").count(), 2);
}

#[test]
fn sidebar_with_no_entries_is_an_empty_nav() {
    let s = sidebar_html(&[], "welcome.md");
    assert!(s.starts_with("<nav class=\"toc\""));
    assert!(toc_hrefs(&s).is_empty());
}

// ---- search records ----

#[test]
fn index_has_one_record_per_section_with_anchor_urls() {
    let r = render_markdown(LINES);
    let got = index_entries("drawing/lines.html", &r);
    let urls: Vec<&str> = got.iter().map(|e| e.url.as_str()).collect();
    assert_eq!(
        urls,
        vec![
            "drawing/lines.html",
            "drawing/lines.html#drawing-arcs",
            "drawing/lines.html#typing-lengths"
        ]
    );
    assert!(got.iter().all(|e| e.title == "Lines"));
    let headings: Vec<&str> = got.iter().map(|e| e.heading.as_str()).collect();
    assert_eq!(headings, vec!["Lines", "Drawing arcs", "Typing lengths"]);
    assert!(got[1].text.contains("zebra"));
}

#[test]
fn index_json_is_compact_and_escaped() {
    let e = IndexEntry {
        title: "T".into(),
        heading: "H".into(),
        url: "u.html#h".into(),
        text: "a \"q\" \\ \n\t\u{1} é".into(),
    };
    assert_eq!(
        index_json(&[e.clone(), e]),
        "[{\"title\":\"T\",\"heading\":\"H\",\"url\":\"u.html#h\",\"text\":\"a \\\"q\\\" \\\\ \\n\\t\\u0001 é\"},\
{\"title\":\"T\",\"heading\":\"H\",\"url\":\"u.html#h\",\"text\":\"a \\\"q\\\" \\\\ \\n\\t\\u0001 é\"}]"
    );
    assert_eq!(index_json(&[]), "[]");
}
