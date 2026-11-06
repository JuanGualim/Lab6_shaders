//! **Thalassa**, el planeta rocoso: océanos turquesa, continentes con biomas, casquetes de hielo,
//! relieve, ciudades en el lado nocturno y una capa de nubes que proyecta sombras.
//!
//! Orden de las capas en `shade`:
//!
//! 1. **Campo de altura** (`continents` + `relief`): fBm con deformación de dominio para los
//!    continentes + ruido "ridged" para las cordilleras. `sea_level` decide qué es océano.
//! 2. **Biomas**: la altura, la latitud y un fBm de humedad eligen entre playa, pradera, bosque,
//!    desierto, roca y nieve. El océano se oscurece con la profundidad.
//! 3. **Bump mapping** (`bumped_normal`): la pendiente del campo de altura inclina la normal de
//!    iluminación. Los vértices no se mueven.
//! 4. **Materiales**: el agua tiene reflejo especular (Blinn-Phong + Fresnel), el hielo un brillo
//!    suave y la tierra es mate (solo difusa).
//! 5. **Sombras de nubes** (`cloud_shadow_point`): desde cada punto se sigue la dirección de la luz
//!    hasta la capa de nubes y se mira si ahí hay nube.
//! 6. **Luces de ciudades**: puntos de Voronoi en la tierra habitable que solo aparecen donde no
//!    llega el sol.
//! 7. **Nubes** (`cloud_density`): una máscara de fBm que gira a otra velocidad que el terreno.
//! 8. **Atmósfera**: un borde azul que depende del ángulo de visión y del lado iluminado.

use crate::math::{Vec3, mix, mixf, rotate_y, rotate_z, saturate, smoothstep};

use super::Surface;
use super::noise::{cell_distance, fbm, fbm01, ridged, warp};

#[derive(Clone, Debug)]
pub struct RockyParams {
    /// Nivel del mar en unidades del campo de altura (más alto = más océano).
    pub sea_level: f32,
    pub bump: bool,
    /// Cuánto inclina la pendiente del relieve a la normal.
    pub bump_strength: f32,
    pub specular: bool,
    /// Exponente de Blinn-Phong del océano (más alto = reflejo más pequeño y nítido).
    pub ocean_shininess: f32,
    pub city_lights: bool,
    pub clouds: bool,
    /// Fracción aproximada del cielo cubierta por nubes (0..1).
    pub cloud_coverage: f32,
    pub cloud_shadows: bool,
    /// Altura de la capa de nubes sobre la superficie (radio del planeta = 1).
    pub cloud_height: f32,
    /// Cuánto oscurece una nube densa la superficie (0..1).
    pub shadow_strength: f32,
    /// Velocidades angulares (rad/s) del terreno y de las nubes: distintas a propósito.
    pub terrain_speed: f32,
    pub cloud_speed: f32,
}

impl Default for RockyParams {
    fn default() -> Self {
        RockyParams {
            sea_level: 0.04,
            bump: true,
            bump_strength: 1.0,
            specular: true,
            ocean_shininess: 70.0,
            city_lights: true,
            clouds: true,
            cloud_coverage: 0.45,
            cloud_shadows: true,
            cloud_height: 0.07,
            shadow_strength: 0.85,
            terrain_speed: 0.03,
            cloud_speed: 0.09,
        }
    }
}

/// Inclinación del eje de rotación (≈ 23°).
const AXIAL_TILT: f32 = 0.4;
const AMBIENT: f32 = 0.025;
/// Convierte la pendiente del relieve en inclinación de la normal cuando `bump_strength = 1`.
const BUMP_SCALE: f32 = 0.05;
const SUN: Vec3 = Vec3::new(1.0, 0.96, 0.90);

const DEEP_OCEAN: Vec3 = Vec3::new(0.03, 0.11, 0.30);
const SHALLOW_OCEAN: Vec3 = Vec3::new(0.05, 0.45, 0.52);
const SAND: Vec3 = Vec3::new(0.80, 0.72, 0.52);
const MEADOW: Vec3 = Vec3::new(0.46, 0.56, 0.20);
const FOREST: Vec3 = Vec3::new(0.08, 0.30, 0.18);
const DESERT: Vec3 = Vec3::new(0.80, 0.44, 0.26);
const ROCK: Vec3 = Vec3::new(0.38, 0.34, 0.33);
const SNOW: Vec3 = Vec3::new(0.93, 0.95, 0.98);
const CLOUD: Vec3 = Vec3::new(0.97, 0.97, 1.0);
const CITY: Vec3 = Vec3::new(1.0, 0.72, 0.32);
const ATMOSPHERE: Vec3 = Vec3::new(0.35, 0.62, 1.0);

/// Pasa un punto del espacio del modelo al marco del terreno (eje inclinado, girando despacio).
fn terrain_frame(p: Vec3, time: f32, params: &RockyParams) -> Vec3 {
    rotate_y(rotate_z(p, -AXIAL_TILT), -time * params.terrain_speed)
}

