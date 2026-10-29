// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Triangle-mesh files for 3D printing.
//!
//! Behaviour: `docs/specs/m7-import-stl.md`. [`stl::read`] reads an STL file (binary or
//! ASCII) into [`MeshObject`]s; [`to_kernel`] turns them into a kernel model with one group
//! per object, merging triangles into faces the way the DAE import does.

use std::path::Path;

use glam::DMat4;
use modeler_dae::{
    ConvertError, GeometryInstance, Mesh, Model, Node, Primitive, PrimitiveKind, UpAxis,
};
use modeler_kernel::model as km;

pub mod stl;

/// One named triangle mesh, in the file's units. Each triangle lists its corners in the
/// file's order (counter-clockwise seen from outside).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshObject {
    pub name: String,
    pub triangles: Vec<[[f32; 3]; 3]>,
}

/// A kernel model holding one group per object that has triangles, in object order, each
/// named after its object. File units are scaled by `unit_meters`; Z is up.
pub fn to_kernel(objects: &[MeshObject], unit_meters: f64) -> Result<km::Model, ConvertError> {
    let mut meshes = Vec::new();
    let mut children = Vec::new();
    for object in objects {
        if object.triangles.is_empty() {
            continue;
        }
        let mut positions = Vec::with_capacity(object.triangles.len() * 3);
        for triangle in &object.triangles {
            positions.extend_from_slice(triangle);
        }
        let indices = (0..positions.len() as u32).collect();
        let mesh = meshes.len();
        meshes.push(Mesh {
            id: object.name.clone(),
            name: object.name.clone(),
            primitives: vec![Primitive {
                kind: PrimitiveKind::Triangles,
                material_symbol: None,
                positions,
                normals: Vec::new(),
                texcoords: Vec::new(),
                indices,
            }],
        });
        children.push(Node {
            name: object.name.clone(),
            transform: DMat4::IDENTITY,
            children: Vec::new(),
            geometry: vec![GeometryInstance {
                mesh,
                bindings: Vec::new(),
            }],
            instances: Vec::new(),
        });
    }
    let model = Model {
        unit_meters,
        up_axis: UpAxis::Z,
        meshes,
        materials: Vec::new(),
        definitions: Vec::new(),
        scene: Node {
            name: String::new(),
            transform: DMat4::IDENTITY,
            children,
            geometry: Vec::new(),
            instances: Vec::new(),
        },
        cameras: Vec::new(),
    };
    modeler_dae::to_kernel(&model)
}

/// Reads an STL file into a kernel model (`stl::read`, then `to_kernel`). Unnamed solids are
/// named after the file stem. The error is a one-line message that names the file.
pub fn import_stl(path: &Path, unit_meters: f64) -> Result<km::Model, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let default_name = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Mesh".to_string());
    let objects =
        stl::read(&bytes, &default_name).map_err(|e| format!("{}: {e}", path.display()))?;
    to_kernel(&objects, unit_meters).map_err(|e| format!("{}: {e:?}", path.display()))
}
