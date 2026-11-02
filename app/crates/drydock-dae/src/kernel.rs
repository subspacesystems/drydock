// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! DAE to kernel: rebuild polygons, hierarchy and materials from an imported document.
//!
//! Behaviour: `docs/specs/m1-dae-to-kernel.md`. Design: `docs/adr/004-kernel-design.md`
//! ("DAE import").

use std::collections::{HashMap, HashSet, VecDeque};

use drydock_kernel::geom::{self, LINEAR_TOL};
use drydock_kernel::model as km;
use drydock_kernel::texture::TextureMap;
use drydock_kernel::topology::{Context, EdgeId, EdgeUse, Edit, Face, FaceId, Loop, VertexId};
use glam::{DMat4, DVec2, DVec3};

use crate::{Model, Node, PrimitiveKind};

/// Why a document can't become a kernel model.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConvertError {
    /// The model refused a record built from the document (for example, component
    /// definitions that instance each other in a cycle).
    Model(km::ModelError),
}

/// Convert an imported document into a kernel model: meters, Z up, coplanar triangles
/// merged into faces, one kernel definition per reusable node, one group per other node.
pub fn to_kernel(model: &Model) -> Result<km::Model, ConvertError> {
    let c = crate::flat::frame_matrix(model.up_axis)
        * DMat4::from_scale(DVec3::splat(model.unit_meters));
    let mut cv = Conv {
        doc: model,
        c,
        c_inv: c.inverse(),
        out: km::Model::new(),
        mats: Vec::new(),
        defs: Vec::new(),
        log: km::ModelEdit::new(),
    };
    for m in &model.materials {
        let name = if m.name.is_empty() {
            m.id.clone()
        } else {
            m.name.clone()
        };
        let id = cv.out.add_material(
            km::Material {
                name,
                color: [m.color[0], m.color[1], m.color[2], m.opacity],
                texture: m
                    .texture
                    .as_ref()
                    .map(|p| km::Texture::from(p.to_string_lossy().as_ref())),
            },
            &mut cv.log,
        );
        cv.mats.push(id);
    }
    for d in &model.definitions {
        let name = if d.name.is_empty() {
            d.id.clone()
        } else {
            d.name.clone()
        };
        let id = cv
            .out
            .add_definition(km::DefinitionInfo { name, group: false }, &mut cv.log);
        cv.defs.push(id);
    }
    for (i, d) in model.definitions.iter().enumerate() {
        cv.contents(&d.node, km::BodyId::Definition(cv.defs[i]))?;
    }
    cv.contents(&model.scene, km::BodyId::Root)?;
    Ok(cv.out)
}

struct Conv<'a> {
    doc: &'a Model,
    c: DMat4,
    c_inv: DMat4,
    out: km::Model,
    mats: Vec<km::MaterialId>,
    defs: Vec<km::DefinitionId>,
    log: km::ModelEdit,
}

fn holds_something(n: &Node) -> bool {
    !n.geometry.is_empty() || !n.instances.is_empty() || n.children.iter().any(holds_something)
}

