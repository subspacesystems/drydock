// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m0-corpus-render: render every corpus model from the iso, top and front views and
//! compare silhouettes with the reference screenshots. Runs only when MODELER_CORPUS
//! points at the corpus (the test oracle sets it).
//!
//! The reference screenshots are cropped by hand, so sizes and margins vary, and their
//! background may be one color or two (sky and ground). Both images are reduced to a
//! silhouette: pixels that are not background, minus thin colored lines (axes), with
//! enclosed regions filled (a face the same color as the background is still inside
//! its outline), then opened to remove hairlines. Silhouettes are cropped to their
//! bounding box and resampled onto the same grid; the score is intersection over
//! union. Resampling hides the aspect ratio, so that is compared separately.
//!
//! MODELER_CORPUS_RENDER_OUT=<dir> keeps the renders and masks.
//! MODELER_CORPUS_RENDER_PROBE=1 scores every model and view (for calibration).

use std::path::{Path, PathBuf};
use std::time::Instant;

use glam::DVec3;
use image::{GrayImage, Luma, RgbaImage};
use modeler_dae::{FlatScene, flatten, import_file};
use modeler_render::{Style, render_headless};
use modeler_view::{Bounds, Camera, Projection, StandardView};

/// Render size.
const SIZE: u32 = 1024;
/// Comparison grid.
const GRID: usize = 128;
/// A pixel is background when every channel is within this of a background color.
const BG_TOLERANCE: i32 = 10;
/// A border color covering at least this share of the border is a background color.
const BG_SHARE: f64 = 0.05;
/// Thin pixels with a channel spread above this are axes or guides, not model.
const AXIS_CHROMA: i32 = 40;
/// Opening radius in pixels per 1024 pixels of image size: removes hairlines.
const HAIRLINE: f64 = 2.0;

/// Render background: a color no corpus material uses.
const RENDER_BG: [f32; 4] = [1.0, 0.0, 1.0, 1.0];

struct Case {
    dir: &'static str,
    view: &'static str,
    min_iou: f64,
}

/// Thresholds calibrated 2026-10-18 against the first renderer; each is a little
/// below the measured score. Front views where the model is edge-on are left out.
const CASES: &[Case] = &[
    Case {
        dir: "01-honeydew-manor",
        view: "iso",
        min_iou: 0.97,
    },
    Case {
        dir: "02-cedar canyon",
        view: "iso",
        min_iou: 0.96,
    },
    Case {
        dir: "02-cedar canyon",
        view: "top",
        min_iou: 0.97,
    },
    Case {
        dir: "04-canning-shed",
        view: "iso",
        min_iou: 0.97,
    },
    Case {
        dir: "04-canning-shed",
        view: "top",
        min_iou: 0.97,
    },
    Case {
        dir: "04-canning-shed",
        view: "front",
        min_iou: 0.97,
    },
    Case {
        dir: "03-shedhouse",
        view: "front",
        min_iou: 0.97,
    },
    Case {
        dir: "05-house",
        view: "iso",
        min_iou: 0.97,
    },
    Case {
        dir: "05-house",
        view: "top",
        min_iou: 0.9,
    },
    Case {
        dir: "06-shop-office",
        view: "iso",
        min_iou: 0.97,
    },
    Case {
        dir: "06-shop-office",
        view: "top",
        min_iou: 0.97,
    },
    Case {
        dir: "07-key-holder",
        view: "iso",
        min_iou: 0.96,
    },
    Case {
        dir: "07-key-holder",
        view: "top",
        min_iou: 0.97,
    },
    Case {
        dir: "07-key-holder",
        view: "front",
        min_iou: 0.97,
    },
    Case {
        dir: "08-desk-and-chair",
        view: "iso",
        min_iou: 0.97,
    },
    Case {
        dir: "08-desk-and-chair",
        view: "front",
        min_iou: 0.97,
    },
];

