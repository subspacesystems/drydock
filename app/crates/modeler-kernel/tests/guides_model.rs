// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m3-guides-model: guides in the model (`docs/specs/m3-guides-model.md`).

use glam::DVec3;
use modeler_kernel::model::*;
use modeler_kernel::tape::{Guide, GuideId};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn line(px: f64, dir: DVec3) -> Guide {
    Guide::Line {
        point: v(px, 0.0, 0.0),
        dir,
    }
}

fn pt(x: f64) -> Guide {
    Guide::Point(v(x, 1.0, 2.0))
}

fn with_definition() -> (Model, DefinitionId) {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = m.add_definition(
        DefinitionInfo {
            name: "d".into(),
            group: false,
        },
        &mut log,
    );
    (m, d)
}

fn guides(m: &Model, body: BodyId) -> Vec<(GuideId, Guide)> {
    m.body(body)
        .unwrap()
        .guides()
        .iter()
        .map(|(id, g)| (id, *g))
        .collect()
}

#[test]
fn add_guide_stores_and_logs_one_record() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let g = line(1.0, DVec3::Z);
    let id = m.add_guide(BodyId::Root, g, &mut log).unwrap();
    assert_eq!(guides(&m, BodyId::Root), vec![(id, g)]);
    assert_eq!(
        log.records,
        vec![ModelRecord::AddGuide(BodyId::Root, id, g)]
    );
    assert!(m.root().geometry().vertices().next().is_none());
    assert!(m.validate().is_empty());
}

#[test]
fn ids_are_distinct_and_ascending() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_guide(BodyId::Root, pt(0.0), &mut log).unwrap();
    let b = m.add_guide(BodyId::Root, pt(1.0), &mut log).unwrap();
    let c = m
        .add_guide(BodyId::Root, line(2.0, DVec3::X), &mut log)
        .unwrap();
    assert!(a < b && b < c);
    assert_eq!(log.records.len(), 3);
    let ids: Vec<GuideId> = guides(&m, BodyId::Root).iter().map(|(i, _)| *i).collect();
    assert_eq!(ids, vec![a, b, c]);
}

#[test]
fn guides_live_in_their_body() {
    let (mut m, d) = with_definition();
    let mut log = ModelEdit::new();
    let body = BodyId::Definition(d);
    let id = m.add_guide(body, pt(3.0), &mut log).unwrap();
    assert_eq!(guides(&m, body), vec![(id, pt(3.0))]);
    assert!(m.root().guides().is_empty());
    assert!(!m.body(body).unwrap().is_empty());
}

#[test]
fn add_guide_errors_change_nothing() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    m.add_guide(BodyId::Root, pt(0.0), &mut log).unwrap();
    let before = m.clone();
    let log_before = log.clone();
    let missing = BodyId::Definition(DefinitionId(7));
    assert_eq!(
        m.add_guide(missing, pt(0.0), &mut log),
        Err(ModelError::MissingBody(missing))
    );
    // MissingBody is checked before BadGuide.
    assert_eq!(
        m.add_guide(missing, Guide::Point(v(f64::NAN, 0.0, 0.0)), &mut log),
        Err(ModelError::MissingBody(missing))
    );
    for bad in [
        Guide::Point(v(f64::NAN, 0.0, 0.0)),
        Guide::Point(v(0.0, f64::INFINITY, 0.0)),
        line(f64::NEG_INFINITY, DVec3::X),
        line(0.0, v(f64::NAN, 0.0, 0.0)),
        line(0.0, v(2.0, 0.0, 0.0)),
        line(0.0, v(0.0, 0.0, 0.0)),
        line(0.0, v(1.0, 1.0, 0.0)),
        line(0.0, v(1.0 + 1e-6, 0.0, 0.0)),
    ] {
        assert_eq!(
            m.add_guide(BodyId::Root, bad, &mut log),
            Err(ModelError::BadGuide),
            "{bad:?}"
        );
    }
    assert_eq!(m, before);
    assert_eq!(log, log_before);
}

#[test]
fn nearly_unit_direction_is_good() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let d = v(1.0, 2.0, 3.0).normalize();
    m.add_guide(BodyId::Root, line(0.0, d), &mut log).unwrap();
    m.add_guide(BodyId::Root, line(0.0, v(1.0 + 5e-10, 0.0, 0.0)), &mut log)
        .unwrap();
    assert_eq!(m.root().guides().len(), 2);
}

