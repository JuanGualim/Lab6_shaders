//! **Pyrene**, la estrella: una superficie activa que hierve.
//!
//! Capas (todas se combinan en un único campo de **temperatura** `T`, que al final se convierte en
//! color con una rampa de cuerpo negro):
//!
//! 1. **Plasma**: fBm de baja frecuencia deformado por un campo de ruido que cambia con el tiempo.
//!    Da las grandes zonas más calientes y más frías.
//! 2. **Granulación**: celdas de Voronoi (`F2 - F1`) deformadas por el mismo campo; los bordes entre
//!    celdas son más fríos, como las celdas de convección del Sol.
//! 3. **Filamentos**: ruido "ridged" que forma hebras brillantes y parpadea con el tiempo.
//! 4. **Manchas**: un fBm lento con dos umbrales (penumbra y umbra), solo en dos franjas de latitud
//!    ("cinturones de actividad"), que enfría la temperatura.
//! 5. **Oscurecimiento del limbo**: el borde del disco se ve más oscuro y rojo porque desde ahí se
//!    ve plasma menos profundo y más frío. Depende de `n · v`, no de la luz externa.
//!
//! La estrella es emisiva: **no usa la intensidad de la luz** del renderer.

use crate::math::{Vec3, ramp, rotate_y, smoothstep};

use super::Surface;
use super::noise::{cell_edges, fbm, fbm01, ridged, warp};

#[derive(Clone, Debug)]
pub struct StarParams {
    /// Frecuencia de las celdas de granulación (más alto = celdas más pequeñas).
    pub granulation_scale: f32,
    /// Velocidad con que el plasma se deforma ("hierve").
    pub boil_speed: f32,
    /// Cuánto oscurecen las manchas (0 = sin manchas).
    pub spot_amount: f32,
    /// Fuerza del oscurecimiento hacia el borde del disco (0..1).
    pub limb_darkening: f32,
}

impl Default for StarParams {
    fn default() -> Self {
        StarParams {
            granulation_scale: 16.0,
            boil_speed: 0.35,
            spot_amount: 1.0,
            limb_darkening: 0.7,
        }
    }
}

/// Rampa de temperatura: rojo oscuro → naranja → dorado → blanco caliente.
const TEMPERATURE: [(f32, Vec3); 5] = [
    (0.10, Vec3::new(0.22, 0.02, 0.02)),
    (0.38, Vec3::new(0.80, 0.16, 0.04)),
    (0.60, Vec3::new(1.00, 0.48, 0.08)),
    (0.80, Vec3::new(1.00, 0.80, 0.32)),
    (1.00, Vec3::new(1.00, 0.97, 0.82)),
];

pub fn shade(s: &Surface, time: f32, params: &StarParams) -> Vec3 {
    let p = s.p;
    // Rotación diferencial: el ecuador gira más rápido que los polos.
    let q = rotate_y(p, -time * (0.03 + 0.03 * (1.0 - p.y * p.y)));

    // Campo de deformación que avanza con el tiempo: hace que todo "hierva".
    let t = time * params.boil_speed;
    let w = warp(q * 2.2 + Vec3::new(0.0, t, t * 0.6), 3);

    let plasma = fbm01(q * 2.4 + w * 0.9, 4);
    let granules = smoothstep(0.0, 0.32, cell_edges(q * params.granulation_scale + w * 1.6));
    let filaments = ridged(q * 4.5 + w * 1.4 + Vec3::splat(t * 0.5), 3);

    let mut temperature = 0.34 + 0.45 * plasma + 0.18 * granules + 0.35 * filaments.powi(3);
    // Parpadeo: los filamentos pulsan con una fase distinta en cada zona del plasma.
    temperature += 0.07 * (time * 2.1 + plasma * 14.0).sin() * filaments;

    // Manchas: solo entre ~10° y ~40° de latitud, y se desplazan despacio.
    let belt = smoothstep(0.12, 0.22, p.y.abs()) * smoothstep(0.68, 0.5, p.y.abs());
    let spots = fbm01(q * 2.6 + Vec3::new(31.0, 7.0, time * 0.02), 4) + 0.04 * fbm(q * 18.0, 2);
    let penumbra = smoothstep(0.62, 0.67, spots) * belt * params.spot_amount;
    let umbra = smoothstep(0.69, 0.73, spots) * belt * params.spot_amount;
    temperature *= (1.0 - 0.32 * penumbra) * (1.0 - 0.45 * umbra);

    let mut color = ramp(&TEMPERATURE, temperature);

    // Oscurecimiento y enrojecimiento del limbo.
    let mu = s.n.dot(s.v).max(0.0);
    let limb = 1.0 - params.limb_darkening * (1.0 - mu.powf(0.55));
    color = color * limb;
    color = color.mul_elem(Vec3::new(1.0, 0.65 + 0.35 * mu, 0.45 + 0.55 * mu));
    // Halo de la cromosfera en el borde del disco.
    color += Vec3::new(1.0, 0.35, 0.12) * (1.0 - mu).powi(4) * 0.6;
    color
}