/// Aspect ratios (width / height) of the two bounding boxes may differ by this factor.
const MAX_ASPECT_RATIO: f64 = 1.1;

fn corpus() -> Option<PathBuf> {
    match std::env::var_os("MODELER_CORPUS") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => {
            eprintln!(
                "MODELER_CORPUS not set; skipping corpus renders (the test oracle runs them)"
            );
            None
        }
    }
}

fn standard_view(name: &str) -> StandardView {
    match name {
        "iso" => StandardView::Iso,
        "top" => StandardView::Top,
        "front" => StandardView::Front,
        other => panic!("unknown view {other}"),
    }
}

fn render_view(scene: &FlatScene, view: &str) -> RgbaImage {
    let aabb = scene.bounds.expect("corpus model is not empty");
    let bounds = Bounds {
        min: DVec3::from_array(aabb.min.map(f64::from)),
        max: DVec3::from_array(aabb.max.map(f64::from)),
    };
    let mut camera = Camera {
        target: DVec3::ZERO,
        distance: 1.0,
        yaw: 0.0,
        pitch: 0.0,
        fov_y: 35f64.to_radians(),
        projection: Projection::Parallel,
    };
    camera.set_standard_view(standard_view(view));
    camera.zoom_extents(&bounds, 1.0);
    let style = Style {
        background: RENDER_BG,
        ..Style::default()
    };
    render_headless(scene, &camera, SIZE, SIZE, &style)
        .expect("headless device (lavapipe) available")
        .0
}

/// A binary image, row-major.
#[derive(Clone)]
struct Mask {
    w: usize,
    h: usize,
    on: Vec<bool>,
}

impl Mask {
    fn to_image(&self) -> GrayImage {
        GrayImage::from_fn(self.w as u32, self.h as u32, |x, y| {
            Luma([if self.on[y as usize * self.w + x as usize] {
                255
            } else {
                0
            }])
        })
    }
}

/// Border colors that cover at least `BG_SHARE` of the border. Empty when the model
/// fills the frame (e.g. a terrain photo).
fn background_colors(img: &RgbaImage) -> Vec<[u8; 3]> {
    let (w, h) = img.dimensions();
    let mut counts = std::collections::HashMap::new();
    let mut total = 0usize;
    let mut add = |x, y| {
        let p = img.get_pixel(x, y).0;
        *counts.entry([p[0], p[1], p[2]]).or_insert(0usize) += 1;
        total += 1;
    };
    for x in 0..w {
        add(x, 0);
        add(x, h - 1);
    }
    for y in 0..h {
        add(0, y);
        add(w - 1, y);
    }
    counts
        .into_iter()
        .filter(|&(_, n)| n as f64 >= BG_SHARE * total as f64)
        .map(|(c, _)| c)
        .collect()
}

/// `None` when the image has no background.
fn silhouette(img: &RgbaImage) -> Option<Mask> {
    let bgs = background_colors(img);
    if bgs.is_empty() {
        return None;
    }
    let (w, h) = (img.width() as usize, img.height() as usize);
    let px: Vec<[i32; 3]> = img
        .pixels()
        .map(|p| [p.0[0].into(), p.0[1].into(), p.0[2].into()])
        .collect();
    let near =
        |p: &[i32; 3], c: &[u8; 3]| (0..3).all(|i| (p[i] - i32::from(c[i])).abs() <= BG_TOLERANCE);
    let raw = Mask {
        w,
        h,
        on: px.iter().map(|p| !bgs.iter().any(|c| near(p, c))).collect(),
    };
    let radius = (HAIRLINE * w.max(h) as f64 / 1024.0).round().max(1.0) as usize;
    // Thin colored pixels are axes and guides: drop them before filling, or they would
    // close off background regions.
    let thick = open(&raw, radius);
    let mut lines_removed = raw.clone();
    for (i, &[r, g, b]) in px.iter().enumerate().take(w * h) {
        let chroma = r.max(g).max(b) - r.min(g).min(b);
        if raw.on[i] && !thick.on[i] && chroma > AXIS_CHROMA {
            lines_removed.on[i] = false;
        }
    }
    Some(open(&fill_holes(&lines_removed), radius))
}

