// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m8-rename-text: the product's shown name is Drydock. See docs/specs/m8-rename-text.md.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use modeler::chrome::{about_lines, about_text, about_window};
use std::path::{Path, PathBuf};

#[test]
fn about_text_starts_with_the_product_name() {
    let text = about_text();
    assert!(text.starts_with("Drydock "), "{text}");
    assert!(text.contains(env!("CARGO_PKG_VERSION")), "{text}");
}

#[test]
fn about_window_is_titled_and_names_the_company() {
    let mut h = Harness::new_ui(|ui| {
        let mut open = true;
        about_window(ui.ctx(), &mut open);
    });
    h.run();
    assert!(h.query_by_label("About Drydock").is_some(), "window title");
    assert!(h.query_by_label("Subspace Drydock").is_some());
    assert!(h.query_by_label("by Subspace Systems").is_some());
    assert!(h.query_by_label("About modeler").is_none());
}

#[test]
fn no_about_line_says_modeler_apart_from_the_license_identifier() {
    for line in about_lines() {
        if line.starts_with("License:") {
            continue;
        }
        assert!(!line.to_lowercase().contains("modeler"), "{line}");
    }
}

// ---- no shown text says "modeler" ----

/// Every string literal in `source` (comments skipped; char literals, raw strings and
/// escapes handled well enough for this code base), with its 1-based line.
fn literals(source: &str) -> Vec<(usize, String)> {
    let b: Vec<char> = source.chars().collect();
    let (mut i, mut line) = (0, 1);
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && b.get(i + 1) == Some(&'*') {
            let mut depth = 0;
            while i < b.len() {
                if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if b[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
            }
        } else if c == '\'' {
            // A char literal ('"', '\n', 'x') or a lifetime.
            if b.get(i + 1) == Some(&'\\') {
                i += 2;
                while i < b.len() && b[i] != '\'' {
                    i += 1;
                }
                i += 1;
            } else if b.get(i + 2) == Some(&'\'') {
                i += 3;
            } else {
                i += 1;
            }
        } else if c == 'r'
            && matches!(b.get(i + 1), Some('"') | Some('#'))
            && (i == 0 || !(b[i - 1].is_alphanumeric() || b[i - 1] == '_'))
        {
            let mut j = i + 1;
            let mut hashes = 0;
            while b.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if b.get(j) != Some(&'"') {
                i += 1;
                continue;
            }
            j += 1;
            let start = j;
            let close: Vec<char> = std::iter::once('"')
                .chain(std::iter::repeat_n('#', hashes))
                .collect();
            while j < b.len() && b[j..].len() >= close.len() && b[j..j + close.len()] != close[..] {
                j += 1;
            }
            let text: String = b[start..j.min(b.len())].iter().collect();
            line += text.matches('\n').count();
            out.push((line, text));
            i = j + close.len();
        } else if c == '"' {
            let start_line = line;
            let mut j = i + 1;
            let mut text = String::new();
            while j < b.len() && b[j] != '"' {
                if b[j] == '\\' {
                    j += 1;
                    if b.get(j) == Some(&'\n') {
                        // Line continuation: skips the newline and leading blanks.
                        line += 1;
                        j += 1;
                        while matches!(b.get(j), Some(' ') | Some('\t')) {
                            j += 1;
                        }
                        continue;
                    }
                    text.push('\\');
                }
                if b[j] == '\n' {
                    line += 1;
                }
                text.push(b[j]);
                j += 1;
            }
            out.push((start_line, text));
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

fn rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, into);
        } else if path.extension().is_some_and(|e| e == "rs") {
            into.push(path);
        }
    }
}

/// What a literal may still say "modeler" for, until the later rename tasks
/// (`m8-rename-extension`, `-crates`, `-config`) shrink this list: each entry is
/// removed from the literal before the check.
const ALLOWED: &[&str] = &[
    ".modeler",              // the native extension (m8-rename-extension)
    "modeler panels",        // panel layout file header (settings, not shown)
    "modeler toolbars",      // toolbar layout file header (settings, not shown)
    "Usage: modeler",        // the program name in --help (m8-rename-crates)
    "      modeler",         // continuation lines of the same usage text
    "CARGO_BIN_EXE_modeler", // never in src, kept for symmetry
];

/// Literals that are exactly `modeler`: the extension string (m8-rename-extension) and
/// the settings folder name (m8-rename-config).
const EXACT_IN: &[&str] = &["app.rs", "document.rs", "recent.rs"];

#[test]
fn no_shown_string_says_modeler() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    rust_files(&root.join("../modeler-mesh/src"), &mut files);
    assert!(files.len() > 20, "found only {} sources", files.len());
    let mut bad = Vec::new();
    for file in files {
        let name = file.file_name().unwrap().to_str().unwrap().to_owned();
        let source = std::fs::read_to_string(&file).unwrap();
        for (line, lit) in literals(&source) {
            if lit == "modeler" && EXACT_IN.contains(&name.as_str()) {
                continue;
            }
            let mut rest = lit.clone();
            for allowed in ALLOWED {
                rest = rest.replace(allowed, "");
            }
            if rest.contains("modeler") {
                bad.push(format!("{name}:{line}: {lit:?}"));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "lowercase modeler in shown text:\n{}",
        bad.join("\n")
    );
}

#[test]
fn the_scanner_reads_literals() {
    let src = "let a = \"x modeler\"; // \"no\"\nlet b = '\"'; let c = r#\"raw \"q\" modeler\"#;\n/* \"no\" */ let d = \"a\\\"b\";";
    let found: Vec<String> = literals(src).into_iter().map(|(_, t)| t).collect();
    assert_eq!(found, vec!["x modeler", "raw \"q\" modeler", "a\\\"b"]);
}
