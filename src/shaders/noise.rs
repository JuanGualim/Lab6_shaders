//! Ruido procedural para los shaders: OpenSimplex2 y celular (Voronoi), más las combinaciones que
//! usan los tres cuerpos (fBm, ruido "ridged" y deformación de dominio).
//!
//! Todo se muestrea en 3D sobre la esfera unitaria. Así no hay costuras ni polos deformados como
//! los que tendría un ruido 2D aplicado a coordenadas de latitud/longitud.

use crate::math::Vec3;

// ---------------------------------------------------------------------------------------------
// OpenSimplex2 3D
// ---------------------------------------------------------------------------------------------
//
// Port del OpenSimplex2 3D de FastNoise Lite (Jordan Peck, licencia MIT; algoritmo de K.jpg). Es
// el mismo cálculo que `fastnoise_lite::FastNoiseLite::get_noise_3d` con `NoiseType::OpenSimplex2`
// y frecuencia 1, pero con la tabla de gradientes en un `static`: la versión del crate la declara
// `const` y Rust la copia en cada consulta, lo que la hacía ~4 veces más lenta en este renderer.

const SEED: i32 = 2026;
const PRIME_X: i32 = 501_125_321;
const PRIME_Y: i32 = 1_136_930_381;
const PRIME_Z: i32 = 1_720_413_743;

/// 12 gradientes hacia las aristas de un cubo, repetidos para llenar 64 entradas (x, y, z, 0).
#[rustfmt::skip]
static GRADIENTS_3D: [f32; 256] = [
    0., 1., 1., 0.,  0.,-1., 1., 0.,  0., 1.,-1., 0.,  0.,-1.,-1., 0.,
    1., 0., 1., 0., -1., 0., 1., 0.,  1., 0.,-1., 0., -1., 0.,-1., 0.,
    1., 1., 0., 0., -1., 1., 0., 0.,  1.,-1., 0., 0., -1.,-1., 0., 0.,
    0., 1., 1., 0.,  0.,-1., 1., 0.,  0., 1.,-1., 0.,  0.,-1.,-1., 0.,
    1., 0., 1., 0., -1., 0., 1., 0.,  1., 0.,-1., 0., -1., 0.,-1., 0.,
    1., 1., 0., 0., -1., 1., 0., 0.,  1.,-1., 0., 0., -1.,-1., 0., 0.,
    0., 1., 1., 0.,  0.,-1., 1., 0.,  0., 1.,-1., 0.,  0.,-1.,-1., 0.,
    1., 0., 1., 0., -1., 0., 1., 0.,  1., 0.,-1., 0., -1., 0.,-1., 0.,
    1., 1., 0., 0., -1., 1., 0., 0.,  1.,-1., 0., 0., -1.,-1., 0., 0.,
    0., 1., 1., 0.,  0.,-1., 1., 0.,  0., 1.,-1., 0.,  0.,-1.,-1., 0.,
    1., 0., 1., 0., -1., 0., 1., 0.,  1., 0.,-1., 0., -1., 0.,-1., 0.,
    1., 1., 0., 0., -1., 1., 0., 0.,  1.,-1., 0., 0., -1.,-1., 0., 0.,
    0., 1., 1., 0.,  0.,-1., 1., 0.,  0., 1.,-1., 0.,  0.,-1.,-1., 0.,
    1., 0., 1., 0., -1., 0., 1., 0.,  1., 0.,-1., 0., -1., 0.,-1., 0.,
    1., 1., 0., 0., -1., 1., 0., 0.,  1.,-1., 0., 0., -1.,-1., 0., 0.,
    1., 1., 0., 0.,  0.,-1., 1., 0., -1., 1., 0., 0.,  0.,-1.,-1., 0.,
];

#[inline(always)]
fn fast_round(f: f32) -> i32 {
    if f >= 0.0 { (f + 0.5) as i32 } else { (f - 0.5) as i32 }
}

#[inline(always)]
fn hash_3d(seed: i32, x: i32, y: i32, z: i32) -> i32 {
    (seed ^ x ^ y ^ z).wrapping_mul(0x27d4_eb2d)
}

#[inline(always)]
fn grad_coord(seed: i32, i: i32, j: i32, k: i32, x: f32, y: f32, z: f32) -> f32 {
    let hash = hash_3d(seed, i, j, k);
    let hash = ((hash ^ (hash >> 15)) & (63 << 2)) as usize;
    x * GRADIENTS_3D[hash] + y * GRADIENTS_3D[hash | 1] + z * GRADIENTS_3D[hash | 2]
}