fn open(m: &Mask, r: usize) -> Mask {
    morph(&morph(m, r, true), r, false)
}

/// Erode (`erode`) or dilate with a (2r+1)-square element; outside counts as off.
fn morph(m: &Mask, r: usize, erode: bool) -> Mask {
    let pass = |src: &Mask, horizontal: bool| {
        let (w, h) = (src.w, src.h);
        let (lines, len) = if horizontal { (h, w) } else { (w, h) };
        let at = |line: usize, k: usize| {
            if horizontal {
                line * w + k
            } else {
                k * w + line
            }
        };
        let mut out = vec![false; w * h];
        let mut prefix = vec![0usize; len + 1];
        for line in 0..lines {
            for k in 0..len {
                prefix[k + 1] = prefix[k] + usize::from(src.on[at(line, k)]);
            }
            for k in 0..len {
                let lo = k.saturating_sub(r);
                let hi = (k + r + 1).min(len);
                let count = prefix[hi] - prefix[lo];
                out[at(line, k)] = if erode {
                    // Off-image pixels in the window count as off.
                    k >= r && k + r < len && count == hi - lo
                } else {
                    count > 0
                };
            }
        }
        Mask { w, h, on: out }
    };
    pass(&pass(m, true), false)
}

/// Off regions not connected to the border become on.
fn fill_holes(m: &Mask) -> Mask {
    let (w, h) = (m.w, m.h);
    let mut outside = vec![false; w * h];
    let mut stack: Vec<usize> = Vec::new();
    for x in 0..w {
        stack.push(x);
        stack.push((h - 1) * w + x);
    }
    for y in 0..h {
        stack.push(y * w);
        stack.push(y * w + w - 1);
    }
    while let Some(i) = stack.pop() {
        if outside[i] || m.on[i] {
            continue;
        }
        outside[i] = true;
        let (x, y) = (i % w, i / w);
        if x > 0 {
            stack.push(i - 1);
        }
        if x + 1 < w {
            stack.push(i + 1);
        }
        if y > 0 {
            stack.push(i - w);
        }
        if y + 1 < h {
            stack.push(i + w);
        }
    }
    Mask {
        w,
        h,
        on: outside.iter().map(|o| !o).collect(),
    }
}

/// Crop to the bounding box and resample onto the grid (on when at least half the
/// covered pixels are on). Also returns the box's aspect ratio.
fn normalise(m: &Mask) -> (Vec<bool>, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
    for (i, _) in m.on.iter().enumerate().filter(|(_, on)| **on) {
        let (x, y) = (i % m.w, i / m.w);
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + 1);
        y1 = y1.max(y + 1);
    }
    assert!(x0 < x1, "silhouette is empty");
    let (bw, bh) = (x1 - x0, y1 - y0);
    let span = |g: usize, size: usize, origin: usize| {
        let a = origin + g * size / GRID;
        let b = (origin + (g + 1) * size / GRID).max(a + 1);
        a..b
    };
    let mut grid = vec![false; GRID * GRID];
    for gy in 0..GRID {
        for gx in 0..GRID {
            let (mut on, mut all) = (0usize, 0usize);
            for y in span(gy, bh, y0) {
                for x in span(gx, bw, x0) {
                    all += 1;
                    on += usize::from(m.on[y * m.w + x]);
                }
            }
            grid[gy * GRID + gx] = 2 * on >= all;
        }
    }
    (grid, bw as f64 / bh as f64)
}

fn iou(a: &[bool], b: &[bool]) -> f64 {
    let inter = a.iter().zip(b).filter(|(x, y)| **x && **y).count();
    let union = a.iter().zip(b).filter(|(x, y)| **x || **y).count();
    inter as f64 / union.max(1) as f64
}

