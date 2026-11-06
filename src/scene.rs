//! Estado de la escena (cuerpo activo, cámara, luz, tiempo y parámetros) y la lista de parámetros
//! que se pueden ajustar desde el teclado.

use crate::math::{Mat4, Vec3, from_lat_lon};
use crate::shaders::{Body, Params, Uniforms};

/// Cámara que orbita alrededor del origen.
#[derive(Clone, Copy, Debug)]
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl OrbitCamera {
    pub const MIN_DISTANCE: f32 = 1.6;
    pub const MAX_DISTANCE: f32 = 8.0;

    pub fn eye(&self) -> Vec3 {
        from_lat_lon(self.pitch, self.yaw) * self.distance
    }

    pub fn orbit(&mut self, d_yaw: f32, d_pitch: f32) {
        self.yaw += d_yaw;
        self.pitch = (self.pitch + d_pitch).clamp(-1.4, 1.4);
    }

    pub fn zoom(&mut self, factor: f32) {
        self.distance = (self.distance * factor).clamp(Self::MIN_DISTANCE, Self::MAX_DISTANCE);
    }
}

impl Default for OrbitCamera {
    fn default() -> Self {
        OrbitCamera {
            yaw: 0.0,
            pitch: 0.12,
            distance: 3.4,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub body: Body,
    pub camera: OrbitCamera,
    /// Dirección de la luz en coordenadas esféricas (radianes).
    pub light_azimuth: f32,
    pub light_elevation: f32,
    pub time: f32,
    pub paused: bool,
    /// La luz gira sola alrededor del planeta (para ver el terminador moverse).
    pub light_orbit: bool,
    pub params: Params,
    /// Escala del modelo para que la esfera del OBJ tenga radio 1.
    pub model_scale: f32,
}

impl Scene {
    pub fn new(body: Body, model_scale: f32) -> Self {
        Scene {
            body,
            camera: OrbitCamera::default(),
            light_azimuth: 0.8,
            light_elevation: 0.25,
            time: 0.0,
            paused: false,
            light_orbit: false,
            params: Params::default(),
            model_scale,
        }
    }

    pub fn light_dir(&self) -> Vec3 {
        from_lat_lon(self.light_elevation, self.light_azimuth)
    }

    pub fn advance(&mut self, dt: f32) {
        if !self.paused {
            self.time += dt;
        }
        if self.light_orbit {
            self.light_azimuth += dt * 0.6;
        }
    }

    /// Uniforms para un framebuffer de `width × height` píxeles.
    pub fn uniforms(&self, width: usize, height: usize) -> Uniforms {
        let camera_pos = self.camera.eye();
        Uniforms {
            model: Mat4::scale(self.model_scale),
            view: Mat4::look_at(camera_pos, Vec3::ZERO, Vec3::Y),
            projection: Mat4::perspective(0.785, width as f32 / height as f32, 0.1, 100.0),
            viewport: Mat4::viewport(width as f32, height as f32),
            time: self.time,
            light_dir: self.light_dir(),
            camera_pos,
            params: self.params.clone(),
        }
    }
}

/// Un parámetro ajustable del shader activo.
pub enum Param<'a> {
    Float {
        name: &'static str,
        value: &'a mut f32,
        step: f32,
        min: f32,
        max: f32,
    },
    Toggle {
        name: &'static str,
        value: &'a mut bool,
    },
}

impl Param<'_> {
    pub fn label(&self) -> String {
        match self {
            Param::Float { name, value, .. } => format!("{name}: {value:.3}"),
            Param::Toggle { name, value } => {
                format!("{name}: {}", if **value { "ON" } else { "OFF" })
            }
        }
    }

    /// `direction` = +1 / -1. Los interruptores se invierten en cualquier dirección.
    pub fn adjust(&mut self, direction: f32) {
        match self {
            Param::Float {
                value,
                step,
                min,
                max,
                ..
            } => **value = (**value + *step * direction).clamp(*min, *max),
            Param::Toggle { value, .. } => **value = !**value,
        }
    }
}

fn float<'a>(name: &'static str, value: &'a mut f32, step: f32, min: f32, max: f32) -> Param<'a> {
    Param::Float {
        name,
        value,
        step,
        min,
        max,
    }
}

/// Parámetros del cuerpo activo, en el orden en que aparecen en el HUD.
pub fn param_list(body: Body, params: &mut Params) -> Vec<Param<'_>> {
    match body {
        Body::Star => {
            let p = &mut params.star;
            vec![
                float("granulacion (escala)", &mut p.granulation_scale, 2.0, 4.0, 40.0),
                float("velocidad de hervor", &mut p.boil_speed, 0.05, 0.0, 2.0),
                float("manchas", &mut p.spot_amount, 0.25, 0.0, 2.0),
                float("oscurecimiento del limbo", &mut p.limb_darkening, 0.1, 0.0, 1.0),
            ]
        }
        Body::Rocky => {
            let p = &mut params.rocky;
            vec![
                float("nivel del mar", &mut p.sea_level, 0.02, -0.3, 0.4),
                Param::Toggle { name: "bump mapping", value: &mut p.bump },
                float("fuerza del relieve", &mut p.bump_strength, 0.25, 0.0, 4.0),
                Param::Toggle { name: "especular del oceano", value: &mut p.specular },
                float("brillo del oceano (exp.)", &mut p.ocean_shininess, 10.0, 5.0, 300.0),
                Param::Toggle { name: "luces de ciudades", value: &mut p.city_lights },
                Param::Toggle { name: "nubes", value: &mut p.clouds },
                float("cobertura de nubes", &mut p.cloud_coverage, 0.05, 0.0, 1.0),
                Param::Toggle { name: "sombras de nubes", value: &mut p.cloud_shadows },
                float("altura de las nubes", &mut p.cloud_height, 0.01, 0.0, 0.3),
                float("intensidad de sombra", &mut p.shadow_strength, 0.1, 0.0, 1.0),
                float("velocidad del terreno", &mut p.terrain_speed, 0.01, -0.5, 0.5),
                float("velocidad de las nubes", &mut p.cloud_speed, 0.01, -0.5, 0.5),
            ]
        }
        Body::GasGiant => {
            let p = &mut params.gas;
            vec![
                float("frecuencia de bandas", &mut p.band_frequency, 1.0, 2.0, 40.0),
                float("turbulencia", &mut p.turbulence, 0.02, 0.0, 0.6),
                float("velocidad del flujo", &mut p.flow_speed, 0.25, 0.0, 6.0),
                float("fuerza de tormentas", &mut p.storm_strength, 0.25, 0.0, 3.0),
            ]
        }
    }
}