#[test]
fn remove_guide_returns_it_and_logs() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_guide(BodyId::Root, pt(0.0), &mut log).unwrap();
    let b = m.add_guide(BodyId::Root, pt(1.0), &mut log).unwrap();
    let mut log = ModelEdit::new();
    assert_eq!(m.remove_guide(BodyId::Root, a, &mut log), Ok(pt(0.0)));
    assert_eq!(
        log.records,
        vec![ModelRecord::RemoveGuide(BodyId::Root, a, pt(0.0))]
    );
    assert_eq!(guides(&m, BodyId::Root), vec![(b, pt(1.0))]);
}

#[test]
fn remove_guide_errors_change_nothing() {
    let (mut m, d) = with_definition();
    let mut log = ModelEdit::new();
    let a = m.add_guide(BodyId::Root, pt(0.0), &mut log).unwrap();
    let before = m.clone();
    let mut log = ModelEdit::new();
    let missing = BodyId::Definition(DefinitionId(9));
    assert_eq!(
        m.remove_guide(missing, a, &mut log),
        Err(ModelError::MissingBody(missing))
    );
    let other = BodyId::Definition(d);
    assert_eq!(
        m.remove_guide(other, a, &mut log),
        Err(ModelError::MissingGuide(other, a))
    );
    assert_eq!(
        m.remove_guide(BodyId::Root, GuideId(99), &mut log),
        Err(ModelError::MissingGuide(BodyId::Root, GuideId(99)))
    );
    assert_eq!(m, before);
    assert!(log.is_empty());
}

#[test]
fn clear_guides_removes_all_in_id_order() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    let a = m.add_guide(BodyId::Root, pt(0.0), &mut log).unwrap();
    let b = m
        .add_guide(BodyId::Root, line(1.0, DVec3::Y), &mut log)
        .unwrap();
    let c = m.add_guide(BodyId::Root, pt(2.0), &mut log).unwrap();
    m.remove_guide(BodyId::Root, b, &mut log).unwrap();
    let mut log = ModelEdit::new();
    let removed = m.clear_guides(BodyId::Root, &mut log).unwrap();
    assert_eq!(removed, vec![(a, pt(0.0)), (c, pt(2.0))]);
    assert_eq!(
        log.records,
        vec![
            ModelRecord::RemoveGuide(BodyId::Root, a, pt(0.0)),
            ModelRecord::RemoveGuide(BodyId::Root, c, pt(2.0)),
        ]
    );
    assert!(m.root().guides().is_empty());
}

#[test]
fn clear_guides_empty_and_missing_body() {
    let mut m = Model::new();
    let mut log = ModelEdit::new();
    assert_eq!(m.clear_guides(BodyId::Root, &mut log), Ok(vec![]));
    assert!(log.is_empty());
    let missing = BodyId::Definition(DefinitionId(1));
    assert_eq!(
        m.clear_guides(missing, &mut log),
        Err(ModelError::MissingBody(missing))
    );
    assert!(log.is_empty());
}

#[test]
fn undo_and_redo_add() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let before = m.clone();
    let mut log = ModelEdit::new();
    let g = line(0.5, DVec3::Y);
    let id = m.add_guide(BodyId::Root, g, &mut log).unwrap();
    let after = m.clone();
    undo.push("Guide", log);
    assert_eq!(undo.undo(&mut m), Ok(Some("Guide".to_string())));
    assert_eq!(m, before);
    assert!(m.root().guides().is_empty());
    assert_eq!(undo.redo(&mut m), Ok(Some("Guide".to_string())));
    assert_eq!(m, after);
    assert_eq!(guides(&m, BodyId::Root), vec![(id, g)]);
}

#[test]
fn undo_clear_restores_ids() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let mut log = ModelEdit::new();
    for i in 0..4 {
        m.add_guide(BodyId::Root, pt(i as f64), &mut log).unwrap();
    }
    undo.push("Guides", log);
    let full = m.clone();
    let mut log = ModelEdit::new();
    m.clear_guides(BodyId::Root, &mut log).unwrap();
    undo.push("Delete Guides", log);
    undo.undo(&mut m).unwrap();
    assert_eq!(m, full);
    undo.redo(&mut m).unwrap();
    assert!(m.root().guides().is_empty());
}

