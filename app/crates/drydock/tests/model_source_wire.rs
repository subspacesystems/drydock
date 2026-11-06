// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-model-source-wire: component and mesh import through a `ModelSource`, shown with a
//! fake in-memory source and with the local-file source. See
//! docs/specs/m10-model-source-wire.md.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use drydock::editor::Editor;
use drydock::mesh_import::MeshUnit;
use drydock::model_source::*;
use drydock_kernel::model::{BodyId, Model};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "drydock-wire-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fixture(name: &str) -> Vec<u8> {
    fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
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

/// A source that holds its models in memory and never touches the disk.
struct MemorySource {
    models: Vec<(String, SourceFormat, Vec<u8>)>,
    fetches: Mutex<usize>,
}

impl MemorySource {
    fn new(models: Vec<(&str, SourceFormat, Vec<u8>)>) -> MemorySource {
        MemorySource {
            models: models
                .into_iter()
                .map(|(name, format, bytes)| (name.to_string(), format, bytes))
                .collect(),
            fetches: Mutex::new(0),
        }
    }

    fn fetches(&self) -> usize {
        *self.fetches.lock().unwrap()
    }

    fn entry(&self, index: usize) -> SourceEntry {
        SourceEntry {
            id: format!("mem:{index}"),
            title: self.models[index].0.clone(),
            format: Some(self.models[index].1),
        }
    }
}

impl ModelSource for MemorySource {
    fn name(&self) -> &str {
        "Memory"
    }

    fn list(&self, _query: &str) -> Result<Vec<SourceEntry>, String> {
        Ok((0..self.models.len()).map(|i| self.entry(i)).collect())
    }

    fn fetch(&self, entry: &SourceEntry) -> Result<SourceModel, String> {
        *self.fetches.lock().unwrap() += 1;
        let index: usize = entry
            .id
            .strip_prefix("mem:")
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| format!("{}: not in this source", entry.title))?;
        let (name, format, bytes) = self
            .models
            .get(index)
            .ok_or_else(|| format!("{}: not in this source", entry.title))?;
        Ok(SourceModel {
            file_name: name.clone(),
            bytes: bytes.clone(),
            format: *format,
            origin: None,
        })
    }
}

fn definition_names(model: &Model) -> Vec<String> {
    model
        .definitions()
        .filter(|(_, info)| !info.group)
        .map(|(_, info)| info.name.clone())
        .collect()
}

fn instance_count(model: &Model) -> usize {
    model.root().instances().count()
}

#[test]
fn a_dae_from_memory_becomes_a_component_named_after_the_file() {
    let source = MemorySource::new(vec![("Chair.dae", SourceFormat::Dae, fixture("slab.dae"))]);
    let (model, name) = fetch_component(&source, &source.entry(0)).unwrap();
    assert_eq!(name, "Chair");
    assert!(!model.root().is_empty());
    assert_eq!(source.fetches(), 1);
}

#[test]
fn a_native_file_from_memory_becomes_a_component() {
    let source = MemorySource::new(vec![(
        "Slab.drydock",
        SourceFormat::Native,
        fixture("slab.drydock"),
    )]);
    let (model, name) = fetch_component(&source, &source.entry(0)).unwrap();
    assert_eq!(name, "Slab");
    assert!(!model.root().is_empty());
}

#[test]
fn an_stl_from_memory_is_a_component_too_and_leaves_no_temp_folder_behind() {
    let source = MemorySource::new(vec![(
        "tetra.stl",
        SourceFormat::Stl,
        STL.as_bytes().to_vec(),
    )]);
    let (model, name) = fetch_component(&source, &source.entry(0)).unwrap();
    assert_eq!(name, "tetra");
    assert!(!model.root().is_empty());
    let leftovers = fs::read_dir(std::env::temp_dir())
        .unwrap()
        .flatten()
        .filter(|e| {
            e.file_name().to_string_lossy().starts_with(&format!(
                "drydock-model-source-work-{}-",
                std::process::id()
            ))
        })
        .count();
    assert_eq!(leftovers, 0);
}

#[test]
fn an_stl_from_memory_imports_as_groups() {
    let source = MemorySource::new(vec![(
        "tetra.stl",
        SourceFormat::Stl,
        STL.as_bytes().to_vec(),
    )]);
    let model = fetch_meshes(&source, &source.entry(0), MeshUnit::Inches).unwrap();
    assert!(!model.root().is_empty());
    let mm = fetch_meshes(&source, &source.entry(0), MeshUnit::Millimeters).unwrap();
    let extent = |m: &Model| {
        let mut max = f64::MIN;
        for (_, instance) in m.root().instances() {
            let body = m.body(BodyId::Definition(instance.definition)).unwrap();
            for (_, v) in body.geometry().vertices() {
                max = max.max(v.position.x);
            }
        }
        max
    };
    assert!((extent(&model) / extent(&mm) - 25.4).abs() < 1e-6);
}

#[test]
fn import_meshes_from_adds_the_groups_to_the_open_model() {
    let source = MemorySource::new(vec![(
        "tetra.stl",
        SourceFormat::Stl,
        STL.as_bytes().to_vec(),
    )]);
    let mut editor = Editor::new();
    let mut model = Model::new();
    let added = import_meshes_from(
        &mut editor,
        &mut model,
        &source,
        &source.entry(0),
        MeshUnit::Millimeters,
    )
    .unwrap();
    assert_eq!(added, 1);
    assert_eq!(instance_count(&model), 1);
}

