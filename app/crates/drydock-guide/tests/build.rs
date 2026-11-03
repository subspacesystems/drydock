// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The whole build on the two sample pages: output layout, page frame, offline search files,
//! and every way a build must fail.

mod common;

use common::{Guide, PNG, sample_files, toc_hrefs, tree};
use drydock_guide::GuideError;

#[test]
fn sample_builds_with_the_expected_files() {
    let g = Guide::sample();
    let report = g.build().unwrap();
    assert_eq!(report.pages, vec!["welcome.html", "drawing/lines.html"]);
    assert_eq!(report.assets, vec!["images/window.png"]);
    assert!(report.orphans.is_empty());
    let files: Vec<String> = tree(&g.out()).into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        files,
        vec![
            "drawing/lines.html",
            "images/window.png",
            "index.html",
            "search-index.js",
            "search-index.json",
            "search.js",
            "style.css",
            "welcome.html",
        ]
    );
    assert_eq!(
        std::fs::read(g.out().join("images/window.png")).unwrap(),
        PNG
    );
}

#[test]
fn page_frame_links_assets_relative_to_the_page() {
    let g = Guide::sample();
    g.build().unwrap();
    let root = g.out_text("welcome.html");
    assert!(root.starts_with("<!DOCTYPE html>"), "{root}");
    assert!(root.contains("<html lang=\"en\">"));
    assert!(root.contains("<title>Welcome to Drydock - Drydock User Guide</title>"));
    assert!(root.contains("<link rel=\"stylesheet\" href=\"style.css\">"));
    assert!(root.contains("<script defer src=\"search-index.js\"></script>"));
    assert!(root.contains("<script defer src=\"search.js\"></script>"));
    assert!(root.contains("<body data-root=\"\">"));
    assert!(root.contains("<input type=\"search\" id=\"search\""));
    assert!(root.contains("<ul id=\"search-results\"></ul>"));
    assert!(root.contains("<main>"));
    assert!(root.contains("<h1 id=\"welcome-to-drydock\">"));
    let deep = g.out_text("drawing/lines.html");
    assert!(deep.contains("<title>Lines - Drydock User Guide</title>"));
    assert!(deep.contains("<link rel=\"stylesheet\" href=\"../style.css\">"));
    assert!(deep.contains("<script defer src=\"../search-index.js\"></script>"));
    assert!(deep.contains("<script defer src=\"../search.js\"></script>"));
    assert!(deep.contains("<body data-root=\"../\">"));
}

#[test]
fn every_page_carries_the_sidebar_in_contents_order() {
    let g = Guide::sample();
    g.build().unwrap();
    assert_eq!(
        toc_hrefs(&g.out_text("welcome.html")),
        vec!["welcome.html", "drawing/lines.html"]
    );
    assert_eq!(
        toc_hrefs(&g.out_text("drawing/lines.html")),
        vec!["../welcome.html", "lines.html"]
    );
    assert!(
        g.out_text("drawing/lines.html")
            .contains("<a class=\"current\" href=\"lines.html\">Lines</a>")
    );
}

#[test]
fn body_links_between_pages_resolve_in_the_output() {
    let g = Guide::sample();
    g.build().unwrap();
    let root = g.out_text("welcome.html");
    assert!(root.contains("href=\"drawing/lines.html#drawing-arcs\""));
    assert!(root.contains("<h2 id=\"getting-started-1\">"));
    let deep = g.out_text("drawing/lines.html");
    assert!(deep.contains("href=\"../welcome.html#getting-started\""));
    assert!(deep.contains("src=\"../images/window.png\""));
}

#[test]
fn page_without_a_heading_is_titled_from_the_contents() {
    let g = Guide::sample_with("welcome.md", "No heading here.\n");
    g.build().unwrap();
    assert!(
        g.out_text("welcome.html")
            .contains("<title>Welcome - Drydock User Guide</title>")
    );
}

#[test]
fn index_page_redirects_to_the_first_page() {
    let g = Guide::sample();
    g.build().unwrap();
    let i = g.out_text("index.html");
    assert!(i.contains("http-equiv=\"refresh\""));
    assert!(i.contains("url=welcome.html"));
    assert!(i.contains("href=\"welcome.html\""));
}

