//! Vectores, matrices y funciones auxiliares de los shaders (`mix`, `smoothstep`, rotaciones).

use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);
    pub const ONE: Vec3 = Vec3::new(1.0, 1.0, 1.0);
    pub const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Vec3 { x, y, z }
    }

    pub const fn splat(v: f32) -> Self {
        Vec3::new(v, v, v)
    }

    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalize(self) -> Vec3 {
        let len = self.length();
        if len > 0.0 { self / len } else { self }
    }

    /// Producto componente a componente (por ejemplo, color × color de la luz).
    pub fn mul_elem(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }

    pub fn max_elem(self) -> f32 {
        self.x.max(self.y).max(self.z)
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}

impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Div<f32> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f32) -> Vec3 {
        Vec3::new(self.x / s, self.y / s, self.z / s)
    }
}

impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

// ---------------------------------------------------------------------------------------------
// Funciones de los shaders (mismos nombres que en GLSL)
// ---------------------------------------------------------------------------------------------

/// Interpolación lineal: `a` cuando `t = 0`, `b` cuando `t = 1`.
pub fn mix(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

pub fn mixf(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Transición suave (Hermite) de 0 a 1 entre `e0` y `e1`. Acepta `e0 > e1` (transición invertida).
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn saturate(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// Rota `p` alrededor del eje Y.
pub fn rotate_y(p: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(c * p.x + s * p.z, p.y, -s * p.x + c * p.z)
}

/// Rota `p` alrededor del eje Z.
pub fn rotate_z(p: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    Vec3::new(c * p.x - s * p.y, s * p.x + c * p.y, p.z)
}

/// Rota `p` alrededor de un eje unitario cualquiera (fórmula de Rodrigues).
pub fn rotate_axis(p: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    let (s, c) = angle.sin_cos();
    p * c + axis.cross(p) * s + axis * (axis.dot(p) * (1.0 - c))
}

/// Dirección unitaria a partir de latitud y longitud (radianes). `y` es el eje polar.
pub fn from_lat_lon(lat: f32, lon: f32) -> Vec3 {
    Vec3::new(lat.cos() * lon.sin(), lat.sin(), lat.cos() * lon.cos())
}

/// Paleta por tramos: `stops` son pares (posición, color) ordenados por posición.
pub fn ramp(stops: &[(f32, Vec3)], t: f32) -> Vec3 {
    if t <= stops[0].0 {
        return stops[0].1;
    }
    for pair in stops.windows(2) {
        let (t0, c0) = pair[0];
        let (t1, c1) = pair[1];
        if t <= t1 {
            return mix(c0, c1, smoothstep(t0, t1, t));
        }
    }
    stops[stops.len() - 1].1
}

// ---------------------------------------------------------------------------------------------
// Matrices 4×4 (filas × columnas, vectores columna)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4 {
    pub m: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        m: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub fn scale(s: f32) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.m[0][0] = s;
        m.m[1][1] = s;
        m.m[2][2] = s;
        m
    }

    /// Matriz de vista: lleva el mundo a coordenadas de cámara (la cámara mira hacia -z).
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
        let f = (target - eye).normalize();
        let r = f.cross(up).normalize();
        let u = r.cross(f);
        Mat4 {
            m: [
                [r.x, r.y, r.z, -r.dot(eye)],
                [u.x, u.y, u.z, -u.dot(eye)],
                [-f.x, -f.y, -f.z, f.dot(eye)],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Proyección en perspectiva (como `gluPerspective`): z de -near..-far → -1..1 en NDC.
    pub fn perspective(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let f = 1.0 / (fovy / 2.0).tan();
        Mat4 {
            m: [
                [f / aspect, 0.0, 0.0, 0.0],
                [0.0, f, 0.0, 0.0],
                [
                    0.0,
                    0.0,
                    (far + near) / (near - far),
                    2.0 * far * near / (near - far),
                ],
                [0.0, 0.0, -1.0, 0.0],
            ],
        }
    }

    /// Viewport: NDC (-1..1) → píxeles (y hacia abajo) y profundidad 0..1.
    pub fn viewport(width: f32, height: f32) -> Mat4 {
        Mat4 {
            m: [
                [width / 2.0, 0.0, 0.0, width / 2.0],
                [0.0, -height / 2.0, 0.0, height / 2.0],
                [0.0, 0.0, 0.5, 0.5],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let mut r = [[0.0; 4]; 4];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..4).map(|k| self.m[i][k] * o.m[k][j]).sum();
            }
        }
        Mat4 { m: r }
    }

    /// Multiplica `(x, y, z, w)` y devuelve el vector homogéneo resultante.
    pub fn transform(&self, v: [f32; 4]) -> [f32; 4] {
        let mut r = [0.0; 4];
        for (i, out) in r.iter_mut().enumerate() {
            *out = (0..4).map(|k| self.m[i][k] * v[k]).sum();
        }
        r
    }

    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let [x, y, z, _] = self.transform([p.x, p.y, p.z, 1.0]);
        Vec3::new(x, y, z)
    }

    pub fn transform_dir(&self, d: Vec3) -> Vec3 {
        let [x, y, z, _] = self.transform([d.x, d.y, d.z, 0.0]);
        Vec3::new(x, y, z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_axis_matches_rotate_y() {
        let p = Vec3::new(0.3, -0.2, 0.9);
        let a = rotate_axis(p, Vec3::Y, 0.7);
        let b = rotate_y(p, 0.7);
        assert!((a - b).length() < 1e-5);
    }

    #[test]
    fn smoothstep_endpoints_and_inverted() {
        assert_eq!(smoothstep(0.0, 1.0, -1.0), 0.0);
        assert_eq!(smoothstep(0.0, 1.0, 2.0), 1.0);
        assert_eq!(smoothstep(1.0, 0.0, 0.0), 1.0);
        assert!((smoothstep(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn perspective_maps_near_and_far() {
        let p = Mat4::perspective(1.0, 1.0, 0.5, 10.0);
        let near = p.transform([0.0, 0.0, -0.5, 1.0]);
        let far = p.transform([0.0, 0.0, -10.0, 1.0]);
        assert!((near[2] / near[3] + 1.0).abs() < 1e-5);
        assert!((far[2] / far[3] - 1.0).abs() < 1e-5);
    }
}
