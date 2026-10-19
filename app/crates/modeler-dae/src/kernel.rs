// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! DAE to kernel: rebuild polygons, hierarchy and materials from an imported document.
//!
//! Behaviour: `docs/specs/m1-dae-to-kernel.md`. Design: `docs/adr/004-kernel-design.md`
//! ("DAE import").

use modeler_kernel::model as km;

use crate::Model;

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
    let _ = model;
    unimplemented!("m1-dae-to-kernel")
}
