// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The search box must work: its script has to run after the box exists, and result links must
//! resolve from pages in subfolders. (Node isn't available in the worker image, so the script
//! is checked as text; sign-off types a word in a real browser.)

mod common;

use common::Guide;

const PAGES: [&str; 2] = ["welcome.html", "drawing/lines.html"];

#[test]
fn search_script_waits_for_the_page_on_every_page() {
    let g = Guide::sample();
    g.build().unwrap();
    for page in PAGES {
        let html = g.out_text(page);
        let index = html.find("search-index.js\"").expect("index script tag");
        let script = html.find("search.js\"").expect("search script tag");
        assert!(index < script, "{page}: index must load before search.js");
        let tag_start = html[..script].rfind("<script").unwrap();
        let deferred = html[tag_start..script].contains(" defer ");
        let input = html.find("id=\"search\"").expect("search input");
        assert!(
            deferred || script > input,
            "{page}: search.js runs before the search box exists"
        );
        let index_start = html[..index].rfind("<script").unwrap();
        assert!(
            html[index_start..index].contains(" defer ") == deferred,
            "{page}: both scripts use the same loading mode, so their order holds"
        );
    }
}

#[test]
fn search_js_sets_itself_up_once_the_document_is_ready() {
    let g = Guide::sample();
    g.build().unwrap();
    let js = g.out_text("search.js");
    assert!(js.contains("document.readyState === \"loading\""), "{js}");
    assert!(js.contains("addEventListener(\"DOMContentLoaded\""), "{js}");
    // The lookup of the box happens inside the setup, not at the top of the script.
    let setup = js.find("DOMContentLoaded").unwrap();
    let lookup = js.find("getElementById(\"search\")").unwrap();
    let function_before_lookup = js[..lookup].contains("function setup");
    assert!(function_before_lookup && setup > 0, "{js}");
}

#[test]
fn result_links_are_built_from_the_page_root() {
    let g = Guide::sample();
    g.build().unwrap();
    let js = g.out_text("search.js");
    assert!(js.contains("dataset.root"), "{js}");
    // Index urls are relative to the guide root, and the deep page's root is "../".
    assert!(
        g.out_text("search-index.json")
            .contains("\"url\":\"drawing/lines.html#drawing-arcs\"")
    );
    assert!(
        g.out_text("drawing/lines.html")
            .contains("<body data-root=\"../\">")
    );
}