#[test]
fn search_files_agree_and_work_from_a_file_url() {
    let g = Guide::sample();
    g.build().unwrap();
    let json = g.out_text("search-index.json");
    let js = g.out_text("search-index.js");
    assert_eq!(js, format!("window.DRYDOCK_SEARCH = {json};\n"));
    assert!(json.starts_with('[') && json.ends_with(']'));
    assert!(json.contains("\"url\":\"drawing/lines.html#drawing-arcs\""));
    assert!(json.contains("\"url\":\"welcome.html#getting-started-1\""));
    assert!(json.contains("zebra crossing"));
    assert!(json.contains("\"title\":\"Welcome to Drydock\""));
    assert!(
        !json.contains("<p>") && !json.contains("**"),
        "plain text only"
    );
    // 3 sections in lines.md, 3 in welcome.md.
    assert_eq!(json.matches("\"heading\":").count(), 6);
}

#[test]
fn nothing_is_fetched_from_elsewhere() {
    let g = Guide::sample();
    g.build().unwrap();
    for f in [
        "welcome.html",
        "drawing/lines.html",
        "index.html",
        "search.js",
        "style.css",
    ] {
        let t = g.out_text(f);
        assert!(!t.contains("http://") && !t.contains("https://"), "{f}");
    }
    let js = g.out_text("search.js");
    for banned in ["fetch(", "XMLHttpRequest", "import(", "WebSocket"] {
        assert!(!js.contains(banned), "search.js uses {banned}");
    }
    assert!(js.contains("DRYDOCK_SEARCH"));
    assert!(js.contains("search-results"));
}

#[test]
fn build_is_repeatable_and_deterministic() {
    let a = Guide::sample();
    let b = Guide::sample();
    a.build().unwrap();
    b.build().unwrap();
    a.build().unwrap();
    assert_eq!(tree(&a.out()), tree(&b.out()));
}

#[test]
fn sources_and_the_contents_file_are_not_copied() {
    let g = Guide::sample();
    g.build().unwrap();
    assert!(!g.out().join("SUMMARY.md").exists());
    assert!(!g.out().join("welcome.md").exists());
}

#[test]
fn pages_missing_from_the_contents_are_reported_not_built() {
    let g = Guide::sample();
    g.write("extra.md", b"# Extra\n\n[gone](nowhere.md)\n");
    let report = g.build().unwrap();
    assert_eq!(report.orphans, vec!["extra.md"]);
    assert!(!g.out().join("extra.html").exists());
}

#[test]
fn links_to_other_files_and_external_places_are_fine() {
    let text =
        "# W\n\n[img](images/window.png) [web](https://example.com/x) [mail](mailto:a@b.c)\n";
    let g = Guide::sample_with("welcome.md", text);
    g.build().unwrap();
    assert!(
        g.out_text("welcome.html")
            .contains("href=\"https://example.com/x\"")
    );
}

// ---- failures ----

fn fails(g: &Guide) -> GuideError {
    g.build().expect_err("build should fail")
}

#[test]
fn missing_page_fails() {
    let mut files = sample_files();
    files.retain(|(p, _)| *p != "drawing/lines.md");
    let g = Guide::new(&files);
    assert_eq!(
        fails(&g),
        GuideError::MissingPage {
            path: "drawing/lines.md".into()
        }
    );
}

#[test]
fn missing_or_bad_contents_file_fails() {
    let mut files = sample_files();
    files.retain(|(p, _)| *p != "SUMMARY.md");
    assert!(matches!(
        fails(&Guide::new(&files)),
        GuideError::Summary { .. }
    ));
    let g = Guide::sample_with("SUMMARY.md", "- [Welcome](welcome.md)\nstray text\n");
    assert!(matches!(fails(&g), GuideError::Summary { line: 2, .. }));
}

#[test]
fn broken_page_link_fails() {
    let g = Guide::sample_with("welcome.md", "# W\n\n[x](missing.md)\n");
    assert_eq!(
        fails(&g),
        GuideError::BrokenLink {
            page: "welcome.md".into(),
            target: "missing.md".into()
        }
    );
}

