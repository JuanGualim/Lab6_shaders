//! **Nyxara**, el gigante gaseoso: bandas violetas y celestes que fluyen como un líquido, con una
//! gran tormenta rosada y dos óvalos blancos que curvan las bandas a su alrededor.
//!
//! Orden de las capas en `shade`:
//!
//! 1. **Flujo** (`advect` + `flowing_color`): el material se desplaza con dos movimientos: cada
//!    latitud avanza a su propia velocidad (corrientes en chorro que alternan este/oeste) y dentro
//!    de cada tormenta el gas gira alrededor de su centro. Para que el patrón no se estire sin fin,
//!    se usan dos fases desfasadas medio período que se reinician y se mezclan con pesos
//!    triangulares (técnica de "flow map").
//! 2. **Vórtices** (`swirl`): después de desplazar el material, el punto de muestreo se rota
//!    alrededor del eje de cada tormenta. El ángulo es máximo en el centro y cae con una gaussiana,
//!    así que las bandas que pasan cerca se enroscan en espiral. Esta deformación no avanza con el
//!    tiempo: la tormenta es un obstáculo fijo por el que pasa el gas.
//! 3. **Bandas** (`band_color`): la latitud se perturba con fBm estirado en longitud (remolinos
//!    alargados) y entra en un seno; una segunda onda lenta separa zonas claras y cinturones oscuros,
//!    y un ruido muy estirado añade vetas finas. El valor final recorre una paleta.
//! 4. **Iluminación**: difusa "envuelta" (el terminador es suave, como en una atmósfera densa),
//!    oscurecimiento del limbo y bruma polar.

use crate::math::{Vec3, from_lat_lon, mix, ramp, rotate_axis, rotate_y, rotate_z, saturate, smoothstep};

use super::Surface;
use super::noise::{fbm, fbm01};

#[derive(Clone, Debug)]
pub struct GasParams {
    /// Cantidad de bandas (frecuencia del seno sobre la latitud).
    pub band_frequency: f32,
    /// Cuánto se ondulan los bordes de las bandas.
    pub turbulence: f32,
    /// Multiplicador de la velocidad del flujo (chorros y giro de las tormentas).
    pub flow_speed: f32,
    /// Multiplicador de la deformación de las tormentas (0 = sin vórtices).
    pub storm_strength: f32,
}

impl Default for GasParams {
    fn default() -> Self {
        GasParams {
            band_frequency: 13.0,
            turbulence: 0.11,
            flow_speed: 1.0,
            storm_strength: 1.0,
        }
    }
}

struct Storm {
    lat: f32,
    lon: f32,
    /// Radio angular (radianes).
    radius: f32,
    /// Deformación en el centro (radianes); el signo da el sentido de rotación.
    swirl: f32,
    /// Velocidad angular del gas en el centro (rad/s), en el mismo sentido que `swirl`.
    spin: f32,
    /// Deriva en longitud (rad/s).
    drift: f32,
    color: Vec3,
}

const STORMS: [Storm; 3] = [
    Storm {
        lat: -0.36,
        lon: 0.30,
        radius: 0.26,
        swirl: 5.5,
        spin: 0.25,
        drift: 0.01,
        color: Vec3::new(0.93, 0.42, 0.55),
    },
    Storm {
        lat: 0.48,
        lon: -0.55,
        radius: 0.11,
        swirl: -4.5,
        spin: -0.4,
        drift: -0.015,
        color: Vec3::new(0.95, 0.95, 1.0),
    },
    Storm {
        lat: -0.05,
        lon: -1.0,
        radius: 0.09,
        swirl: 4.0,
        spin: 0.4,
        drift: 0.02,
        color: Vec3::new(0.92, 0.94, 1.0),
    },
];

const PALETTE: [(f32, Vec3); 5] = [
    (0.00, Vec3::new(0.10, 0.06, 0.24)),
    (0.30, Vec3::new(0.34, 0.22, 0.55)),
    (0.55, Vec3::new(0.66, 0.57, 0.86)),
    (0.76, Vec3::new(0.89, 0.90, 0.97)),
    (1.00, Vec3::new(0.42, 0.78, 0.86)),
];
const POLAR_HAZE: Vec3 = Vec3::new(0.16, 0.12, 0.30);
const RIM_HAZE: Vec3 = Vec3::new(0.62, 0.55, 0.95);
const AXIAL_TILT: f32 = 0.12;
/// Duración (s) de cada fase del flujo antes de reiniciarse.
const FLOW_PERIOD: f32 = 6.0;
/// Velocidad angular máxima de los chorros (rad/s) con `flow_speed = 1`.
const JET_SPEED: f32 = 0.12;

fn storm_center(storm: &Storm, time: f32) -> Vec3 {
    from_lat_lon(storm.lat, storm.lon + time * storm.drift)
}

/// Peso gaussiano de una tormenta en `q` (1 en el centro, ~0 a dos radios) y la distancia angular.
fn storm_falloff(q: Vec3, center: Vec3, radius: f32) -> (f32, f32) {
    let d = q.dot(center).clamp(-1.0, 1.0).acos();
    ((-(d / radius).powi(2)).exp(), d)
}

/// Velocidad de la corriente en cada latitud: chorros alternados hacia el este y el oeste.
fn jet_speed(y: f32) -> f32 {
    (y * 9.0).sin() * 0.7 + (y * 4.0 + 1.0).cos() * 0.3
}