impl Conv<'_> {
    fn conj(&self, t: DMat4) -> DMat4 {
        self.c * t * self.c_inv
    }

    fn instance(
        &mut self,
        body: km::BodyId,
        def: usize,
        t: DMat4,
        name: &str,
    ) -> Result<(), ConvertError> {
        let Some(&definition) = self.defs.get(def) else {
            return Ok(());
        };
        let inst = km::Instance {
            definition,
            transform: self.conj(t),
            name: name.to_string(),
            material: None,
        };
        self.out
            .add_instance(body, inst, &mut self.log)
            .map_err(ConvertError::Model)?;
        Ok(())
    }

    fn contents(&mut self, n: &Node, body: km::BodyId) -> Result<(), ConvertError> {
        self.geometry(n, body)?;
        for k in &n.children {
            if k.geometry.is_empty() && k.children.is_empty() && k.instances.len() == 1 {
                let d = k.instances[0];
                let dt = self
                    .doc
                    .definitions
                    .get(d)
                    .map(|x| x.node.transform)
                    .unwrap_or(DMat4::IDENTITY);
                self.instance(body, d, k.transform * dt, &k.name)?;
            } else if !holds_something(k) {
                continue;
            } else {
                let id = self.out.add_definition(
                    km::DefinitionInfo {
                        name: k.name.clone(),
                        group: true,
                    },
                    &mut self.log,
                );
                self.contents(k, km::BodyId::Definition(id))?;
                let inst = km::Instance {
                    definition: id,
                    transform: self.conj(k.transform),
                    name: k.name.clone(),
                    material: None,
                };
                self.out
                    .add_instance(body, inst, &mut self.log)
                    .map_err(ConvertError::Model)?;
            }
        }
        for &d in &n.instances {
            let dt = self
                .doc
                .definitions
                .get(d)
                .map(|x| x.node.transform)
                .unwrap_or(DMat4::IDENTITY);
            self.instance(body, d, dt, "")?;
        }
        Ok(())
    }

    fn geometry(&mut self, n: &Node, body: km::BodyId) -> Result<(), ConvertError> {
        let mut w = Welder::default();
        let mut tris: Vec<([usize; 3], Option<km::MaterialId>, Option<TextureMap>)> = Vec::new();
        let mut segs: Vec<[usize; 2]> = Vec::new();
        for gi in &n.geometry {
            let Some(mesh) = self.doc.meshes.get(gi.mesh) else {
                continue;
            };
            for p in &mesh.primitives {
                let mat = p
                    .material_symbol
                    .as_deref()
                    .and_then(|s| gi.material_for(s))
                    .and_then(|i| self.mats.get(i).copied());
                let pt = |i: u32| -> Option<DVec3> {
                    let v = p.positions.get(i as usize)?;
                    let q =
                        self.c
                            .transform_point3(DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64));
                    q.is_finite().then_some(q)
                };
                match p.kind {
                    PrimitiveKind::Triangles => {
                        for t in p.indices.as_chunks::<3>().0 {
                            let (Some(a), Some(b), Some(cc)) = (pt(t[0]), pt(t[1]), pt(t[2]))
                            else {
                                continue;
                            };
                            let s = [w.slot(a), w.slot(b), w.slot(cc)];
                            let map = mat
                                .and_then(|id| self.out.material(id))
                                .and_then(|material| material.texture.as_ref())
                                .and_then(|texture| {
                                    let mut st = [DVec2::ZERO; 3];
                                    for (k, &index) in t.iter().enumerate() {
                                        let uv = p.texcoords.get(index as usize)?;
                                        st[k] =
                                            DVec2::new(uv[0] as f64, uv[1] as f64) * texture.size;
                                    }
                                    TextureMap::fit(&[w.pos[s[0]], w.pos[s[1]], w.pos[s[2]]], &st)
                                });
                            tris.push((s, mat, map));
                        }
                    }
                    PrimitiveKind::Lines => {
                        for t in p.indices.as_chunks::<2>().0 {
                            let (Some(a), Some(b)) = (pt(t[0]), pt(t[1])) else {
                                continue;
                            };
                            segs.push([w.slot(a), w.slot(b)]);
                        }
                    }
                }
            }
        }
        let pos = &w.pos;
        // Degenerate.
        tris.retain(|(s, _, _)| {
            if s[0] == s[1] || s[1] == s[2] || s[0] == s[2] {
                return false;
            }
            let pts = [pos[s[0]], pos[s[1]], pos[s[2]]];
            let nrm = geom::polygon_normal(&pts);
            nrm != DVec3::ZERO
                && geom::polygon_is_simple_2d(&geom::to_plane_2d(&pts, nrm), LINEAR_TOL)
        });
        segs.retain(|s| s[0] != s[1]);
        // Twins.
        let mut kept: Vec<T> = Vec::new();
        let mut by_set: HashMap<[usize; 3], usize> = HashMap::new();
        for (s, mat, map) in tris {
            let mut key = s;
            key.sort();
            if let Some(&k) = by_set.get(&key) {
                let same = same_way(kept[k].s, s);
                if !same && !kept[k].twinned {
                    kept[k].twinned = true;
                    kept[k].fm.back = mat;
                    kept[k].fm.back_map = map;
                }
                continue;
            }
            by_set.insert(key, kept.len());
            kept.push(T {
                s,
                fm: km::FaceMaterials {
                    front: mat,
                    front_map: map,
                    ..Default::default()
                },
                twinned: false,
            });
        }
        // Edge uses.
        let segset: HashSet<(usize, usize)> = segs.iter().map(|s| upair(s[0], s[1])).collect();
        let mut uses: HashMap<(usize, usize), Vec<(usize, bool)>> = HashMap::new();
        for (i, t) in kept.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (t.s[k], t.s[(k + 1) % 3]);
                uses.entry(upair(a, b)).or_default().push((i, a < b));
            }
        }
        let joinable = |pair: (usize, usize)| -> Option<(usize, usize)> {
            let u = uses.get(&pair)?;
            if u.len() != 2 || u[0].1 == u[1].1 || segset.contains(&pair) {
                return None;
            }
            if !same_look(&kept, u[0].0, u[1].0, pos) {
                return None;
            }
            Some((u[0].0, u[1].0))
        };
        // Patches.
        let mut patch_of = vec![usize::MAX; kept.len()];
        let mut patches: Vec<(Vec<usize>, DVec3)> = Vec::new();
        for seed in 0..kept.len() {
            if patch_of[seed] != usize::MAX {
                continue;
            }
            let pid = patches.len();
            let sp = [
                pos[kept[seed].s[0]],
                pos[kept[seed].s[1]],
                pos[kept[seed].s[2]],
            ];
            let nrm = geom::polygon_normal(&sp);
            let plane = geom::Plane {
                point: sp[0],
                normal: nrm,
            };
            let mut members = vec![seed];
            patch_of[seed] = pid;
            let mut q = VecDeque::from([seed]);
            while let Some(t) = q.pop_front() {
                for k in 0..3 {
                    let (a, b) = (kept[t].s[k], kept[t].s[(k + 1) % 3]);
                    let Some((x, y)) = joinable(upair(a, b)) else {
                        continue;
                    };
                    let u = if x == t { y } else { x };
                    if patch_of[u] != usize::MAX {
                        continue;
                    }
                    let up = [pos[kept[u].s[0]], pos[kept[u].s[1]], pos[kept[u].s[2]]];
                    if up
                        .iter()
                        .all(|p| plane.signed_distance(*p).abs() <= LINEAR_TOL / 2.0)
                        && geom::polygon_normal(&up).dot(nrm) > 0.0
                    {
                        patch_of[u] = pid;
                        members.push(u);
                        q.push_back(u);
                    }
                }
            }
            patches.push((members, nrm));
        }
        // Build.
        let mut fm_list: Vec<(drydock_kernel::topology::FaceId, km::FaceMaterials)> = Vec::new();
        let kept_ref = &kept;
        let segs_ref = &segs;
        self.out
            .edit_geometry(body, &mut self.log, |ctx, log| {
                let mut b = Builder {
                    ctx,
                    log,
                    pos,
                    verts: HashMap::new(),
                    edges: HashMap::new(),
                };
                for (pid, (members, nrm)) in patches.iter().enumerate() {
                    let inside_patch = |t: usize| patch_of[t] == pid;
                    if let Some(f) = insert_patch_face(
                        &mut b,
                        members,
                        kept_ref,
                        &inside_patch,
                        &joinable,
                        pos,
                        *nrm,
                    ) {
                        fm_list.push((f, kept_ref[members[0]].fm));
                        continue;
                    }
                    let regions = split_regions(members, kept_ref, &joinable, pos);
                    if regions.len() <= 1 {
                        for &t in members {
                            if let Some(f) = insert_triangle(&mut b, t, kept_ref) {
                                fm_list.push((f, kept_ref[t].fm));
                            }
                        }
                        continue;
                    }
                    for region in &regions {
                        let region_set: HashSet<usize> = region.iter().copied().collect();
                        let inside_region = |t: usize| region_set.contains(&t);
                        if let Some(f) = insert_patch_face(
                            &mut b,
                            region,
                            kept_ref,
                            &inside_region,
                            &joinable,
                            pos,
                            *nrm,
                        ) {
                            fm_list.push((f, kept_ref[region[0]].fm));
                        } else {
                            for &t in region {
                                if let Some(f) = insert_triangle(&mut b, t, kept_ref) {
                                    fm_list.push((f, kept_ref[t].fm));
                                }
                            }
                        }
                    }
                }
                for s in segs_ref {
                    b.edge_use(s[0], s[1]);
                }
            })
            .map_err(ConvertError::Model)?;
        for (f, fm) in fm_list {
            if fm != km::FaceMaterials::default() {
                self.out
                    .set_face_materials(body, f, fm, &mut self.log)
                    .map_err(ConvertError::Model)?;
            }
        }
        Ok(())
    }
}

