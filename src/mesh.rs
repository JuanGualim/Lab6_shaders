//! Cargador de archivos Wavefront OBJ (el del laboratorio 4, adaptado) y normales por vértice.
//!
//! - `v x y z`: posición de un vértice.
//! - `f a b c ...`: una cara. Cada referencia puede ser `v`, `v/vt`, `v//vn` o `v/vt/vn`; solo se
//!   usa el índice de posición. Los índices de OBJ empiezan en 1 y los negativos cuentan desde el
//!   último vértice leído. Las caras con más de tres vértices se dividen en abanico.
//!
//! `sphere.obj` trae una normal **por cara** (`vn`), así que la esfera se vería facetada. En lugar de
//! usarlas, cada vértice recibe el promedio de las normales de las caras que lo tocan (ponderado por
//! área). La geometría no cambia: solo se calcula un atributo que el pipeline interpola.

use std::fs;
use std::path::Path;

use crate::math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: Vec3,
    pub normal: Vec3,
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    /// Índices (base 0): cada grupo de tres forma un triángulo en sentido antihorario visto desde
    /// fuera.
    pub indices: Vec<usize>,
}

impl Mesh {
    pub fn load(path: impl AsRef<Path>) -> Result<Mesh, String> {
        let path = path.as_ref();
        let source = fs::read_to_string(path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
        Mesh::parse(&source).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(source: &str) -> Result<Mesh, String> {
        let mut positions = Vec::new();
        let mut indices = Vec::new();

        for (number, line) in source.lines().enumerate() {
            let number = number + 1;
            let mut tokens = line.split_whitespace();
            match tokens.next() {
                Some("v") => {
                    let mut coord = || -> Result<f32, String> {
                        let token = tokens
                            .next()
                            .ok_or_else(|| format!("línea {number}: vértice incompleto"))?;
                        token
                            .parse()
                            .map_err(|_| format!("línea {number}: coordenada inválida {token:?}"))
                    };
                    positions.push(Vec3::new(coord()?, coord()?, coord()?));
                }
                Some("f") => {
                    let face = tokens
                        .map(|token| resolve_index(token, positions.len(), number))
                        .collect::<Result<Vec<_>, _>>()?;
                    if face.len() < 3 {
                        return Err(format!("línea {number}: una cara necesita al menos 3 vértices"));
                    }
                    for i in 1..face.len() - 1 {
                        indices.extend([face[0], face[i], face[i + 1]]);
                    }
                }
                _ => {}
            }
        }

        // Normales suaves: suma de los productos cruz (su longitud es el doble del área).
        let mut normals = vec![Vec3::ZERO; positions.len()];
        for tri in indices.chunks_exact(3) {
            let [a, b, c] = [positions[tri[0]], positions[tri[1]], positions[tri[2]]];
            let face_normal = (b - a).cross(c - a);
            for &i in tri {
                normals[i] += face_normal;
            }
        }

        let vertices = positions
            .into_iter()
            .zip(normals)
            .map(|(position, normal)| Vertex {
                position,
                normal: normal.normalize(),
            })
            .collect();
        Ok(Mesh { vertices, indices })
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Distancia máxima de un vértice al origen (para escalar el modelo a radio 1).
    pub fn bounding_radius(&self) -> f32 {
        self.vertices
            .iter()
            .map(|v| v.position.length())
            .fold(0.0, f32::max)
    }
}

/// Convierte una referencia de cara (`7`, `7/2`, `7//3`, `7/2/3`, `-1`...) en un índice base 0.
fn resolve_index(token: &str, vertex_count: usize, number: usize) -> Result<usize, String> {
    let position = token.split('/').next().unwrap_or("");
    let index: i64 = position
        .parse()
        .map_err(|_| format!("línea {number}: índice inválido {token:?}"))?;
    let resolved = if index > 0 {
        index - 1
    } else {
        vertex_count as i64 + index
    };
    if index == 0 || resolved < 0 || resolved >= vertex_count as i64 {
        return Err(format!(
            "línea {number}: el índice {index} no corresponde a ningún vértice ({vertex_count} leídos)"
        ));
    }
    Ok(resolved as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuadrilatero_en_abanico() {
        let mesh = Mesh::parse("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n").unwrap();
        assert_eq!(mesh.indices, vec![0, 1, 2, 0, 2, 3]);
        assert!((mesh.vertices[0].normal - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-6);
    }

    #[test]
    fn referencias_con_texturas_y_normales() {
        let source = "v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvn 0 0 1\nf 1/1/1 2//1 3/1\n";
        assert_eq!(Mesh::parse(source).unwrap().indices, vec![0, 1, 2]);
    }

    #[test]
    fn esfera_del_laboratorio() {
        let mesh = Mesh::load(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sphere.obj")).unwrap();
        assert_eq!(mesh.vertices.len(), 98);
        assert_eq!(mesh.triangle_count(), 192);
        // Todas las normales promediadas apuntan hacia fuera (la esfera está centrada en el origen).
        for v in &mesh.vertices {
            assert!(v.normal.dot(v.position.normalize()) > 0.95);
        }
    }
}