#[test]
fn ids_never_reused_after_undo_or_remove() {
    let mut m = Model::new();
    let mut undo = UndoStack::new();
    let mut log = ModelEdit::new();
    let a = m.add_guide(BodyId::Root, pt(0.0), &mut log).unwrap();
    undo.push("Guide", log);
    undo.undo(&mut m).unwrap();
    let mut log = ModelEdit::new();
    let b = m.add_guide(BodyId::Root, pt(1.0), &mut log).unwrap();
    assert_ne!(a, b);
    m.remove_guide(BodyId::Root, b, &mut log).unwrap();
    let c = m.add_guide(BodyId::Root, pt(2.0), &mut log).unwrap();
    assert_ne!(c, a);
    assert_ne!(c, b);
}

#[test]
fn models_equal_regardless_of_counter() {
    let mut m = Model::new();
    let empty = Model::new();
    let mut log = ModelEdit::new();
    let id = m.add_guide(BodyId::Root, pt(0.0), &mut log).unwrap();
    m.remove_guide(BodyId::Root, id, &mut log).unwrap();
    assert_eq!(m, empty);
    assert!(m.root().is_empty());
}

#[test]
fn records_apply_and_refuse() {
    let mut m = Model::new();
    let r = BodyId::Root;
    let g = pt(1.0);
    let rev = m.revision();
    m.apply(&ModelRecord::AddGuide(r, GuideId(5), g)).unwrap();
    assert_ne!(m.revision(), rev);
    assert_eq!(guides(&m, r), vec![(GuideId(5), g)]);

    let before = m.clone();
    assert_eq!(
        m.apply(&ModelRecord::AddGuide(r, GuideId(5), pt(2.0))),
        Err(ModelError::IdInUse)
    );
    assert_eq!(
        m.apply(&ModelRecord::AddGuide(
            r,
            GuideId(6),
            line(0.0, v(0.0, 3.0, 0.0))
        )),
        Err(ModelError::BadGuide)
    );
    let missing = BodyId::Definition(DefinitionId(3));
    assert_eq!(
        m.apply(&ModelRecord::AddGuide(missing, GuideId(1), g)),
        Err(ModelError::MissingBody(missing))
    );
    assert_eq!(
        m.apply(&ModelRecord::RemoveGuide(r, GuideId(5), pt(9.0))),
        Err(ModelError::Mismatch)
    );
    assert_eq!(
        m.apply(&ModelRecord::RemoveGuide(r, GuideId(6), g)),
        Err(ModelError::MissingGuide(r, GuideId(6)))
    );
    assert_eq!(
        m.apply(&ModelRecord::RemoveGuide(missing, GuideId(5), g)),
        Err(ModelError::MissingBody(missing))
    );
    assert_eq!(m, before);

    // A guide inserted under id 5 means the next new id is above 5.
    let mut log = ModelEdit::new();
    let next = m.add_guide(r, pt(3.0), &mut log).unwrap();
    assert!(next > GuideId(5));

    m.apply(&ModelRecord::RemoveGuide(r, GuideId(5), g))
        .unwrap();
    assert_eq!(guides(&m, r), vec![(next, pt(3.0))]);
}

#[test]
fn record_inverse_round_trip() {
    let mut m = Model::new();
    let rec = ModelRecord::AddGuide(BodyId::Root, GuideId(2), pt(4.0));
    m.apply(&rec).unwrap();
    m.apply(&rec.inverse()).unwrap();
    assert_eq!(m, Model::new());
}

#[test]
fn definition_with_guides_is_not_removable() {
    let (mut m, d) = with_definition();
    let mut log = ModelEdit::new();
    let id = m
        .add_guide(BodyId::Definition(d), pt(0.0), &mut log)
        .unwrap();
    let before = m.clone();
    assert_eq!(
        m.remove_definition(d, &mut log),
        Err(ModelError::DefinitionNotEmpty(d))
    );
    assert_eq!(m, before);
    m.remove_guide(BodyId::Definition(d), id, &mut log).unwrap();
    m.remove_definition(d, &mut log).unwrap();
}

#[test]
fn apply_edit_rolls_back_guides() {
    let mut m = Model::new();
    let r = BodyId::Root;
    let edit = ModelEdit {
        records: vec![
            ModelRecord::AddGuide(r, GuideId(0), pt(0.0)),
            ModelRecord::AddGuide(r, GuideId(0), pt(1.0)),
        ],
    };
    assert_eq!(m.apply_edit(&edit), Err(ModelError::IdInUse));
    assert_eq!(m, Model::new());
}