fn maps_agree(a: Option<TextureMap>, b: Option<TextureMap>, points: &[DVec3]) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => points.iter().all(|&p| {
            let x = a.apply(p);
            let y = b.apply(p);
            let tolerance = 1e-3
                * 1.0f64
                    .max(x.x.abs())
                    .max(x.y.abs())
                    .max(y.x.abs())
                    .max(y.y.abs());
            (x.x - y.x).abs() <= tolerance && (x.y - y.y).abs() <= tolerance
        }),
        _ => false,
    }
}

fn same_look(kept: &[T], a: usize, b: usize, pos: &[DVec3]) -> bool {
    let (a, b) = (&kept[a], &kept[b]);
    if a.fm.front != b.fm.front || a.fm.back != b.fm.back {
        return false;
    }
    let points = [
        pos[a.s[0]],
        pos[a.s[1]],
        pos[a.s[2]],
        pos[b.s[0]],
        pos[b.s[1]],
        pos[b.s[2]],
    ];
    maps_agree(a.fm.front_map, b.fm.front_map, &points)
        && maps_agree(a.fm.back_map, b.fm.back_map, &points)
}

fn upair(a: usize, b: usize) -> (usize, usize) {
    if a < b { (a, b) } else { (b, a) }
}

fn same_way(a: [usize; 3], b: [usize; 3]) -> bool {
    (0..3).any(|r| (0..3).all(|i| a[i] == b[(i + r) % 3]))
}

