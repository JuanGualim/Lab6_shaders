//! Pipeline del software renderer:
//!
//! ```text
//! vertex shader → ensamblaje de primitivas → descarte de caras traseras → rasterización
//!   (con prueba de profundidad) → fragment shader (en paralelo) → framebuffer
//! ```
//!
//! La geometría es siempre la misma esfera; lo único que cambia entre cuerpos celestes es el
//! fragment shader que se llama en la última etapa.

use rayon::prelude::*;

use crate::framebuffer::{Color, Framebuffer};
use crate::math::Vec3;
use crate::mesh::{Mesh, Vertex};
use crate::shaders::{Body, Uniforms, fragment_shader};

/// Vértice después del vertex shader.
#[derive(Clone, Copy, Debug)]
pub struct TransformedVertex {
    /// Posición en pantalla: `x`, `y` en píxeles y `z` = profundidad 0..1.
    pub screen: Vec3,
    /// `1 / w` del espacio de recorte, para interpolar con corrección de perspectiva.
    pub inv_w: f32,
    /// Posición en el mundo (para el vector hacia la cámara).
    pub world: Vec3,
    /// Posición en el espacio del modelo: los shaders la normalizan para obtener el punto de la
    /// esfera donde muestrean el ruido.
    pub object: Vec3,
    /// Normal en el mundo.
    pub normal: Vec3,
}

/// Fragmento: candidato a píxel con sus atributos interpolados.
#[derive(Clone, Copy, Debug)]
pub struct Fragment {
    pub x: i32,
    pub y: i32,
    pub depth: f32,
    pub world: Vec3,
    pub object: Vec3,
    /// Normal interpolada y normalizada.
    pub normal: Vec3,
    /// Iluminación difusa que calcula el renderer: `max(0, n · l)`.
    pub intensity: f32,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RenderStats {
    pub triangles: usize,
    pub culled: usize,
    pub fragments: usize,
}

pub fn vertex_shader(vertex: &Vertex, u: &Uniforms) -> TransformedVertex {
    let world = u.model.transform_point(vertex.position);
    let clip = u
        .projection
        .transform(u.view.transform([world.x, world.y, world.z, 1.0]));
    let inv_w = 1.0 / clip[3];
    let ndc = Vec3::new(clip[0] * inv_w, clip[1] * inv_w, clip[2] * inv_w);
    TransformedVertex {
        screen: u.viewport.transform_point(ndc),
        inv_w,
        world,
        object: vertex.position,
        // El modelo solo se escala de forma uniforme, así que basta normalizar.
        normal: u.model.transform_dir(vertex.normal).normalize(),
    }
}

pub fn primitive_assembly(vertices: &[TransformedVertex], indices: &[usize]) -> Vec<[TransformedVertex; 3]> {
    indices
        .chunks_exact(3)
        .map(|t| [vertices[t[0]], vertices[t[1]], vertices[t[2]]])
        .collect()
}

/// Una cara es trasera si su normal (por el orden antihorario de los vértices) se aleja de la cámara.
pub fn is_back_facing(tri: &[TransformedVertex; 3], camera: Vec3) -> bool {
    let [a, b, c] = tri;
    let face_normal = (b.world - a.world).cross(c.world - a.world);
    face_normal.dot(camera - a.world) <= 0.0
}

/// Función de arista: el doble del área con signo del triángulo `(a, b, p)` en pantalla.
fn edge_function(a: Vec3, b: Vec3, px: f32, py: f32) -> f32 {
    (b.x - a.x) * (py - a.y) - (b.y - a.y) * (px - a.x)
}

/// Regla top-left (ver laboratorio 5): un centro exactamente sobre una arista solo cuenta si la
/// arista es superior o izquierda. Evita huecos y fragmentos duplicados entre triángulos vecinos.
fn is_top_left(a: Vec3, b: Vec3) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    dy < 0.0 || (dy == 0.0 && dx > 0.0)
}

