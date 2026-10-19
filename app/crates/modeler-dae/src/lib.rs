// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! COLLADA 1.4.1 (.dae) import, written from the Khronos specification.
//!
//! Behaviour: `docs/specs/m0-dae-import.md`. [`import_file`] reads a document into a
//! [`Model`] that keeps the file's structure (component definitions, instances, units,
//! up axis). [`flatten`] turns a model into world-space draw batches in meters, Z up.
//! [`to_kernel`] converts a model into a `modeler-kernel` model, merging triangles back
//! into polygons (`docs/specs/m1-dae-to-kernel.md`). [`kernel_scene`] draws a kernel model
//! with the same batches as [`flatten`] (`docs/specs/m1-kernel-scene.md`).

use std::fmt;
use std::path::{Path, PathBuf};

use glam::DMat4;

mod flat;
mod kernel;
mod reader;
mod scene;

pub use flat::{Aabb, Batch, FlatScene, LineBatch, flatten};
pub use kernel::{ConvertError, to_kernel};
pub use scene::kernel_scene;

/// Why an import failed. Messages name the element or file involved.
#[derive(Debug)]
pub enum ImportError {
    /// The file could not be read.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Not well-formed XML.
    Xml(String),
    /// Well-formed XML that isn't a COLLADA document we can use (wrong root, broken
    /// reference, index out of range, unsupported construct). The string says which.
    Invalid(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            ImportError::Xml(msg) => write!(f, "XML error: {msg}"),
            ImportError::Invalid(msg) => write!(f, "invalid COLLADA: {msg}"),
        }
    }
}

impl std::error::Error for ImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ImportError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// The document's `<up_axis>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpAxis {
    X,
    Y,
    Z,
}

/// A whole document, in the file's own units and axes.
#[derive(Clone, Debug)]
pub struct Model {
    /// Meters per file unit (`<unit meter>`), 1.0 when absent.
    pub unit_meters: f64,
    /// `<up_axis>`, `Y` when absent (the COLLADA default).
    pub up_axis: UpAxis,
    /// One per `<geometry>`, in document order.
    pub meshes: Vec<Mesh>,
    /// One per `<material>`, in document order.
    pub materials: Vec<Material>,
    /// One per node in `<library_nodes>` (reusable components), in document order.
    pub definitions: Vec<Definition>,
    /// The instantiated visual scene. Its transform is the identity; its children are the
    /// scene's top-level nodes.
    pub scene: Node,
    /// Cameras placed in the visual scene, with their world transforms.
    pub cameras: Vec<ViewCamera>,
}

/// A scene-graph node.
#[derive(Clone, Debug)]
pub struct Node {
    pub name: String,
    /// Local transform: the product of the node's transform elements, in order.
    pub transform: DMat4,
    pub children: Vec<Node>,
    pub geometry: Vec<GeometryInstance>,
    /// `<instance_node>` references, as indices into [`Model::definitions`].
    pub instances: Vec<usize>,
}

/// A reusable node from `<library_nodes>`.
#[derive(Clone, Debug)]
pub struct Definition {
    pub id: String,
    pub name: String,
    pub node: Node,
}

/// An `<instance_geometry>`: a mesh plus its material bindings.
#[derive(Clone, Debug)]
pub struct GeometryInstance {
    /// Index into [`Model::meshes`].
    pub mesh: usize,
    /// `(symbol, material index)` pairs from `<bind_material>`.
    pub bindings: Vec<(String, usize)>,
}

impl GeometryInstance {
    /// The material bound to a primitive's material symbol, if any.
    pub fn material_for(&self, symbol: &str) -> Option<usize> {
        self.bindings
            .iter()
            .find(|(s, _)| s == symbol)
            .map(|&(_, m)| m)
    }
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub id: String,
    pub name: String,
    pub primitives: Vec<Primitive>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrimitiveKind {
    /// Every 3 indices form a triangle.
    Triangles,
    /// Every 2 indices form a line segment.
    Lines,
}

/// A `<triangles>` or `<lines>` element, re-indexed so one index addresses a position,
/// normal and texture coordinate together.
#[derive(Clone, Debug)]
pub struct Primitive {
    pub kind: PrimitiveKind,
    /// The element's `material` attribute.
    pub material_symbol: Option<String>,
    pub positions: Vec<[f32; 3]>,
    /// Same length as `positions`, or empty when the file has no normals.
    pub normals: Vec<[f32; 3]>,
    /// Same length as `positions`, or empty when the file has no texture coordinates.
    pub texcoords: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub id: String,
    pub name: String,
    /// Diffuse (or emission, for `constant`) RGBA as written in the file. White when the
    /// diffuse is a texture.
    pub color: [f32; 4],
    /// The diffuse texture's image file, resolved against the document's directory.
    pub texture: Option<PathBuf>,
    /// 0 (invisible) to 1 (opaque), from `<transparent>` and `<transparency>`.
    pub opacity: f32,
    /// True for the `constant` shading model (no lighting).
    pub unlit: bool,
}

/// A camera placed in the scene.
#[derive(Clone, Debug)]
pub struct ViewCamera {
    pub name: String,
    /// World transform of the camera node (file units and axes). The camera looks down
    /// its local -Z with +Y up, as COLLADA defines.
    pub transform: DMat4,
    pub projection: Projection,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Projection {
    Perspective { yfov_degrees: f64 },
    Orthographic { xmag: f64 },
}

/// Read a `.dae` file. Image paths resolve against the file's directory.
pub fn import_file(path: &Path) -> Result<Model, ImportError> {
    let text = std::fs::read_to_string(path).map_err(|source| ImportError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    import_str(&text, base)
}

/// Parse a COLLADA document held in memory. `base_dir` is where relative image paths
/// resolve from.
pub fn import_str(xml: &str, base_dir: &Path) -> Result<Model, ImportError> {
    reader::read(xml, base_dir)
}