/// OpenSimplex2 en 3D, en el rango -1..1. Usa dos rejillas cúbicas rotadas y desplazadas medio
/// cubo (una rejilla BCC): cada punto suma la contribución de los vértices cercanos de ambas.
pub fn simplex(p: Vec3) -> f32 {
    // Rotación (no sesgo) que alinea la rejilla para que no se vean ejes preferidos.
    let r = (p.x + p.y + p.z) * (2.0 / 3.0);
    let (x, y, z) = (r - p.x, r - p.y, r - p.z);

    let mut seed = SEED;
    let (i, j, k) = (fast_round(x), fast_round(y), fast_round(z));
    let mut x0 = x - i as f32;
    let mut y0 = y - j as f32;
    let mut z0 = z - k as f32;

    let mut x_sign = (-1.0 - x0) as i32 | 1;
    let mut y_sign = (-1.0 - y0) as i32 | 1;
    let mut z_sign = (-1.0 - z0) as i32 | 1;

    let mut ax0 = x_sign as f32 * -x0;
    let mut ay0 = y_sign as f32 * -y0;
    let mut az0 = z_sign as f32 * -z0;

    let mut i = i.wrapping_mul(PRIME_X);
    let mut j = j.wrapping_mul(PRIME_Y);
    let mut k = k.wrapping_mul(PRIME_Z);

    let mut value = 0.0;
    let mut a = (0.6 - x0 * x0) - (y0 * y0 + z0 * z0);

    for lattice in 0..2 {
        if a > 0.0 {
            value += (a * a) * (a * a) * grad_coord(seed, i, j, k, x0, y0, z0);
        }

        // Segundo vértice: el vecino en el eje donde el punto está más lejos del centro.
        if ax0 >= ay0 && ax0 >= az0 {
            let b = a + ax0 + ax0;
            if b > 1.0 {
                let b = b - 1.0;
                let i1 = i.wrapping_sub(x_sign.wrapping_mul(PRIME_X));
                value += (b * b) * (b * b) * grad_coord(seed, i1, j, k, x0 + x_sign as f32, y0, z0);
            }
        } else if ay0 > ax0 && ay0 >= az0 {
            let b = a + ay0 + ay0;
            if b > 1.0 {
                let b = b - 1.0;
                let j1 = j.wrapping_sub(y_sign.wrapping_mul(PRIME_Y));
                value += (b * b) * (b * b) * grad_coord(seed, i, j1, k, x0, y0 + y_sign as f32, z0);
            }
        } else {
            let b = a + az0 + az0;
            if b > 1.0 {
                let b = b - 1.0;
                let k1 = k.wrapping_sub(z_sign.wrapping_mul(PRIME_Z));
                value += (b * b) * (b * b) * grad_coord(seed, i, j, k1, x0, y0, z0 + z_sign as f32);
            }
        }

        if lattice == 1 {
            break;
        }

        // Pasar a la segunda rejilla, desplazada medio cubo.
        ax0 = 0.5 - ax0;
        ay0 = 0.5 - ay0;
        az0 = 0.5 - az0;

        x0 = x_sign as f32 * ax0;
        y0 = y_sign as f32 * ay0;
        z0 = z_sign as f32 * az0;

        a += (0.75 - ax0) - (ay0 + az0);

        i = i.wrapping_add((x_sign >> 1) & PRIME_X);
        j = j.wrapping_add((y_sign >> 1) & PRIME_Y);
        k = k.wrapping_add((z_sign >> 1) & PRIME_Z);

        x_sign = -x_sign;
        y_sign = -y_sign;
        z_sign = -z_sign;

        seed = !seed;
    }

    value * 32.694_283
}

/// Movimiento browniano fraccional: `octaves` capas de simplex, cada una al doble de frecuencia y
/// la mitad de amplitud. Normalizado a -1..1 (en la práctica casi siempre queda en ±0.6).
pub fn fbm(p: Vec3, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amplitude = 1.0;
    let mut total = 0.0;
    let mut q = p;
    for i in 0..octaves {
        // Un desplazamiento distinto por octava evita que todas coincidan en el origen.
        sum += simplex(q + Vec3::new(i as f32 * 17.13, i as f32 * -9.71, i as f32 * 5.37))
            * amplitude;
        total += amplitude;
        amplitude *= 0.5;
        q = q * 2.03;
    }
    sum / total
}