/// Outer loop first, then holes; `None` when the patch must fall back.
fn boundary_loops(
    members: &[usize],
    kept: &[T],
    inside: &dyn Fn(usize) -> bool,
    joinable: &dyn Fn((usize, usize)) -> Option<(usize, usize)>,
    pos: &[DVec3],
    nrm: DVec3,
) -> Option<Vec<Vec<(usize, usize)>>> {
    let mut bnd: Vec<(usize, usize)> = Vec::new();
    for &t in members {
        let s = kept[t].s;
        for k in 0..3 {
            let (a, b) = (s[k], s[(k + 1) % 3]);
            let interior = matches!(joinable(upair(a, b)), Some((x, y)) if inside(x) && inside(y));
            if !interior {
                bnd.push((a, b));
            }
        }
    }
    let mut next: HashMap<usize, usize> = HashMap::new();
    for (i, &(a, _)) in bnd.iter().enumerate() {
        if next.insert(a, i).is_some() {
            return None;
        }
    }
    let mut used = vec![false; bnd.len()];
    let mut loops = Vec::new();
    for start in 0..bnd.len() {
        if used[start] {
            continue;
        }
        let mut l = Vec::new();
        let mut i = start;
        loop {
            if used[i] {
                return None;
            }
            used[i] = true;
            l.push(bnd[i]);
            let &n = next.get(&bnd[i].1)?;
            if n == start {
                break;
            }
            i = n;
        }
        loops.push(l);
    }
    let area = |l: &Vec<(usize, usize)>| {
        let pts: Vec<DVec3> = l.iter().map(|&(a, _)| pos[a]).collect();
        geom::signed_area_2d(&geom::to_plane_2d(&pts, nrm))
    };
    let pos_idx: Vec<usize> = (0..loops.len())
        .filter(|&i| area(&loops[i]) > 0.0)
        .collect();
    if pos_idx.len() != 1 {
        return None;
    }
    let outer = loops.remove(pos_idx[0]);
    loops.insert(0, outer);
    Some(loops)
}

