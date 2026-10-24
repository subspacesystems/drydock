// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-fix-copy-speed on a real exported house: copying a chair instance inside it is quick,
//! succeeds and leaves the body's geometry alone, although that body holds import defects.
//! Runs only when MODELER_CORPUS points at the corpus checkout.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use glam::{DMat4, DVec3};
use modeler_dae::{import_file, to_kernel};
use modeler_kernel::copy::copy_selection;
use modeler_kernel::model::{BodyId, UndoStack};
use modeler_kernel::select::Entity;

fn corpus() -> Option<PathBuf> {
    match std::env::var_os("MODELER_CORPUS") {
        Some(dir) => Some(PathBuf::from(dir)),
        None => {
            eprintln!("MODELER_CORPUS not set; skipping corpus checks (the test oracle runs them)");
            None
        }
    }
}

#[test]
fn copying_a_chair_in_the_house_is_quick_and_keeps_the_room() {
    let Some(dir) = corpus() else {
        return;
    };
    let scene = import_file(&dir.join("05-house/model.dae")).unwrap();
    let mut model = to_kernel(&scene).unwrap();
    // The file's scene is one instance at the root; the rooms and furniture are inside it.
    let top = model
        .body(BodyId::Root)
        .unwrap()
        .instances()
        .next()
        .unwrap()
        .1
        .definition;
    let body = BodyId::Definition(top);
    assert!(!model.validate_bodies(&[body]).is_empty());
    let (chair, definition) = model
        .body(body)
        .unwrap()
        .instances()
        .find(|(_, instance)| {
            model
                .definition(instance.definition)
                .is_some_and(|info| info.name.to_lowercase().contains("chair"))
        })
        .map(|(id, instance)| (id, instance.definition))
        .unwrap();
    let geometry = model.body(body).unwrap().geometry().clone();
    // On a thread with a time limit: before this task the copy ran for many minutes.
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut undo = UndoStack::new();
        let start = Instant::now();
        let got = copy_selection(
            &mut model,
            body,
            &[Entity::Instance(chair)],
            DMat4::from_translation(DVec3::new(1.0, 0.0, 0.0)),
            &mut undo,
            "Copy",
        );
        let _ = send.send((got, model, start.elapsed()));
    });
    let limit = Duration::from_secs(60);
    let (got, model, elapsed) = receive
        .recv_timeout(limit)
        .unwrap_or_else(|_| panic!("copy took longer than {limit:?}"));
    eprintln!("copy took {elapsed:?}");
    let got = got.unwrap();
    let [Entity::Instance(copy)] = got[..] else {
        panic!("expected one instance, got {got:?}");
    };
    assert_eq!(
        model.body(body).unwrap().instance(copy).unwrap().definition,
        definition
    );
    assert_eq!(model.body(body).unwrap().geometry(), &geometry);
}
