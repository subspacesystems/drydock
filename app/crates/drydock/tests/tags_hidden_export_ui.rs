// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-tags-hidden: File > Export leaves out instances on a hidden tag
//! (`docs/specs/m10-tags-hidden.md`).

use std::path::PathBuf;

use drydock::mesh_export::*;
use drydock_kernel::model::*;
use glam::{DMat4, DVec3};

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("m10-tags-hidden-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A model with one unit cube group at the root.
fn one_cube() -> (Model, InstanceId) {
    let mut m = Model::new();
    let d = m.add_definition(
        DefinitionInfo {
            name: "Cube".into(),
            group: true,
        },
        &mut ModelEdit::new(),
    );
    let c = |i: usize| DVec3::new((i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64);
    m.edit_geometry(BodyId::Definition(d), &mut ModelEdit::new(), |ctx, edit| {
        for a in 0..8usize {
            for bit in [1usize, 2, 4] {
                if a & bit == 0 {
                    ctx.add_edge(c(a), c(a | bit), edit).unwrap();
                }
            }
        }
    })
    .unwrap();
    let i = m
        .add_instance(
            BodyId::Root,
            Instance {
                definition: d,
                transform: DMat4::IDENTITY,
                name: "Cube".into(),
                material: None,
            },
            &mut ModelEdit::new(),
        )
        .unwrap();
    (m, i)
}

#[test]
fn a_hidden_cube_is_not_exported_and_no_file_is_written() {
    let dir = scratch("hidden");
    let (mut m, i) = one_cube();
    let tag = m.add_tag("Hidden", &mut ModelEdit::new()).unwrap();
    m.assign_tag(i, Some(tag), &mut ModelEdit::new()).unwrap();

    let ok_path = dir.join("shown.stl");
    let report = export(&m, &[], &ExportOptions::new(ExportFormat::Stl), &ok_path).unwrap();
    assert_eq!(report.objects, 1);

    m.set_tag_visible(tag, false, &mut ModelEdit::new())
        .unwrap();
    for format in [ExportFormat::ThreeMf, ExportFormat::Stl] {
        let path = dir.join(format!("hidden.{}", format.extension()));
        let error = export(&m, &[], &ExportOptions::new(format), &path).unwrap_err();
        assert!(error.starts_with("Nothing to export"), "{error}");
        assert!(!path.exists());
    }

    m.set_tag_visible(tag, true, &mut ModelEdit::new()).unwrap();
    let path = dir.join("again.3mf");
    let report = export(&m, &[], &ExportOptions::new(ExportFormat::ThreeMf), &path).unwrap();
    assert_eq!(report.objects, 1);
}

#[test]
fn can_export_needs_a_visible_instance() {
    let (mut m, i) = one_cube();
    assert!(can_export(&m));
    let tag = m.add_tag("Hidden", &mut ModelEdit::new()).unwrap();
    m.assign_tag(i, Some(tag), &mut ModelEdit::new()).unwrap();
    assert!(can_export(&m));
    m.set_tag_visible(tag, false, &mut ModelEdit::new())
        .unwrap();
    assert!(!can_export(&m));
    m.set_tag_visible(tag, true, &mut ModelEdit::new()).unwrap();
    assert!(can_export(&m));
}
