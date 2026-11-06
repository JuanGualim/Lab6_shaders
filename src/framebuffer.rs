//! Framebuffer en memoria (CPU): color RGBA8 y profundidad. La ventana solo copia el color a una
//! textura.

use crate::math::Vec3;

/// Tamaño de la ventana y del framebuffer.
pub const WIDTH: usize = 800;
pub const HEIGHT: usize = 600;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    /// Convierte un color de shader (0..1 por canal) a 8 bits, recortando lo que se sale del rango.
    pub fn from_vec3(c: Vec3) -> Self {
        let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        Color::new(to_u8(c.x), to_u8(c.y), to_u8(c.z))
    }
}

pub struct Framebuffer {
    pub width: usize,
    pub height: usize,
    color: Vec<Color>,
    depth: Vec<f32>,
    /// Fondo que se copia en cada `clear` (cielo estrellado generado una sola vez).
    background: Vec<Color>,
}

impl Framebuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let background = starfield(width, height);
        Framebuffer {
            width,
            height,
            color: background.clone(),
            depth: vec![f32::INFINITY; width * height],
            background,
        }
    }

    pub fn clear(&mut self) {
        self.color.copy_from_slice(&self.background);
        self.depth.fill(f32::INFINITY);
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height)
            .then(|| y as usize * self.width + x as usize)
    }

    /// Prueba de profundidad: si `depth` está más cerca que lo guardado, lo guarda y devuelve `true`.
    pub fn depth_test(&mut self, x: i32, y: i32, depth: f32) -> bool {
        match self.index(x, y) {
            Some(i) if depth < self.depth[i] => {
                self.depth[i] = depth;
                true
            }
            _ => false,
        }
    }

    pub fn depth_at(&self, x: i32, y: i32) -> f32 {
        self.index(x, y).map_or(f32::INFINITY, |i| self.depth[i])
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if let Some(i) = self.index(x, y) {
            self.color[i] = color;
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> Color {
        self.color[y * self.width + x]
    }

    /// Bytes RGBA8 en orden de filas, listos para `UpdateTexture` o para exportar.
    pub fn to_rgba_bytes(&self) -> Vec<u8> {
        self.color
            .iter()
            .flat_map(|c| [c.r, c.g, c.b, c.a])
            .collect()
    }
}

/// Hash entero → 0..1, reproducible (no depende de un generador aleatorio).
fn hash(mut n: u32) -> f32 {
    n = (n ^ 61) ^ (n >> 16);
    n = n.wrapping_mul(9);
    n ^= n >> 4;
    n = n.wrapping_mul(0x27d4_eb2d);
    n ^= n >> 15;
    n as f32 / u32::MAX as f32
}

/// Fondo: degradado oscuro con estrellas dispersas de brillo y tono variables.
fn starfield(width: usize, height: usize) -> Vec<Color> {
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let v = y as f32 / height as f32;
            let mut c = Vec3::new(0.010, 0.012, 0.030) + Vec3::new(0.012, 0.006, 0.025) * v;
            let h = hash((y * width + x) as u32);
            if h > 0.9985 {
                let brightness = (h - 0.9985) / 0.0015;
                let tint = hash((x * 7919 + y) as u32);
                let star = if tint > 0.5 {
                    Vec3::new(0.85, 0.9, 1.0)
                } else {
                    Vec3::new(1.0, 0.88, 0.75)
                };
                c += star * (0.25 + 0.6 * brightness);
            }
            pixels.push(Color::from_vec3(c));
        }
    }
    pixels
}
