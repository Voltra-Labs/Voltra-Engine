//! A 3D mesh on the CPU: vertices and the triangles that index them.

use std::fmt;

use glam::Vec3;

use super::bounds::Aabb3;
use super::vertex::Vertex3D;

/// Vertices and triangle indices, three per triangle, counter-clockwise
/// seen from outside — what [`Mesh3D`](super::Mesh3D) uploads.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MeshData3D {
    /// The vertices.
    pub vertices: Vec<Vertex3D>,
    /// Indices into `vertices`, three per triangle.
    pub indices: Vec<u32>,
}

/// Why a [`MeshData3D`] cannot be drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshDataError {
    /// No triangle at all.
    Empty,
    /// The index count is not a multiple of three.
    PartialTriangle(usize),
    /// An index names a vertex past the last.
    IndexOutOfRange {
        /// The index.
        index: u32,
        /// How many vertices there are.
        vertices: usize,
    },
    /// A position is not a finite number.
    NonFinite(usize),
}

impl fmt::Display for MeshDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "the mesh has no triangles"),
            Self::PartialTriangle(n) => write!(f, "{n} indices is not whole triangles"),
            Self::IndexOutOfRange { index, vertices } => {
                write!(f, "index {index} is past the last of {vertices} vertices")
            }
            Self::NonFinite(at) => write!(f, "vertex {at} is not a finite position"),
        }
    }
}

impl std::error::Error for MeshDataError {}

impl MeshData3D {
    /// Whether it can be drawn: whole triangles, every index naming a
    /// vertex, every position finite.
    pub fn check(&self) -> Result<(), MeshDataError> {
        if self.indices.is_empty() {
            return Err(MeshDataError::Empty);
        }
        if !self.indices.len().is_multiple_of(3) {
            return Err(MeshDataError::PartialTriangle(self.indices.len()));
        }
        if let Some(&index) = self
            .indices
            .iter()
            .find(|&&i| i as usize >= self.vertices.len())
        {
            return Err(MeshDataError::IndexOutOfRange {
                index,
                vertices: self.vertices.len(),
            });
        }
        if let Some(at) = self
            .vertices
            .iter()
            .position(|v| !Vec3::from(v.position).is_finite())
        {
            return Err(MeshDataError::NonFinite(at));
        }
        Ok(())
    }

    /// The box around every vertex, or `None` with no vertices.
    pub fn bounds(&self) -> Option<Aabb3> {
        Aabb3::from_points(self.vertices.iter().map(|v| Vec3::from(v.position)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32) -> Vertex3D {
        Vertex3D {
            position: [x, 0.0, 0.0],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0; 2],
            tangent: [1.0, 0.0, 0.0, 1.0],
        }
    }

    #[test]
    fn a_mesh_is_checked_before_it_is_drawn() {
        let mut mesh = MeshData3D {
            vertices: vec![at(0.0), at(1.0), at(2.0)],
            indices: vec![0, 1, 2],
        };
        assert_eq!(mesh.check(), Ok(()));
        mesh.indices.push(0);
        assert_eq!(mesh.check(), Err(MeshDataError::PartialTriangle(4)));
        mesh.indices = vec![0, 1, 3];
        assert!(matches!(
            mesh.check(),
            Err(MeshDataError::IndexOutOfRange { index: 3, .. })
        ));
        mesh.indices.clear();
        assert_eq!(mesh.check(), Err(MeshDataError::Empty));
        mesh.indices = vec![0, 1, 2];
        mesh.vertices[1].position[2] = f32::INFINITY;
        assert_eq!(mesh.check(), Err(MeshDataError::NonFinite(1)));
    }
}