/// fBm reescalado a 0..1.
pub fn fbm01(p: Vec3, octaves: u32) -> f32 {
    fbm(p, octaves) * 0.5 + 0.5
}

/// Ruido "ridged": `1 - |simplex|` elevado al cuadrado forma crestas finas (cordilleras, filamentos).
/// Cada octava se multiplica por la anterior para que el detalle se concentre sobre las crestas.
pub fn ridged(p: Vec3, octaves: u32) -> f32 {
    let mut sum = 0.0;
    let mut amplitude = 0.5;
    let mut weight = 1.0;
    let mut q = p;
    for i in 0..octaves {
        let n = 1.0 - simplex(q + Vec3::splat(i as f32 * 3.1)).abs();
        let n = n * n * weight;
        weight = (n * 2.0).clamp(0.0, 1.0);
        sum += n * amplitude;
        amplitude *= 0.5;
        q = q * 2.1;
    }
    (sum / 0.9).clamp(0.0, 1.0)
}

/// Campo vectorial de deformación de dominio: tres fBm independientes. Sumarlo a `p` antes de
/// muestrear otro ruido curva sus formas (continentes orgánicos, remolinos, plasma).
pub fn warp(p: Vec3, octaves: u32) -> Vec3 {
    Vec3::new(
        fbm(p, octaves),
        fbm(p + Vec3::new(5.2, 1.3, 2.8), octaves),
        fbm(p + Vec3::new(-3.7, 8.1, -6.4), octaves),
    )
}

// ---------------------------------------------------------------------------------------------
// Celular (Voronoi / Worley)
// ---------------------------------------------------------------------------------------------

/// Desplazamiento pseudoaleatorio (0..1 en cada eje) del punto característico de una celda.
#[inline(always)]
fn feature_offset(i: i32, j: i32, k: i32) -> Vec3 {
    let h = hash_3d(SEED, i.wrapping_mul(PRIME_X), j.wrapping_mul(PRIME_Y), k.wrapping_mul(PRIME_Z));
    let unit = |shift: u32| ((h >> shift) & 0x3ff) as f32 / 1023.0;
    Vec3::new(unit(0), unit(10), unit(20))
}

/// Distancias al punto característico más cercano (F1) y al segundo más cercano (F2). Cada
/// celda entera tiene un punto en una posición aleatoria; basta revisar las 27 celdas vecinas.
pub fn worley(p: Vec3) -> (f32, f32) {
    let (ci, cj, ck) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
    let mut f1 = f32::MAX;
    let mut f2 = f32::MAX;
    for di in -1..=1 {
        for dj in -1..=1 {
            for dk in -1..=1 {
                let (i, j, k) = (ci + di, cj + dj, ck + dk);
                let feature = Vec3::new(i as f32, j as f32, k as f32) + feature_offset(i, j, k);
                let d = feature - p;
                let d2 = d.dot(d);
                if d2 < f1 {
                    f2 = f1;
                    f1 = d2;
                } else if d2 < f2 {
                    f2 = d2;
                }
            }
        }
    }
    (f1.sqrt(), f2.sqrt())
}

/// Distancia al punto de Voronoi más cercano (0 en el punto, ~1 lejos).
pub fn cell_distance(p: Vec3) -> f32 {
    worley(p).0
}

/// `F2 - F1` de Voronoi: 0 sobre los bordes entre celdas y crece hacia el interior.
pub fn cell_edges(p: Vec3) -> f32 {
    let (f1, f2) = worley(p);
    f2 - f1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simplex_matches_fastnoise_lite() {
        let mut reference = fastnoise_lite::FastNoiseLite::with_seed(SEED);
        reference.set_noise_type(Some(fastnoise_lite::NoiseType::OpenSimplex2));
        reference.set_frequency(Some(1.0));
        for i in 0..200 {
            let p = Vec3::new(i as f32 * 0.137 - 9.0, i as f32 * -0.071 + 3.0, i as f32 * 0.29);
            let expected = reference.get_noise_3d(p.x, p.y, p.z);
            assert!((simplex(p) - expected).abs() < 1e-5, "{p:?}");
        }
    }

    #[test]
    fn worley_is_zero_at_a_feature_point() {
        let feature = Vec3::new(2.0, -1.0, 5.0) + feature_offset(2, -1, 5);
        let (f1, f2) = worley(feature);
        assert!(f1 < 1e-6);
        assert!(f2 > f1);
    }
}
