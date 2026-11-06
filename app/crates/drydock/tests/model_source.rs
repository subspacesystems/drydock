// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-model-source-trait: the `ModelSource` interface and the local-file source, on temp
//! folders. See docs/specs/m10-model-source-trait.md.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use drydock::mesh_import::MeshUnit;
use drydock::model_source::*;
use drydock_mesh::print_mesh::PrintObject;
use glam::DVec3;

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A fresh empty folder under the system temp dir.
fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "drydock-model-source-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    fs::read(path).unwrap()
}

const STL: &str = "solid tet
facet normal 0 0 -1
 outer loop
  vertex 0 0 0
  vertex 0 10 0
  vertex 10 0 0
 endloop
endfacet
facet normal 0 -1 0
 outer loop
  vertex 0 0 0
  vertex 10 0 0
  vertex 0 0 10
 endloop
endfacet
facet normal -1 0 0
 outer loop
  vertex 0 0 0
  vertex 0 0 10
  vertex 0 10 0
 endloop
endfacet
facet normal 1 1 1
 outer loop
  vertex 10 0 0
  vertex 0 10 0
  vertex 0 0 10
 endloop
endfacet
endsolid tet
";

fn threemf() -> Vec<u8> {
    let v = |x, y, z| DVec3::new(x, y, z);
    let (a, b, c, d) = (
        v(0., 0., 0.),
        v(10., 0., 0.),
        v(0., 10., 0.),
        v(0., 0., 10.),
    );
    drydock_mesh::threemf_write::encode(&[PrintObject {
        name: "Tet".to_string(),
        color: None,
        triangles: vec![[a, c, b], [a, b, d], [a, d, c], [b, c, d]],
    }])
    .unwrap()
}

/// A folder with one file of each kind, a file that is no model, and a subfolder.
fn library() -> PathBuf {
    let dir = temp_dir("library");
    fs::write(dir.join("Slab.dae"), fixture("slab.dae")).unwrap();
    fs::write(dir.join("slab-native.drydock"), fixture("slab.drydock")).unwrap();
    fs::write(dir.join("Old.modeler"), fixture("slab.modeler")).unwrap();
    fs::write(dir.join("tetra.stl"), STL).unwrap();
    fs::write(dir.join("PRISM.3MF"), threemf()).unwrap();
    fs::write(dir.join("notes.txt"), "not a model").unwrap();
    fs::create_dir(dir.join("nested")).unwrap();
    fs::write(dir.join("nested").join("deep.stl"), STL).unwrap();
    dir
}

fn titles(entries: &[SourceEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.title.as_str()).collect()
}

#[test]
fn format_from_extension_any_case() {
    let f = |p: &str| SourceFormat::from_path(Path::new(p));
    assert_eq!(f("a.dae"), Some(SourceFormat::Dae));
    assert_eq!(f("a.DAE"), Some(SourceFormat::Dae));
    assert_eq!(f("dir/a.stl"), Some(SourceFormat::Stl));
    assert_eq!(f("a.Stl"), Some(SourceFormat::Stl));
    assert_eq!(f("a.3mf"), Some(SourceFormat::ThreeMf));
    assert_eq!(f("a.3MF"), Some(SourceFormat::ThreeMf));
    assert_eq!(f("a.drydock"), Some(SourceFormat::Native));
    assert_eq!(f("a.modeler"), Some(SourceFormat::Native));
    assert_eq!(f("a.txt"), None);
    assert_eq!(f("a"), None);
    assert_eq!(f("dae"), None);
}

#[test]
fn source_has_a_name_and_works_behind_a_trait_object() {
    let dir = library();
    let source: Box<dyn ModelSource> = Box::new(LocalFileSource::new(&dir));
    assert!(!source.name().trim().is_empty());
    assert_eq!(source.list("").unwrap().len(), 5);
    fn needs_send_sync<T: Send + Sync>() {}
    needs_send_sync::<LocalFileSource>();
    assert_eq!(LocalFileSource::new(&dir).dir(), dir.as_path());
}

#[test]
fn list_gives_model_files_only_sorted_by_title() {
    let dir = library();
    let entries = LocalFileSource::new(&dir).list("").unwrap();
    // No notes.txt, no subfolder, nothing from it; case-insensitive order.
    assert_eq!(
        titles(&entries),
        ["Old", "PRISM", "Slab", "slab-native", "tetra"]
    );
    let formats: Vec<_> = entries.iter().map(|e| e.format).collect();
    assert_eq!(
        formats,
        [
            Some(SourceFormat::Native),
            Some(SourceFormat::ThreeMf),
            Some(SourceFormat::Dae),
            Some(SourceFormat::Native),
            Some(SourceFormat::Stl),
        ]
    );
    // Ids are distinct and name the file.
    let mut ids: Vec<_> = entries.iter().map(|e| e.id.clone()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 5);
    assert!(entries[2].id.ends_with("Slab.dae"));
}

