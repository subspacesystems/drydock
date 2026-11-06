// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m10-import-component-tolerant: the editor imports a source that already has geometry
//! problems, and a refused import says why on one line.
//! See docs/specs/m10-import-component-tolerant.md.

use drydock::editor::Editor;
use drydock::model_source::import_failure_message;
use drydock_kernel::copy::CopyError;
use drydock_kernel::model::*;
use drydock_kernel::topology::{EdgeId, Violation};
use glam::DVec3;

/// A model whose root holds two edges crossing at their middles (raw inserts), so it
/// already has one `EdgesIntersect`, plus a clean square drawn with the normal tools.
fn crossing_model() -> Model {
    let mut model = Model::new();
    let mut log = ModelEdit::new();
    model
        .edit_geometry(BodyId::Root, &mut log, |ctx, edit| {
            let a = ctx.insert_vertex(DVec3::new(0.0, 0.0, 0.0), edit);
            let b = ctx.insert_vertex(DVec3::new(2.0, 2.0, 0.0), edit);
            let c = ctx.insert_vertex(DVec3::new(0.0, 2.0, 0.0), edit);
            let d = ctx.insert_vertex(DVec3::new(2.0, 0.0, 0.0), edit);
            ctx.insert_edge(a, b, edit).unwrap();
            ctx.insert_edge(c, d, edit).unwrap();
            let pts = [
                DVec3::new(5.0, 0.0, 0.0),
                DVec3::new(6.0, 0.0, 0.0),
                DVec3::new(6.0, 1.0, 0.0),
                DVec3::new(5.0, 1.0, 0.0),
            ];
            for i in 0..4 {
                ctx.add_edge(pts[i], pts[(i + 1) % 4], edit).unwrap();
            }
        })
        .unwrap();
    model
}

fn crossing(n: u64) -> ModelViolation {
    ModelViolation::Geometry(
        BodyId::Root,
        Violation::EdgesIntersect(EdgeId(n), EdgeId(n + 1)),
    )
}

#[test]
fn the_editor_imports_a_source_with_crossing_edges_and_starts_placing_it() {
    let source = crossing_model();
    assert_eq!(source.validate().len(), 1);
    let mut editor = Editor::new();
    let mut model = Model::new();
    let id = editor
        .import_component(&mut model, &source, "model")
        .expect("the source's own problem doesn't stop the import");
    assert_eq!(model.definition(id).unwrap().name, "model");
    assert_eq!(editor.placing(), Some(id));
    assert_eq!(editor.undo_stack().undo_count(), 1);
    assert_eq!(editor.undo_stack().undo_label(), Some("Import Component"));
    assert_eq!(model.validate().len(), 1);
    assert!(
        editor
            .message()
            .is_none_or(|m| !m.contains("Import failed")),
        "{:?}",
        editor.message()
    );
}

#[test]
fn one_added_problem_is_counted_in_the_singular() {
    assert_eq!(
        import_failure_message("key-holder", &CopyError::Invalid(vec![crossing(0)])),
        "Can't import key-holder: it would add 1 problem to the model"
    );
}

#[test]
fn several_added_problems_are_counted() {
    let added = vec![crossing(0), crossing(2), crossing(4)];
    assert_eq!(
        import_failure_message("Shed house", &CopyError::Invalid(added)),
        "Can't import Shed house: it would add 3 problems to the model"
    );
}

#[test]
fn other_refusals_say_the_geometry_could_not_be_copied() {
    for error in [
        CopyError::BadMatrix,
        CopyError::Model(ModelError::Cycle),
        CopyError::Model(ModelError::MissingDefinition(DefinitionId(7))),
    ] {
        assert_eq!(
            import_failure_message("desk", &error),
            "Can't import desk: its geometry couldn't be copied",
            "{error:?}"
        );
    }
}

#[test]
fn an_empty_source_is_nothing_to_import() {
    assert_eq!(
        import_failure_message("desk", &CopyError::Empty),
        "Nothing to import"
    );
}

#[test]
fn the_name_is_trimmed_and_a_blank_one_reads_component() {
    assert_eq!(
        import_failure_message("  desk  ", &CopyError::Invalid(vec![crossing(0)])),
        "Can't import desk: it would add 1 problem to the model"
    );
    assert_eq!(
        import_failure_message("   ", &CopyError::BadMatrix),
        "Can't import Component: its geometry couldn't be copied"
    );
}

#[test]
fn the_message_is_one_line() {
    let message = import_failure_message("two\nlines\r", &CopyError::BadMatrix);
    assert!(
        !message.contains('\n') && !message.contains('\r'),
        "{message:?}"
    );
    assert!(message.starts_with("Can't import two lines"), "{message:?}");
}