struct Score {
    iou: f64,
    aspect_ref: f64,
    aspect_ours: f64,
}

/// `None` when the reference has no background to compare against.
fn score(model_dir: &Path, scene: &FlatScene, view: &str, out: Option<&Path>) -> Option<Score> {
    let reference = image::open(model_dir.join(format!("{view}.png")))
        .expect("reference screenshot")
        .to_rgba8();
    let rendered = render_view(scene, view);
    let ref_mask = silhouette(&reference)?;
    let our_mask = silhouette(&rendered).expect("render has a background");
    if let Some(out) = out {
        let name = model_dir
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace(' ', "_");
        let stem = format!("{name}-{view}");
        rendered
            .save(out.join(format!("{stem}-render.png")))
            .unwrap();
        ref_mask
            .to_image()
            .save(out.join(format!("{stem}-refmask.png")))
            .unwrap();
        our_mask
            .to_image()
            .save(out.join(format!("{stem}-mask.png")))
            .unwrap();
    }
    let (a, aspect_ref) = normalise(&ref_mask);
    let (b, aspect_ours) = normalise(&our_mask);
    Some(Score {
        iou: iou(&a, &b),
        aspect_ref,
        aspect_ours,
    })
}

fn load(model_dir: &Path) -> FlatScene {
    flatten(&import_file(&model_dir.join("model.dae")).expect("corpus model imports"))
}

#[test]
fn corpus_silhouettes_match_the_reference_screenshots() {
    let Some(root) = corpus() else { return };
    let out = std::env::var_os("MODELER_CORPUS_RENDER_OUT").map(PathBuf::from);
    if let Some(out) = &out {
        std::fs::create_dir_all(out).unwrap();
    }
    if std::env::var_os("MODELER_CORPUS_RENDER_PROBE").is_some() {
        let mut dirs: Vec<_> = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.join("model.dae").exists())
            .collect();
        dirs.sort();
        for dir in &dirs {
            let t0 = Instant::now();
            let scene = load(dir);
            for view in ["iso", "top", "front"] {
                if dir.join(format!("{view}.png")).exists() {
                    let Some(s) = score(dir, &scene, view, out.as_deref()) else {
                        eprintln!("PROBE {} {view}: no background", dir.display());
                        continue;
                    };
                    eprintln!(
                        "PROBE {} {view}: iou {:.3} aspect ref {:.3} ours {:.3}",
                        dir.display(),
                        s.iou,
                        s.aspect_ref,
                        s.aspect_ours
                    );
                }
            }
            eprintln!("PROBE {} took {:.1?}", dir.display(), t0.elapsed());
        }
        return;
    }
    let mut failures = Vec::new();
    let mut last: Option<(&str, FlatScene)> = None;
    for case in CASES {
        if last.as_ref().is_none_or(|(dir, _)| *dir != case.dir) {
            last = Some((case.dir, load(&root.join(case.dir))));
        }
        let scene = &last.as_ref().unwrap().1;
        let s = score(&root.join(case.dir), scene, case.view, out.as_deref())
            .expect("reference has a background");
        let what = format!("{} {}", case.dir, case.view);
        eprintln!(
            "{what}: iou {:.3} (min {}), aspect ref {:.3} ours {:.3}",
            s.iou, case.min_iou, s.aspect_ref, s.aspect_ours
        );
        if s.iou < case.min_iou {
            failures.push(format!("{what}: iou {:.3} < {}", s.iou, case.min_iou));
        }
        let ratio = s.aspect_ref.max(s.aspect_ours) / s.aspect_ref.min(s.aspect_ours);
        if ratio > MAX_ASPECT_RATIO {
            failures.push(format!(
                "{what}: aspect {:.3} vs reference {:.3}",
                s.aspect_ours, s.aspect_ref
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "silhouettes differ:\n{}",
        failures.join("\n")
    );
}