/// Lleva `q` a donde estaba el gas hace `tau` segundos: chorros + giro dentro de las tormentas.
/// Los chorros se apagan dentro de las tormentas para que el vórtice no se estire con ellos.
fn advect(q: Vec3, tau: f32, time: f32) -> Vec3 {
    let shield: f32 = STORMS
        .iter()
        .map(|storm| 1.0 - storm_falloff(q, storm_center(storm, time), storm.radius * 1.3).0)
        .product();
    let mut q = rotate_y(q, -jet_speed(q.y) * JET_SPEED * shield * tau);
    for storm in &STORMS {
        let center = storm_center(storm, time);
        let (falloff, _) = storm_falloff(q, center, storm.radius);
        q = rotate_axis(q, center, -storm.spin * falloff * tau);
    }
    q
}

/// Enrosca el punto de muestreo alrededor de cada tormenta. Devuelve el punto deformado y el
/// color/peso del núcleo de la tormenta más cercana.
fn swirl(mut q: Vec3, time: f32, params: &GasParams) -> (Vec3, Vec3, f32) {
    let mut core_color = Vec3::ZERO;
    let mut core = 0.0;
    for storm in &STORMS {
        let center = storm_center(storm, time);
        let (falloff, d) = storm_falloff(q, center, storm.radius);
        q = rotate_axis(q, center, storm.swirl * params.storm_strength * falloff);

        let weight = smoothstep(storm.radius * 0.8, storm.radius * 0.25, d) * params.storm_strength.min(1.0);
        if weight > core {
            core = weight;
            core_color = storm.color;
        }
    }
    (q, core_color, core)
}

/// Color de las bandas en un punto de muestreo (ya desplazado por el flujo y las tormentas).
fn band_color(qs: Vec3, params: &GasParams) -> Vec3 {
    let turbulence = fbm(qs.mul_elem(Vec3::new(2.6, 7.0, 2.6)), 4);
    let y = qs.y + turbulence * params.turbulence;
    let bands = (y * params.band_frequency).sin() * 0.5 + 0.5;
    let zones = (y * 3.6 + 0.9).sin() * 0.5 + 0.5;
    let streaks = fbm(qs.mul_elem(Vec3::new(6.0, 26.0, 6.0)), 3) * 0.14;
    ramp(&PALETTE, saturate(bands * 0.62 + zones * 0.34 + streaks))
}

/// Color del gas en `q` en un instante del flujo: desplaza, enrosca, pinta las bandas y tiñe el
/// núcleo de las tormentas (conservando las vetas en espiral).
fn gas_color(q: Vec3, tau: f32, time: f32, params: &GasParams) -> Vec3 {
    let (qs, core_color, core) = swirl(advect(q, tau, time), time, params);
    let color = band_color(qs, params);
    let detail = fbm01(qs * 9.0, 2);
    mix(color, core_color * (0.75 + 0.35 * detail), core * 0.8)
}

/// Mezcla dos fases del flujo para que el gas avance indefinidamente sin estirarse. `flow_speed`
/// acelera el ciclo; el desplazamiento máximo de cada fase no cambia.
fn flowing_color(q: Vec3, time: f32, params: &GasParams) -> Vec3 {
    let t = time * params.flow_speed / FLOW_PERIOD;
    let phase_a = t.fract();
    let phase_b = (t + 0.5).fract();
    let sample = |phase: f32| gas_color(q, (phase - 0.5) * FLOW_PERIOD, time, params);
    let weight_a = 1.0 - (2.0 * phase_a - 1.0).abs();
    sample(phase_a) * weight_a + sample(phase_b) * (1.0 - weight_a)
}

pub fn shade(s: &Surface, time: f32, params: &GasParams) -> Vec3 {
    let q = rotate_z(s.p, -AXIAL_TILT);
    let mut color = flowing_color(q, time, params);

    // Bruma polar
    color = mix(color, POLAR_HAZE, smoothstep(0.72, 0.97, q.y.abs()) * 0.7);

    // Iluminación: difusa envuelta + oscurecimiento del limbo.
    let ndl = s.n.dot(s.l);
    let wrap = saturate((ndl + 0.2) / 1.2).powf(1.3);
    let mu = s.n.dot(s.v).max(0.0);
    let limb = 0.55 + 0.45 * mu.powf(0.4);
    color = color * (0.03 + wrap * limb);
    // Bruma iluminada en el borde del disco.
    color += RIM_HAZE * ((1.0 - mu).powi(3) * wrap * 0.45);
    color
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_phases_are_continuous_at_reset() {
        // Justo antes y justo después de que una fase se reinicia el color casi no cambia.
        let q = Vec3::new(0.3, 0.2, 0.93).normalize();
        let params = GasParams::default();
        let before = flowing_color(q, FLOW_PERIOD - 0.001, &params);
        let after = flowing_color(q, FLOW_PERIOD + 0.001, &params);
        assert!((before - after).length() < 0.01);
    }

    #[test]
    fn storms_leave_far_points_untouched() {
        let far = from_lat_lon(0.9, 2.5);
        let (q, _, core) = swirl(far, 0.0, &GasParams::default());
        assert!((q - far).length() < 1e-3);
        assert_eq!(core, 0.0);
    }

    #[test]
    fn advection_is_identity_at_zero_time_and_zero_inside_storm_centers() {
        let q = Vec3::new(-0.2, -0.3, 0.9).normalize();
        assert!((advect(q, 0.0, 1.0) - q).length() < 1e-6);
        // En el centro de una tormenta el gas solo gira sobre sí mismo: el punto no se mueve.
        let center = storm_center(&STORMS[0], 1.0);
        assert!((advect(center, 2.0, 1.0) - center).length() < 1e-4);
    }
}