#[test]
fn list_filters_by_title_ignoring_case() {
    let dir = library();
    let source = LocalFileSource::new(&dir);
    assert_eq!(
        titles(&source.list("slab").unwrap()),
        ["Slab", "slab-native"]
    );
    assert_eq!(titles(&source.list("SLAB-").unwrap()), ["slab-native"]);
    assert_eq!(titles(&source.list("  prism ").unwrap()), ["PRISM"]);
    assert_eq!(source.list("   ").unwrap().len(), 5);
    assert!(source.list("zzz").unwrap().is_empty());
}

#[test]
fn query_does_not_match_the_extension_or_folder() {
    let dir = library();
    let source = LocalFileSource::new(&dir);
    assert!(source.list("dae").unwrap().is_empty());
    assert!(source.list("drydock-model-source").unwrap().is_empty());
}

#[test]
fn list_of_a_missing_folder_is_an_error_naming_it() {
    let dir = temp_dir("missing").join("nope");
    let err = LocalFileSource::new(&dir).list("").unwrap_err();
    assert!(err.contains("nope"), "{err}");
    assert!(!err.contains('\n'));
}

#[test]
fn list_of_an_empty_folder_is_empty() {
    let dir = temp_dir("empty");
    assert!(LocalFileSource::new(&dir).list("").unwrap().is_empty());
}

#[test]
fn fetch_returns_name_bytes_and_format() {
    let dir = library();
    let source = LocalFileSource::new(&dir);
    let entries = source.list("tetra").unwrap();
    let model = source.fetch(&entries[0]).unwrap();
    assert_eq!(model.file_name, "tetra.stl");
    assert_eq!(model.bytes, STL.as_bytes());
    assert_eq!(model.format, SourceFormat::Stl);
    assert_eq!(model.title(), "tetra");
    assert_eq!(model.origin, Some(dir.join("tetra.stl")));

    let prism = source.fetch(&source.list("prism").unwrap()[0]).unwrap();
    assert_eq!(prism.file_name, "PRISM.3MF");
    assert_eq!(prism.format, SourceFormat::ThreeMf);
    assert_eq!(prism.bytes, threemf());
}

#[test]
fn fetch_a_missing_file_is_an_error_naming_it() {
    let dir = library();
    let source = LocalFileSource::new(&dir);
    let entry = SourceEntry {
        id: dir.join("gone.dae").to_string_lossy().into_owned(),
        title: "gone".into(),
        format: Some(SourceFormat::Dae),
    };
    let err = source.fetch(&entry).unwrap_err();
    assert!(err.contains("gone.dae"), "{err}");
    assert!(!err.contains('\n'));
}

#[test]
fn fetch_a_file_that_is_no_model_is_an_error_naming_it() {
    let dir = library();
    let entry = SourceEntry {
        id: dir.join("notes.txt").to_string_lossy().into_owned(),
        title: "notes".into(),
        format: None,
    };
    let err = LocalFileSource::new(&dir).fetch(&entry).unwrap_err();
    assert!(err.contains("notes.txt"), "{err}");
}

#[test]
fn every_kind_reads_into_a_model() {
    let dir = library();
    let source = LocalFileSource::new(&dir);
    for entry in source.list("").unwrap() {
        let fetched = source.fetch(&entry).unwrap();
        let model = fetched
            .into_model(MeshUnit::Millimeters)
            .unwrap_or_else(|e| panic!("{}: {e}", fetched.file_name));
        assert!(!model.root().is_empty(), "{} is empty", fetched.file_name);
    }
}

#[test]
fn stl_unit_is_applied() {
    let dir = library();
    let source = LocalFileSource::new(&dir);
    let fetched = source.fetch(&source.list("tetra").unwrap()[0]).unwrap();
    let small = fetched.into_model(MeshUnit::Millimeters).unwrap();
    let big = fetched.into_model(MeshUnit::Meters).unwrap();
    let text = |m: &drydock_kernel::model::Model| drydock_kernel::format::to_text(m).unwrap();
    assert_ne!(text(&small), text(&big), "the unit changed nothing");
}

#[test]
fn into_model_without_origin_uses_a_temp_file_and_cleans_up() {
    let before = temp_entries();
    let model = SourceModel {
        file_name: "bytes.stl".into(),
        bytes: STL.as_bytes().to_vec(),
        format: SourceFormat::Stl,
        origin: None,
    };
    assert!(
        !model
            .into_model(MeshUnit::Millimeters)
            .unwrap()
            .root()
            .is_empty()
    );
    let dae = SourceModel {
        file_name: "Slab.dae".into(),
        bytes: fixture("slab.dae"),
        format: SourceFormat::Dae,
        origin: None,
    };
    assert!(
        !dae.into_model(MeshUnit::default())
            .unwrap()
            .root()
            .is_empty()
    );
    assert_eq!(temp_entries(), before, "temp folder left behind");
}

fn temp_entries() -> usize {
    fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("drydock-source-")
        })
        .count()
}

#[test]
fn into_model_error_names_the_file_on_one_line() {
    let model = SourceModel {
        file_name: "broken.3mf".into(),
        bytes: b"this is not a zip".to_vec(),
        format: SourceFormat::ThreeMf,
        origin: None,
    };
    let err = model.into_model(MeshUnit::default()).unwrap_err();
    assert!(err.contains("broken.3mf"), "{err}");
    assert!(!err.contains('\n'));
}
