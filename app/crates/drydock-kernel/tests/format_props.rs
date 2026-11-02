// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

use drydock_kernel::format::*;
use drydock_kernel::model::*;
use drydock_kernel::topology::*;
use glam::{DMat4, DVec3};
use proptest::prelude::*;

fn finite64() -> impl Strategy<Value = f64> {
    prop_oneof![
        3 => -100.0f64..100.0,
        1 => any::<f64>().prop_filter("finite", |x| x.is_finite()),
        1 => prop::sample::select(vec![0.0, -0.0, 1e-300, -5e-324, f64::MAX, f64::MIN, 0.1, 1.0 / 3.0]),
    ]
}

fn finite32() -> impl Strategy<Value = f32> {
    prop_oneof![
        3 => 0.0f32..1.0,
        1 => any::<f32>().prop_filter("finite", |x| x.is_finite()),
    ]
}

fn name() -> impl Strategy<Value = String> {
    prop_oneof![
        2 => "[a-zA-Z0-9 _.#|+-]{0,12}",
        1 => any::<String>(),
        1 => prop::collection::vec(
            prop::sample::select(vec!['"', '\\', '\n', '\r', '\t', '\u{0}', '\u{7f}', '\u{1b}', ' ', '-', 'u', '{', '}', 'é', '\u{1F600}']),
            0..10
        )
        .prop_map(|v| v.into_iter().collect()),
    ]
}

#[derive(Clone, Debug)]
enum Op {
    Material {
        name: String,
        color: [f32; 4],
        texture: Option<String>,
    },
    Definition {
        name: String,
        group: bool,
    },
    Draw {
        body: usize,
        points: Vec<[f64; 3]>,
        face: bool,
        holes: usize,
    },
    Paint {
        body: usize,
        face: u64,
        front: Option<usize>,
        back: Option<usize>,
    },
    EraseFace {
        body: usize,
        pick: usize,
    },
    Instance {
        body: usize,
        def: usize,
        mat: Option<usize>,
        cols: [f64; 16],
        name: String,
    },
}

fn op() -> impl Strategy<Value = Op> {
    let ix = 0usize..8;
    prop_oneof![
        2 => (name(), prop::array::uniform4(finite32()), prop::option::of(name()))
            .prop_map(|(name, color, texture)| Op::Material { name, color, texture }),
        2 => (name(), any::<bool>()).prop_map(|(name, group)| Op::Definition { name, group }),
        4 => (
            ix.clone(),
            prop::collection::vec(prop::array::uniform3(finite64()), 1..6),
            any::<bool>(),
            0usize..3
        )
            .prop_map(|(body, points, face, holes)| Op::Draw { body, points, face, holes }),
        2 => (ix.clone(), 0u64..6, prop::option::of(ix.clone()), prop::option::of(ix.clone()))
            .prop_map(|(body, face, front, back)| Op::Paint { body, face, front, back }),
        1 => (ix.clone(), ix.clone()).prop_map(|(body, pick)| Op::EraseFace { body, pick }),
        3 => (
            ix.clone(),
            ix.clone(),
            prop::option::of(ix),
            prop::array::uniform16(finite64()),
            name()
        )
            .prop_map(|(body, def, mat, cols, name)| Op::Instance { body, def, mat, cols, name }),
    ]
}

fn bodies(m: &Model) -> Vec<BodyId> {
    std::iter::once(BodyId::Root)
        .chain(m.definitions().map(|(d, _)| BodyId::Definition(d)))
        .collect()
}

fn pick<T: Copy>(all: &[T], i: usize) -> Option<T> {
    if all.is_empty() {
        None
    } else {
        Some(all[i % all.len()])
    }
}