#[test]
fn link_to_a_page_outside_the_contents_fails() {
    let g = Guide::sample_with("welcome.md", "# W\n\n[x](extra.md)\n");
    g.write("extra.md", b"# Extra\n");
    assert!(matches!(fails(&g), GuideError::BrokenLink { .. }));
}

#[test]
fn link_leaving_the_guide_folder_fails() {
    let g = Guide::sample_with("welcome.md", "# W\n\n[x](../outside.md)\n");
    assert_eq!(
        fails(&g),
        GuideError::BrokenLink {
            page: "welcome.md".into(),
            target: "../outside.md".into()
        }
    );
}

#[test]
fn link_resolves_from_the_page_folder_not_the_root() {
    // images/window.png exists at the root, not under drawing/.
    let g = Guide::sample_with("drawing/lines.md", "# L\n\n[x](images/window.png)\n");
    assert!(matches!(fails(&g), GuideError::BrokenLink { .. }));
}

#[test]
fn unknown_anchor_fails() {
    let g = Guide::sample_with("welcome.md", "# W\n\n[x](drawing/lines.md#nope)\n");
    assert_eq!(
        fails(&g),
        GuideError::BrokenAnchor {
            page: "welcome.md".into(),
            target: "drawing/lines.md#nope".into()
        }
    );
    let g = Guide::sample_with("welcome.md", "# W\n\n[x](#nope)\n");
    assert_eq!(
        fails(&g),
        GuideError::BrokenAnchor {
            page: "welcome.md".into(),
            target: "#nope".into()
        }
    );
}

#[test]
fn anchor_on_the_same_page_passes() {
    let g = Guide::sample_with(
        "welcome.md",
        "# W\n\n## Two words\n\n[x](#two-words) [top](#w)\n",
    );
    g.build().unwrap();
}

#[test]
fn missing_image_fails() {
    let g = Guide::sample_with("welcome.md", "# W\n\n![x](images/none.png)\n");
    assert_eq!(
        fails(&g),
        GuideError::MissingImage {
            page: "welcome.md".into(),
            target: "images/none.png".into()
        }
    );
}

#[test]
fn image_path_is_relative_to_its_page() {
    let g = Guide::sample_with("drawing/lines.md", "# L\n\n![x](images/window.png)\n");
    assert!(matches!(fails(&g), GuideError::MissingImage { .. }));
}

#[test]
fn images_from_elsewhere_fail() {
    for target in [
        "https://example.com/a.png",
        "http://example.com/a.png",
        "/images/window.png",
    ] {
        let g = Guide::sample_with("welcome.md", &format!("# W\n\n![x]({target})\n"));
        assert_eq!(
            fails(&g),
            GuideError::ExternalImage {
                page: "welcome.md".into(),
                target: target.into()
            }
        );
    }
}

#[test]
fn errors_name_the_page_in_their_message() {
    let g = Guide::sample_with("welcome.md", "# W\n\n[x](missing.md)\n");
    let msg = fails(&g).to_string();
    assert!(
        msg.contains("welcome.md") && msg.contains("missing.md"),
        "{msg}"
    );
}

// ---- command line ----

fn run(args: &[&std::path::Path]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_drydock-guide"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn command_builds_the_guide() {
    let g = Guide::sample();
    let out = g.out();
    let o = run(&[&g.src(), &out]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    assert!(out.join("index.html").exists());
    assert!(out.join("drawing/lines.html").exists());
}

#[test]
fn command_exits_1_and_names_the_problem_when_the_build_fails() {
    let g = Guide::sample_with("welcome.md", "# W\n\n[x](missing.md)\n");
    let o = run(&[&g.src(), &g.out()]);
    assert_eq!(o.status.code(), Some(1));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.starts_with("drydock-guide: "), "{err}");
    assert!(err.contains("missing.md"), "{err}");
}

#[test]
fn command_without_two_arguments_prints_usage_and_exits_2() {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_drydock-guide"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("usage: drydock-guide SRC_DIR OUT_DIR"));
}