/// Lo mismo para las nubes, que giran a su propia velocidad.
fn cloud_frame(p: Vec3, time: f32, params: &RockyParams) -> Vec3 {
    rotate_y(rotate_z(p, -AXIAL_TILT), -time * params.cloud_speed)
}

/// Forma de los continentes: fBm de baja frecuencia con deformación de dominio. `w` es la
/// deformación, que se calcula una vez por fragmento.
fn continents(q: Vec3, w: Vec3) -> f32 {
    fbm(q * 1.15 + w * 0.5, 5)
}

/// Relieve que se suma a los continentes: cordilleras ("ridged") que solo crecen tierra adentro
/// (`inland`) más una rugosidad fina. Es la parte del campo de altura que usa el bump mapping.
fn relief(q: Vec3, w: Vec3, inland: f32) -> f32 {
    ridged(q * 3.4 + w * 0.7, 4) * inland * 0.32 + fbm(q * 11.0, 2) * 0.03
}

/// Normal con relieve: diferencias finitas de `relief` en dos direcciones tangentes a la esfera.
/// La forma de los continentes casi no tiene pendiente, así que no se deriva (ahorra 10 muestras
/// de ruido por fragmento), y `w`/`inland` se reutilizan del punto central porque cambian muy poco
/// en una distancia tan corta.
fn bumped_normal(n: Vec3, p: Vec3, r0: f32, w: Vec3, inland: f32, time: f32, params: &RockyParams) -> Vec3 {
    const EPS: f32 = 0.003;
    let helper = if p.y.abs() < 0.99 { Vec3::Y } else { Vec3::new(1.0, 0.0, 0.0) };
    let tangent = helper.cross(p).normalize();
    let bitangent = p.cross(tangent);

    let r_at = |dir: Vec3| relief(terrain_frame((p + dir * EPS).normalize(), time, params), w, inland);
    let dr_t = (r_at(tangent) - r0) / EPS;
    let dr_b = (r_at(bitangent) - r0) / EPS;

    // El gradiente apunta cuesta arriba; la normal se inclina hacia el lado contrario.
    let gradient = tangent * dr_t + bitangent * dr_b;
    (n - gradient * (BUMP_SCALE * params.bump_strength)).normalize()
}

/// Densidad de nubes (0..1) en un punto de la esfera, ya en el marco de las nubes.
pub fn cloud_density(c: Vec3, time: f32, coverage: f32) -> f32 {
    // La deformación avanza con el tiempo: las nubes cambian de forma además de desplazarse.
    let w = warp(c * 1.4 + Vec3::new(0.0, time * 0.05, 0.0), 1);
    // Algo estiradas en longitud (más frecuencia en y), como los frentes de tormenta.
    let base = fbm01(c.mul_elem(Vec3::new(2.4, 3.2, 2.4)) + w * 0.3, 5);
    let threshold = mixf(0.66, 0.36, coverage);
    let body = smoothstep(threshold, threshold + 0.12, base);
    // Un ruido fino erosiona la nube: bordes deshilachados y huecos en lugar de manchas lisas.
    let detail = fbm01(c * 13.0 + w, 2);
    saturate(body * (0.45 + 0.75 * detail) - (1.0 - body) * 0.2)
}

/// Punto de la capa de nubes (radio `1 + cloud_height`) que está entre el punto `p` de la
/// superficie y la luz. Resuelve `|p + t·l| = r` y toma la raíz positiva:
/// `t = -(p·l) + sqrt((p·l)² - 1 + r²)`.
///
/// Con la luz cenital (`l = p`) da `t = cloud_height` y la sombra queda justo debajo de la nube;
/// cerca del terminador (`p·l ≈ 0`) el rayo cruza la capa muy lejos y la sombra se alarga.
pub fn cloud_shadow_point(p: Vec3, l: Vec3, cloud_height: f32) -> Vec3 {
    let r = 1.0 + cloud_height;
    let b = p.dot(l);
    let t = -b + (b * b - 1.0 + r * r).sqrt();
    (p + l * t).normalize()
}