fn apply_op(m: &mut Model, op: &Op) {
    let mut log = ModelEdit::new();
    let materials: Vec<MaterialId> = m.materials().map(|(id, _)| id).collect();
    let definitions: Vec<DefinitionId> = m.definitions().map(|(id, _)| id).collect();
    let all_bodies = bodies(m);
    match op {
        Op::Material {
            name,
            color,
            texture,
        } => {
            m.add_material(
                Material {
                    name: name.clone(),
                    color: *color,
                    texture: texture.as_deref().map(Texture::from),
                },
                &mut log,
            );
        }
        Op::Definition { name, group } => {
            m.add_definition(
                DefinitionInfo {
                    name: name.clone(),
                    group: *group,
                },
                &mut log,
            );
        }
        Op::Draw {
            body,
            points,
            face,
            holes,
        } => {
            let body = all_bodies[body % all_bodies.len()];
            m.edit_geometry(body, &mut log, |c, e| {
                let vs: Vec<VertexId> = points
                    .iter()
                    .map(|p| c.insert_vertex(DVec3::from_array(*p), e))
                    .collect();
                let mut uses = Vec::new();
                for w in 0..vs.len().saturating_sub(1) {
                    let id = c.insert_edge(vs[w], vs[w + 1], e).unwrap();
                    uses.push(EdgeUse {
                        edge: id,
                        reversed: w % 2 == 1,
                    });
                }
                if *face {
                    let inner = (0..*holes)
                        .map(|h| Loop {
                            uses: uses.iter().rev().skip(h).copied().collect(),
                        })
                        .collect();
                    c.insert_face(
                        Face {
                            outer: Loop { uses },
                            inner,
                        },
                        e,
                    )
                    .unwrap();
                }
            })
            .unwrap();
        }
        Op::Paint {
            body,
            face,
            front,
            back,
        } => {
            let body = all_bodies[body % all_bodies.len()];
            let fm = FaceMaterials {
                front: front.and_then(|i| pick(&materials, i)),
                back: back.and_then(|i| pick(&materials, i)),
                ..Default::default()
            };
            m.set_face_materials(body, FaceId(*face), fm, &mut log)
                .unwrap();
        }
        Op::EraseFace { body, pick: p } => {
            let body = all_bodies[body % all_bodies.len()];
            let faces: Vec<FaceId> = m
                .body(body)
                .unwrap()
                .geometry()
                .faces()
                .map(|(id, _)| id)
                .collect();
            if let Some(f) = pick(&faces, *p) {
                m.edit_geometry(body, &mut log, |c, e| c.delete_face(f, e))
                    .unwrap()
                    .unwrap();
            }
        }
        Op::Instance {
            body,
            def,
            mat,
            cols,
            name,
        } => {
            let body = all_bodies[body % all_bodies.len()];
            if let Some(d) = pick(&definitions, *def) {
                let inst = Instance {
                    definition: d,
                    transform: DMat4::from_cols_array(cols),
                    name: name.clone(),
                    material: mat.and_then(|i| pick(&materials, i)),
                };
                // Cycles are refused; that's fine.
                let _ = m.add_instance(body, inst, &mut log);
            }
        }
    }
}

fn build(ops: &[Op]) -> Model {
    let mut m = Model::new();
    for op in ops {
        apply_op(&mut m, op);
    }
    m
}

/// The model with face material entries for missing faces cleared.
fn without_stale(m: &Model) -> Model {
    let mut out = m.clone();
    let mut log = ModelEdit::new();
    for body in bodies(m) {
        let b = m.body(body).unwrap();
        for (f, _) in b.face_material_entries() {
            if b.geometry().face(f).is_none() {
                out.set_face_materials(body, f, FaceMaterials::default(), &mut log)
                    .unwrap();
            }
        }
    }
    out
}

#[derive(Clone, Debug)]
enum Damage {
    Truncate(usize),
    Replace(usize, char),
    Insert(usize, char),
    DropLine(usize),
    DuplicateLine(usize),
    SwapLines(usize, usize),
}

fn damage() -> impl Strategy<Value = Damage> {
    let ch = prop::sample::select(vec![
        ' ', '"', '\\', '|', '+', '-', 'x', '\n', '\r', '9', '0', '.', 'e', '{', '}', '#', 'u',
        '\t', 'é',
    ]);
    let ix = any::<usize>;
    prop_oneof![
        ix().prop_map(Damage::Truncate),
        (ix(), ch.clone()).prop_map(|(i, c)| Damage::Replace(i, c)),
        (ix(), ch).prop_map(|(i, c)| Damage::Insert(i, c)),
        ix().prop_map(Damage::DropLine),
        ix().prop_map(Damage::DuplicateLine),
        (ix(), ix()).prop_map(|(a, b)| Damage::SwapLines(a, b)),
    ]
}