/// Rasterización con coordenadas baricéntricas: recorre el rectángulo envolvente, prueba el centro
/// de cada píxel e interpola los atributos con corrección de perspectiva.
pub fn rasterize_triangle(
    tri: &[TransformedVertex; 3],
    light_dir: Vec3,
    width: usize,
    height: usize,
    mut emit: impl FnMut(Fragment),
) {
    let [v0, mut v1, mut v2] = *tri;
    let mut area = edge_function(v0.screen, v1.screen, v2.screen.x, v2.screen.y);
    if area == 0.0 {
        return;
    }
    if area < 0.0 {
        std::mem::swap(&mut v1, &mut v2);
        area = -area;
    }
    let (a, b, c) = (v0.screen, v1.screen, v2.screen);

    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as i32;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as i32;
    let max_x = a.x.max(b.x).max(c.x).ceil().min(width as f32 - 1.0) as i32;
    let max_y = a.y.max(b.y).max(c.y).ceil().min(height as f32 - 1.0) as i32;

    let inside = |w: f32, top_left: bool| w > 0.0 || (w == 0.0 && top_left);
    let (tl0, tl1, tl2) = (is_top_left(b, c), is_top_left(c, a), is_top_left(a, b));

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge_function(b, c, px, py);
            let w1 = edge_function(c, a, px, py);
            let w2 = edge_function(a, b, px, py);
            if !(inside(w0, tl0) && inside(w1, tl1) && inside(w2, tl2)) {
                continue;
            }
            let (l0, l1, l2) = (w0 / area, w1 / area, w2 / area);
            // La profundidad de pantalla es lineal en pantalla; los demás atributos no.
            let depth = l0 * a.z + l1 * b.z + l2 * c.z;
            let (p0, p1, p2) = (l0 * v0.inv_w, l1 * v1.inv_w, l2 * v2.inv_w);
            let sum = p0 + p1 + p2;
            let lerp = |f0: Vec3, f1: Vec3, f2: Vec3| (f0 * p0 + f1 * p1 + f2 * p2) / sum;

            let normal = lerp(v0.normal, v1.normal, v2.normal).normalize();
            emit(Fragment {
                x,
                y,
                depth,
                world: lerp(v0.world, v1.world, v2.world),
                object: lerp(v0.object, v1.object, v2.object),
                normal,
                intensity: normal.dot(light_dir).max(0.0),
            });
        }
    }
}

/// Dibuja la malla con el fragment shader del cuerpo indicado.
pub fn render(fb: &mut Framebuffer, mesh: &Mesh, u: &Uniforms, body: Body) -> RenderStats {
    let mut stats = RenderStats::default();

    // 1. Vertex shader
    let transformed: Vec<_> = mesh.vertices.iter().map(|v| vertex_shader(v, u)).collect();

    // 2. Ensamblaje de primitivas
    let triangles = primitive_assembly(&transformed, &mesh.indices);
    stats.triangles = triangles.len();

    // 3. Descarte de caras traseras + rasterización con prueba de profundidad temprana: solo se
    //    sombrean los fragmentos que quedan visibles.
    let mut fragments = Vec::new();
    for tri in &triangles {
        if is_back_facing(tri, u.camera_pos) {
            stats.culled += 1;
            continue;
        }
        rasterize_triangle(tri, u.light_dir, fb.width, fb.height, |f| {
            if fb.depth_test(f.x, f.y, f.depth) {
                fragments.push(f);
            }
        });
    }
    // Un fragmento pudo pasar la prueba y luego quedar tapado por otro más cercano.
    fragments.retain(|f| fb.depth_at(f.x, f.y) == f.depth);
    stats.fragments = fragments.len();

    // 4. Fragment shader: cada fragmento es independiente, así que se reparten entre los núcleos.
    let colors: Vec<Color> = fragments
        .par_iter()
        .map(|f| Color::from_vec3(fragment_shader(body, f, u)))
        .collect();

    // 5. Escritura en el framebuffer
    for (f, color) in fragments.iter().zip(colors) {
        fb.set_pixel(f.x, f.y, color);
    }
    stats
}