/// Insert one face for `members`; `None` when tracing or the kernel refuses it.
fn insert_patch_face(
    b: &mut Builder,
    members: &[usize],
    kept: &[T],
    inside: &dyn Fn(usize) -> bool,
    joinable: &dyn Fn((usize, usize)) -> Option<(usize, usize)>,
    pos: &[DVec3],
    nrm: DVec3,
) -> Option<FaceId> {
    let loops = boundary_loops(members, kept, inside, joinable, pos, nrm)?;
    let face_loops: Option<Vec<Loop>> = loops
        .iter()
        .map(|l| {
            let uses: Option<Vec<EdgeUse>> = l.iter().map(|&(a, z)| b.edge_use(a, z)).collect();
            Some(Loop { uses: uses? })
        })
        .collect();
    let mut face_loops = face_loops?;
    let f = b
        .ctx
        .insert_face(
            Face {
                outer: face_loops.remove(0),
                inner: face_loops,
            },
            b.log,
        )
        .ok()?;
    if b.ctx.face_violation(f).is_some() {
        let _ = b.ctx.delete_face(f, b.log);
        None
    } else {
        Some(f)
    }
}

/// Insert one face for a single triangle.
fn insert_triangle(b: &mut Builder, t: usize, kept: &[T]) -> Option<FaceId> {
    let s = kept[t].s;
    let uses: Option<Vec<EdgeUse>> = (0..3).map(|k| b.edge_use(s[k], s[(k + 1) % 3])).collect();
    let uses = uses?;
    b.ctx
        .insert_face(
            Face {
                outer: Loop { uses },
                inner: Vec::new(),
            },
            b.log,
        )
        .ok()
}

/// Split a patch into greedily grown regions whose outlines stay clean as they grow.
fn split_regions(
    members: &[usize],
    kept: &[T],
    joinable: &dyn Fn((usize, usize)) -> Option<(usize, usize)>,
    pos: &[DVec3],
) -> Vec<Vec<usize>> {
    let member_set: HashSet<usize> = members.iter().copied().collect();
    let mut assigned: HashSet<usize> = HashSet::new();
    let mut regions: Vec<Vec<usize>> = Vec::new();
    for &seed in members {
        if assigned.contains(&seed) {
            continue;
        }
        let mut region = vec![seed];
        let mut region_set = HashSet::from([seed]);
        assigned.insert(seed);
        let s = kept[seed].s;
        let mut outline: Vec<(usize, usize)> = (0..3).map(|k| (s[k], s[(k + 1) % 3])).collect();
        loop {
            let mut added = false;
            let mut i = 0;
            while i < region.len() {
                let t = region[i];
                for k in 0..3 {
                    let (a, c) = (kept[t].s[k], kept[t].s[(k + 1) % 3]);
                    let Some((x, y)) = joinable(upair(a, c)) else {
                        continue;
                    };
                    let u = if x == t { y } else { x };
                    if !member_set.contains(&u) || assigned.contains(&u) {
                        continue;
                    }
                    if let Some(next) =
                        try_add_to_region(&outline, u, &region_set, kept, joinable, pos)
                    {
                        outline = next;
                        region.push(u);
                        region_set.insert(u);
                        assigned.insert(u);
                        added = true;
                    }
                }
                i += 1;
            }
            if !added {
                break;
            }
        }
        regions.push(region);
    }
    regions
}

