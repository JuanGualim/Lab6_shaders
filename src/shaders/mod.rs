//! Fragment shaders de los tres cuerpos celestes y los datos que comparten (`Uniforms`).

pub mod gas_giant;
pub mod noise;
pub mod rocky;
pub mod star;

use crate::math::{Mat4, Vec3};
use crate::pipeline::Fragment;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Body {
    Star,
    Rocky,
    GasGiant,
}

impl Body {
    pub const ALL: [Body; 3] = [Body::Star, Body::Rocky, Body::GasGiant];

    pub fn name(self) -> &'static str {
        match self {
            Body::Star => "Pyrene (estrella)",
            Body::Rocky => "Thalassa (planeta rocoso)",
            Body::GasGiant => "Nyxara (gigante gaseoso)",
        }
    }

    /// Nombre corto para archivos y para `--body`.
    pub fn slug(self) -> &'static str {
        match self {
            Body::Star => "star",
            Body::Rocky => "rocky",
            Body::GasGiant => "gas",
        }
    }

    pub fn from_slug(s: &str) -> Option<Body> {
        Body::ALL.into_iter().find(|b| b.slug() == s)
    }
}

/// Parámetros ajustables de los tres shaders (se cambian con el teclado).
#[derive(Clone, Debug, Default)]
pub struct Params {
    pub star: star::StarParams,
    pub rocky: rocky::RockyParams,
    pub gas: gas_giant::GasParams,
}

/// Datos iguales para todos los vértices y fragmentos de un cuadro.
#[derive(Clone, Debug)]
pub struct Uniforms {
    pub model: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
    pub viewport: Mat4,
    /// Segundos de animación.
    pub time: f32,
    /// Dirección unitaria **hacia** la luz (luz direccional, como un sol lejano).
    pub light_dir: Vec3,
    pub camera_pos: Vec3,
    pub params: Params,
}

/// Lo que un shader necesita saber del punto que está coloreando.
pub struct Surface {
    /// Punto de la esfera unitaria en espacio del modelo: donde se muestrea el ruido.
    pub p: Vec3,
    /// Normal geométrica (interpolada).
    pub n: Vec3,
    /// Dirección hacia la cámara.
    pub v: Vec3,
    /// Dirección hacia la luz.
    pub l: Vec3,
    /// `max(0, n · l)`, calculada por el renderer.
    pub intensity: f32,
}

impl Surface {
    fn new(f: &Fragment, u: &Uniforms) -> Self {
        Surface {
            p: f.object.normalize(),
            n: f.normal,
            v: (u.camera_pos - f.world).normalize(),
            l: u.light_dir,
            intensity: f.intensity,
        }
    }
}

/// Elige el fragment shader del cuerpo activo.
pub fn fragment_shader(body: Body, f: &Fragment, u: &Uniforms) -> Vec3 {
    let s = Surface::new(f, u);
    match body {
        Body::Star => star::shade(&s, u.time, &u.params.star),
        Body::Rocky => rocky::shade(&s, u.time, &u.params.rocky),
        Body::GasGiant => gas_giant::shade(&s, u.time, &u.params.gas),
    }
}