#[test]
fn import_component_from_adds_a_named_definition() {
    let source = MemorySource::new(vec![("Chair.dae", SourceFormat::Dae, fixture("slab.dae"))]);
    let mut editor = Editor::new();
    let mut model = Model::new();
    let id = import_component_from(&mut editor, &mut model, &source, &source.entry(0)).unwrap();
    assert_eq!(definition_names(&model), vec!["Chair".to_string()]);
    assert!(model.definition(id).is_some());
}

#[test]
fn meshes_refuse_a_non_mesh_format_without_changing_the_model() {
    let source = MemorySource::new(vec![("Chair.dae", SourceFormat::Dae, fixture("slab.dae"))]);
    let err = fetch_meshes(&source, &source.entry(0), MeshUnit::default()).unwrap_err();
    assert_eq!(err, "Chair.dae: not an STL or 3MF file");
    let mut editor = Editor::new();
    let mut model = Model::new();
    let err = import_meshes_from(
        &mut editor,
        &mut model,
        &source,
        &source.entry(0),
        MeshUnit::default(),
    )
    .unwrap_err();
    assert!(err.contains("Chair.dae") && !err.contains('\n'), "{err}");
    assert!(model.root().is_empty());
}

#[test]
fn a_fetch_error_comes_back_unchanged_and_the_model_is_untouched() {
    let source = MemorySource::new(vec![]);
    let missing = SourceEntry {
        id: "mem:7".to_string(),
        title: "Ghost".to_string(),
        format: None,
    };
    assert_eq!(
        fetch_component(&source, &missing).unwrap_err(),
        "Ghost: not in this source"
    );
    assert_eq!(
        fetch_meshes(&source, &missing, MeshUnit::default()).unwrap_err(),
        "Ghost: not in this source"
    );
    let mut editor = Editor::new();
    let mut model = Model::new();
    assert!(import_component_from(&mut editor, &mut model, &source, &missing).is_err());
    assert!(definition_names(&model).is_empty());
    assert!(model.root().is_empty());
}

#[test]
fn unreadable_bytes_give_a_one_line_error_naming_the_file() {
    let source = MemorySource::new(vec![(
        "broken.dae",
        SourceFormat::Dae,
        b"this is not xml".to_vec(),
    )]);
    let err = fetch_component(&source, &source.entry(0)).unwrap_err();
    assert!(err.contains("broken.dae"), "{err}");
    assert!(!err.contains('\n') && !err.contains('\r'), "{err}");
    let mut editor = Editor::new();
    let mut model = Model::new();
    assert!(import_component_from(&mut editor, &mut model, &source, &source.entry(0)).is_err());
    assert!(definition_names(&model).is_empty());
}

#[test]
fn entry_for_path_describes_the_file() {
    let entry = entry_for_path(Path::new("/some/dir/Chair.DAE"));
    assert_eq!(entry.id, "/some/dir/Chair.DAE");
    assert_eq!(entry.title, "Chair");
    assert_eq!(entry.format, Some(SourceFormat::Dae));
    assert_eq!(entry_for_path(Path::new("notes.txt")).format, None);
    assert_eq!(
        local_source_for(Path::new("/some/dir/Chair.dae")).dir(),
        Path::new("/some/dir")
    );
    assert_eq!(
        local_source_for(Path::new("Chair.dae")).dir(),
        Path::new(".")
    );
}

#[test]
fn a_file_on_disk_imports_through_the_local_source() {
    let dir = temp_dir("local");
    let dae = dir.join("slab.dae");
    fs::write(&dae, fixture("slab.dae")).unwrap();
    let (model, name) = fetch_component(&local_source_for(&dae), &entry_for_path(&dae)).unwrap();
    assert_eq!(name, "slab");
    assert!(!model.root().is_empty());

    let stl = dir.join("tetra.STL");
    fs::write(&stl, STL).unwrap();
    let model = fetch_meshes(
        &local_source_for(&stl),
        &entry_for_path(&stl),
        MeshUnit::Millimeters,
    )
    .unwrap();
    assert!(!model.root().is_empty());
}

#[test]
fn the_local_source_reports_an_unsupported_extension_and_a_missing_file() {
    let dir = temp_dir("errors");
    let txt = dir.join("notes.txt");
    fs::write(&txt, "hello").unwrap();
    for result in [
        fetch_component(&local_source_for(&txt), &entry_for_path(&txt)).map(|_| ()),
        fetch_meshes(
            &local_source_for(&txt),
            &entry_for_path(&txt),
            MeshUnit::default(),
        )
        .map(|_| ()),
    ] {
        let err = result.unwrap_err();
        assert!(err.contains("notes.txt") && !err.contains('\n'), "{err}");
    }
    let gone = dir.join("nothing-here.dae");
    let err = fetch_component(&local_source_for(&gone), &entry_for_path(&gone)).unwrap_err();
    assert!(
        err.contains("nothing-here.dae") && !err.contains('\n'),
        "{err}"
    );
}