fn char_index(text: &str, i: usize) -> usize {
    let bounds: Vec<usize> = text
        .char_indices()
        .map(|(b, _)| b)
        .chain(std::iter::once(text.len()))
        .collect();
    bounds[i % bounds.len()]
}

fn apply_damage(text: &str, d: &Damage) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() && !matches!(d, Damage::Replace(..) | Damage::Insert(..)) {
        return text.to_string();
    }
    match d {
        Damage::Truncate(i) => text[..char_index(text, *i)].to_string(),
        Damage::Replace(i, c) => {
            let at = char_index(text, *i);
            let mut s = text[..at].to_string();
            s.push(*c);
            if let Some(next) = text[at..].chars().next() {
                s.push_str(&text[at + next.len_utf8()..]);
            }
            s
        }
        Damage::Insert(i, c) => {
            let at = char_index(text, *i);
            format!("{}{}{}", &text[..at], c, &text[at..])
        }
        Damage::DropLine(i) => {
            lines.remove(i % lines.len());
            lines.join("\n")
        }
        Damage::DuplicateLine(i) => {
            let k = i % lines.len();
            lines.insert(k, lines[k]);
            lines.join("\n")
        }
        Damage::SwapLines(a, b) => {
            let n = lines.len();
            lines.swap(a % n, b % n);
            lines.join("\n")
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn round_trip_is_exact(ops in prop::collection::vec(op(), 0..30)) {
        let m = build(&ops);
        let text = to_text(&m).unwrap();
        let back = from_text(&text).unwrap();
        prop_assert_eq!(&back, &without_stale(&m));
        prop_assert_eq!(to_text(&back).unwrap(), text);
        // Every number came back bit for bit (equality treats 0 and -0 alike).
        for body in bodies(&m) {
            let a = m.body(body).unwrap();
            let b = back.body(body).unwrap();
            for (id, v) in a.geometry().vertices() {
                let w = b.geometry().vertex(id).unwrap();
                for k in 0..3 {
                    prop_assert_eq!(v.position[k].to_bits(), w.position[k].to_bits());
                }
            }
            for (id, i) in a.instances() {
                let j = b.instance(id).unwrap();
                for (x, y) in i.transform.to_cols_array().iter().zip(j.transform.to_cols_array()) {
                    prop_assert_eq!(x.to_bits(), y.to_bits());
                }
            }
        }
        for (id, mat) in m.materials() {
            let other = back.material(id).unwrap();
            for k in 0..4 {
                prop_assert_eq!(mat.color[k].to_bits(), other.color[k].to_bits());
            }
        }
    }

    #[test]
    fn damaged_text_never_panics(
        ops in prop::collection::vec(op(), 0..15),
        damages in prop::collection::vec(damage(), 1..4),
    ) {
        let m = build(&ops);
        let mut text = to_text(&m).unwrap();
        for d in &damages {
            text = apply_damage(&text, d);
        }
        match from_text(&text) {
            Ok(read) => {
                // Whatever was read can be written and read again.
                let again = to_text(&read).unwrap();
                prop_assert_eq!(from_text(&again).unwrap(), read);
            }
            Err(e) => {
                prop_assert!(e.line >= 1);
                prop_assert!(e.line <= text.lines().count().max(1));
            }
        }
    }

    #[test]
    fn random_lines_never_panic(body in prop::collection::vec("[a-z0-9 +|\"\\\\{}.\\-]{0,40}", 0..8)) {
        let text = format!("modeler-model 1\n{}\n", body.join("\n"));
        if let Err(e) = from_text(&text) {
            prop_assert!(e.line >= 1 && e.line <= text.lines().count());
        }
    }
}
