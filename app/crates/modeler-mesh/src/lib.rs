// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Triangle-mesh files for 3D printing.
//!
//! Behaviour: `docs/specs/m7-import-stl.md`. [`stl::read`] reads an STL file (binary or
//! ASCII) into [`MeshObject`]s; [`to_kernel`] turns them into a kernel model with one group
//! per object, merging triangles into faces the way the DAE import does.

use std::path::Path;

use modeler_dae::ConvertError;
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
    let _ = (objects, unit_meters);
    unimplemented!()
}

/// Reads an STL file into a kernel model (`stl::read`, then `to_kernel`). Unnamed solids are
/// named after the file stem. The error is a one-line message that names the file.
pub fn import_stl(path: &Path, unit_meters: f64) -> Result<km::Model, String> {
    let _ = (path, unit_meters);
    unimplemented!()
}