pub fn shade(s: &Surface, time: f32, params: &RockyParams) -> Vec3 {
    let p = s.p;
    let q = terrain_frame(p, time, params);
    let w = warp(q * 1.3, 2);
    let c = continents(q, w);
    let inland = smoothstep(0.0, 0.25, c);
    let r = relief(q, w, inland);
    let e = c + r - params.sea_level;
    let lat = q.y.abs();

    // --- Biomas -------------------------------------------------------------------------------
    let land = smoothstep(-0.004, 0.004, e);
    let depth = smoothstep(0.0, 0.12, -e);
    let ocean_color = mix(SHALLOW_OCEAN, DEEP_OCEAN, depth);

    let elevation = saturate(e / 0.5);
    let moisture = fbm01(q * 2.2 + w * 0.8 + Vec3::new(11.0, 0.0, 0.0), 3);
    let temperature = 1.0 - lat * 1.1 - elevation * 0.35 + fbm(q * 3.0, 2) * 0.12;
    let desert = smoothstep(0.48, 0.38, moisture) * smoothstep(0.52, 0.66, temperature);
    let mountain = smoothstep(0.45, 0.75, elevation);
    let snow = smoothstep(0.2, 0.1, temperature);

    let mut land_color = mix(MEADOW, FOREST, smoothstep(0.42, 0.6, moisture));
    land_color = mix(land_color, DESERT, desert);
    land_color = mix(land_color, SAND, smoothstep(0.035, 0.0, e));
    land_color = mix(land_color, ROCK, mountain);
    land_color = mix(land_color, SNOW, snow);

    // Hielo marino en los polos, con borde irregular.
    let sea_ice = smoothstep(0.80, 0.86, lat + fbm(q * 4.0, 3) * 0.08) * (1.0 - land);
    let albedo = mix(mix(ocean_color, land_color, land), SNOW, sea_ice);

    // --- Respuesta a la luz según el material -------------------------------------------------
    let water = (1.0 - land) * (1.0 - sea_ice);
    let icy = saturate(sea_ice + snow * land);
    let spec_strength = water * 1.0 + icy * 0.08;
    let shininess = mixf(params.ocean_shininess, 10.0, icy);

    let n_geo = s.n;
    let ndl_geo = n_geo.dot(s.l);
    // Terminador suave: el relieve no puede iluminar el lado nocturno.
    let day = smoothstep(-0.06, 0.12, ndl_geo);

    let (n, diffuse) = if params.bump && land > 0.0 {
        let n = mix(n_geo, bumped_normal(n_geo, p, r, w, inland, time, params), land).normalize();
        (n, n.dot(s.l).max(0.0) * day)
    } else {
        (n_geo, s.intensity * day)
    };

    let clouds_on = params.clouds;
    let cq = cloud_frame(p, time, params);
    let cloud = if clouds_on { cloud_density(cq, time, params.cloud_coverage) } else { 0.0 };

    let shadow = if clouds_on && params.cloud_shadows && ndl_geo > -0.1 {
        let above = cloud_shadow_point(p, s.l, params.cloud_height);
        let density = cloud_density(cloud_frame(above, time, params), time, params.cloud_coverage);
        1.0 - params.shadow_strength * density
    } else {
        1.0
    };

    let specular = if params.specular && spec_strength > 0.0 {
        let half = (s.l + s.v).normalize();
        let fresnel = 0.04 + 0.96 * (1.0 - n.dot(s.v).max(0.0)).powi(5);
        spec_strength * n.dot(half).max(0.0).powf(shininess) * (0.6 + fresnel) * day
    } else {
        0.0
    };

    let mut surface = albedo * (AMBIENT + diffuse * shadow) + SUN * (specular * shadow);

    // --- Lado nocturno: ciudades --------------------------------------------------------------
    if params.city_lights {
        let night = smoothstep(0.05, -0.2, ndl_geo);
        if night > 0.0 {
            let points = smoothstep(0.42, 0.05, cell_distance(q * 22.0));
            let region = smoothstep(0.45, 0.6, fbm01(q * 3.2 + Vec3::splat(4.0), 3));
            let coast = smoothstep(0.14, 0.0, e);
            let habitable = land * (1.0 - snow) * (1.0 - desert * 0.85) * (1.0 - mountain);
            let city = points * saturate(region + coast * 0.7) * habitable;
            // Las nubes tapan las luces.
            surface += CITY * (city * night * 2.0 * (1.0 - cloud * 0.85));
        }
    }

    // --- Nubes y atmósfera --------------------------------------------------------------------
    let cloud_light = AMBIENT + saturate((ndl_geo + 0.1) / 1.1);
    let mut color = mix(surface, CLOUD * cloud_light, cloud);

    let rim = (1.0 - n_geo.dot(s.v).max(0.0)).powi(3);
    color += ATMOSPHERE * (rim * (0.1 + 0.9 * smoothstep(-0.25, 0.4, ndl_geo)) * 0.8);
    color
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shadow_point_is_straight_above_with_overhead_light() {
        let p = Vec3::new(0.0, 0.6, 0.8);
        let above = cloud_shadow_point(p, p, 0.05);
        assert!((above - p).length() < 1e-5);
    }

    #[test]
    fn shadow_point_moves_toward_the_light_and_grows_near_the_terminator() {
        let p = Vec3::new(0.0, 0.0, 1.0);
        let high_sun = Vec3::new(0.3, 0.0, 1.0).normalize();
        let low_sun = Vec3::new(1.0, 0.0, 0.15).normalize();
        let near = cloud_shadow_point(p, high_sun, 0.06);
        let far = cloud_shadow_point(p, low_sun, 0.06);
        // El punto de la nube se desplaza hacia la luz (+x en ambos casos)...
        assert!(near.x > 0.0 && far.x > 0.0);
        // ...y mucho más cuando el sol está bajo.
        assert!(far.x > 3.0 * near.x);
    }
}