fn point_segment_distance(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 == 0.0 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// The outline after adding `t`, if it stays clean; `None` otherwise.
fn try_add_to_region(
    outline: &[(usize, usize)],
    t: usize,
    region: &HashSet<usize>,
    kept: &[T],
    joinable: &dyn Fn((usize, usize)) -> Option<(usize, usize)>,
    pos: &[DVec3],
) -> Option<Vec<(usize, usize)>> {
    let s = kept[t].s;
    let mut edges = outline.to_vec();
    let mut added_edges = Vec::new();
    for k in 0..3 {
        let (a, b) = (s[k], s[(k + 1) % 3]);
        let neighbor_in_region = match joinable(upair(a, b)) {
            Some((x, y)) => {
                let u = if x == t { y } else { x };
                region.contains(&u)
            }
            None => false,
        };
        if neighbor_in_region {
            let at = edges.iter().position(|&e| e == (b, a))?;
            edges.remove(at);
        } else {
            edges.push((a, b));
            added_edges.push((a, b));
        }
    }
    for &v in &s {
        if edges.iter().filter(|&&(a, _)| a == v).count() > 1 {
            return None;
        }
    }
    for &v in &s {
        if !edges.iter().any(|&(a, _)| a == v) {
            continue;
        }
        let p = pos[v];
        for &(c, d) in &edges {
            if c == v || d == v {
                continue;
            }
            if point_segment_distance(p, pos[c], pos[d]) <= LINEAR_TOL {
                return None;
            }
        }
    }
    for &(a, b) in &added_edges {
        for &(c, _) in &edges {
            if c == a || c == b {
                continue;
            }
            if point_segment_distance(pos[c], pos[a], pos[b]) <= LINEAR_TOL {
                return None;
            }
        }
    }
    Some(edges)
}

struct T {
    s: [usize; 3],
    fm: km::FaceMaterials,
    twinned: bool,
}

struct Builder<'a> {
    ctx: &'a mut Context,
    log: &'a mut Edit,
    pos: &'a [DVec3],
    verts: HashMap<usize, VertexId>,
    edges: HashMap<(usize, usize), (EdgeId, usize)>,
}

impl Builder<'_> {
    fn vertex(&mut self, s: usize) -> VertexId {
        if let Some(&v) = self.verts.get(&s) {
            return v;
        }
        let v = self.ctx.insert_vertex(self.pos[s], self.log);
        self.verts.insert(s, v);
        v
    }

    /// The edge between two slots, made on first use; `None` only if the context
    /// refuses it (it can't for two distinct slots).
    fn edge_use(&mut self, a: usize, b: usize) -> Option<EdgeUse> {
        let key = upair(a, b);
        if let Some(&(e, first)) = self.edges.get(&key) {
            return Some(EdgeUse {
                edge: e,
                reversed: first != a,
            });
        }
        let va = self.vertex(a);
        let vb = self.vertex(b);
        let e = self.ctx.insert_edge(va, vb, self.log).ok()?;
        self.edges.insert(key, (e, a));
        Some(EdgeUse::forward(e))
    }
}

#[derive(Default)]
struct Welder {
    pos: Vec<DVec3>,
    grid: HashMap<(i64, i64, i64), Vec<usize>>,
}

impl Welder {
    fn cell(p: DVec3) -> (i64, i64, i64) {
        let f = |x: f64| (x / LINEAR_TOL).floor() as i64;
        (f(p.x), f(p.y), f(p.z))
    }

    fn slot(&mut self, p: DVec3) -> usize {
        let (cx, cy, cz) = Self::cell(p);
        let mut best: Option<(f64, usize)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(list) = self.grid.get(&(cx + dx, cy + dy, cz + dz)) {
                        for &s in list {
                            let d = self.pos[s].distance(p);
                            if d <= LINEAR_TOL
                                && best.is_none_or(|(bd, bs)| d < bd || (d == bd && s < bs))
                            {
                                best = Some((d, s));
                            }
                        }
                    }
                }
            }
        }
        if let Some((_, s)) = best {
            return s;
        }
        let s = self.pos.len();
        self.pos.push(p);
        self.grid.entry((cx, cy, cz)).or_default().push(s);
        s
    }
}
