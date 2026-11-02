// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Sample guide and a throw-away folder for each test.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub const SUMMARY: &str =
    "# User Guide\n\n- [Welcome](welcome.md)\n  - [Lines](drawing/lines.md)\n";

pub const WELCOME: &str = "# Welcome to Drydock

Drydock is a modeler for **3D printing**. See [drawing lines](drawing/lines.md) or the [arc section](drawing/lines.md#drawing-arcs).

## Getting started

Open the *File* menu.

## Getting started

Same heading again.

![The main window](images/window.png)
";

pub const LINES: &str = "# Lines

Press `L` to draw a line. Back to [Welcome](../welcome.md#getting-started).

## Drawing arcs

Click twice for a chord, then the bulge. A zebra crossing is not a tool.

### Typing lengths

Type a number and press Enter.

![A line](../images/window.png)
";

pub const PNG: &[u8] = b"\x89PNG not really";

/// The two sample pages and an image, as (path, bytes).
pub fn sample_files() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("SUMMARY.md", SUMMARY.as_bytes().to_vec()),
        ("welcome.md", WELCOME.as_bytes().to_vec()),
        ("drawing/lines.md", LINES.as_bytes().to_vec()),
        ("images/window.png", PNG.to_vec()),
    ]
}

/// A fresh folder with `src/` filled from `files`; removed on drop.
pub struct Guide {
    root: PathBuf,
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

impl Guide {
    pub fn new(files: &[(&str, Vec<u8>)]) -> Guide {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("drydock-guide-test-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        let g = Guide { root };
        for (path, bytes) in files {
            g.write(path, bytes);
        }
        g
    }

    pub fn sample() -> Guide {
        Guide::new(&sample_files())
    }

    /// The sample with one file replaced (or added).
    pub fn sample_with(path: &str, text: &str) -> Guide {
        let mut files: Vec<(&str, Vec<u8>)> = sample_files()
            .into_iter()
            .filter(|(p, _)| *p != path)
            .collect();
        files.push((path, text.as_bytes().to_vec()));
        Guide::new(&files)
    }

    pub fn write(&self, path: &str, bytes: &[u8]) {
        let p = self.src().join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    pub fn src(&self) -> PathBuf {
        self.root.join("src")
    }

    pub fn out(&self) -> PathBuf {
        self.root.join("out")
    }

    pub fn out_text(&self, path: &str) -> String {
        std::fs::read_to_string(self.out().join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    pub fn build(&self) -> Result<drydock_guide::BuildReport, drydock_guide::GuideError> {
        drydock_guide::build(&self.src(), &self.out())
    }
}

impl Drop for Guide {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Every file under `dir` as sorted (relative path, bytes).
pub fn tree(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(base: &Path, dir: &Path, acc: &mut Vec<(String, Vec<u8>)>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, acc);
            } else {
                let rel = p
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                acc.push((rel, std::fs::read(&p).unwrap()));
            }
        }
    }
    let mut acc = Vec::new();
    walk(dir, dir, &mut acc);
    acc.sort();
    acc
}

/// The `href` values inside the first `<nav class="toc">` element, in order.
pub fn toc_hrefs(html: &str) -> Vec<String> {
    let start = html.find("<nav class=\"toc\"").expect("toc nav");
    let end = start + html[start..].find("</nav>").expect("end of toc nav");
    let nav = &html[start..end];
    let mut out = Vec::new();
    let mut rest = nav;
    while let Some(i) = rest.find("href=\"") {
        rest = &rest[i + 6..];
        let j = rest.find('"').unwrap();
        out.push(rest[..j].to_string());
        rest = &rest[j..];
    }
    out
}
