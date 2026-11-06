//! Exportación sin ventana: las capturas y las secuencias de cuadros del README.
//!
//! Cada captura es una escena con parámetros fijos, así que las imágenes se pueden regenerar
//! exactamente con `cargo run --release -- --export docs`. Los GIF se arman después con
//! `scripts/make_gifs.sh` (ffmpeg).

use std::path::Path;

use raylib::prelude::{Color as RlColor, Image};

use crate::framebuffer::{Framebuffer, HEIGHT, WIDTH};
use crate::mesh::Mesh;
use crate::pipeline::render;
use crate::scene::Scene;
use crate::shaders::Body;

/// Una captura: nombre del archivo, cuerpo y ajustes sobre la escena por defecto.
struct Still {
    name: &'static str,
    body: Body,
    setup: fn(&mut Scene),
}

/// Una secuencia de cuadros: `update` recibe la escena y el número de cuadro.
struct Clip {
    name: &'static str,
    body: Body,
    frames: u32,
    fps: f32,
    setup: fn(&mut Scene),
    update: fn(&mut Scene, u32),
}

fn none(_: &mut Scene) {}
fn no_update(_: &mut Scene, _: u32) {}

/// Luz rasante desde la derecha y cámara cerca: resalta el relieve y las sombras.
fn grazing_light(s: &mut Scene) {
    s.time = 0.0;
    s.light_azimuth = 1.42;
    s.light_elevation = 0.15;
    s.camera.distance = 2.2;
    s.camera.yaw = 0.55;
    s.camera.pitch = 0.2;
}

const STILLS: &[Still] = &[
    Still { name: "star", body: Body::Star, setup: |s| s.time = 4.0 },
    Still { name: "rocky", body: Body::Rocky, setup: none },
    Still { name: "gas", body: Body::GasGiant, setup: |s| s.time = 2.0 },
    // Bump mapping: misma escena con y sin relieve.
    Still { name: "rocky_bump_off", body: Body::Rocky, setup: |s| {
        grazing_light(s);
        s.params.rocky.clouds = false;
        s.params.rocky.bump = false;
    } },
    Still { name: "rocky_bump_on", body: Body::Rocky, setup: |s| {
        grazing_light(s);
        s.params.rocky.clouds = false;
    } },
    // Sombras de nubes: con y sin sombras, luz rasante.
    Still { name: "rocky_shadows_off", body: Body::Rocky, setup: |s| {
        grazing_light(s);
        s.params.rocky.cloud_shadows = false;
    } },
    Still { name: "rocky_shadows_on", body: Body::Rocky, setup: grazing_light },
    // Especular: el reflejo del sol en el océano y la tierra mate.
    Still { name: "rocky_specular_off", body: Body::Rocky, setup: |s| {
        s.light_azimuth = 0.45;
        s.params.rocky.clouds = false;
        s.params.rocky.specular = false;
    } },
    Still { name: "rocky_specular_on", body: Body::Rocky, setup: |s| {
        s.light_azimuth = 0.45;
        s.params.rocky.clouds = false;
    } },
    // Día y noche: luz desde atrás a la derecha, las ciudades aparecen en el lado oscuro.
    Still { name: "rocky_night", body: Body::Rocky, setup: |s| {
        s.light_azimuth = 2.3;
        s.light_elevation = 0.1;
    } },
    // Cobertura de nubes.
    Still { name: "rocky_clouds_low", body: Body::Rocky, setup: |s| s.params.rocky.cloud_coverage = 0.2 },
    Still { name: "rocky_clouds_high", body: Body::Rocky, setup: |s| s.params.rocky.cloud_coverage = 0.75 },
    // Vórtices: las bandas sin y con tormentas.
    Still { name: "gas_storms_off", body: Body::GasGiant, setup: |s| {
        s.time = 2.0;
        s.params.gas.storm_strength = 0.0;
    } },
    Still { name: "gas_storms_on", body: Body::GasGiant, setup: |s| s.time = 2.0 },
    Still { name: "gas_storm_closeup", body: Body::GasGiant, setup: |s| {
        s.time = 2.0;
        s.camera.distance = 1.9;
        s.camera.yaw = 0.3;
        s.camera.pitch = -0.36;
        s.light_azimuth = 0.6;
    } },
    Still { name: "star_no_spots", body: Body::Star, setup: |s| {
        s.time = 4.0;
        s.params.star.spot_amount = 0.0;
    } },
];

const CLIPS: &[Clip] = &[
    // Material animado: la estrella hierve (la geometría no se mueve).
    Clip { name: "star_boil", body: Body::Star, frames: 60, fps: 15.0, setup: none, update: no_update },
    // Nubes a otra velocidad que el terreno (acelerado ×6 para que se note en un GIF corto).
    Clip { name: "rocky_clouds", body: Body::Rocky, frames: 60, fps: 15.0, setup: |s| {
        s.params.rocky.terrain_speed = 0.0;
        s.params.rocky.cloud_speed = 0.5;
        s.params.rocky.city_lights = false;
    }, update: no_update },
    // La luz da una vuelta: sombras de nubes, terminador, ciudades y reflejo del océano se mueven.
    Clip { name: "rocky_light_orbit", body: Body::Rocky, frames: 72, fps: 12.0, setup: |s| {
        s.paused = true;
        s.light_elevation = 0.2;
    }, update: |s, i| s.light_azimuth = 0.2 + i as f32 / 72.0 * std::f32::consts::TAU },
    // Sombras de nubes con luz rasante: la luz se mueve un poco y las sombras se desplazan.
    Clip { name: "rocky_cloud_shadows", body: Body::Rocky, frames: 48, fps: 12.0, setup: |s| {
        grazing_light(s);
        s.paused = true;
    }, update: |s, i| s.light_azimuth = 0.9 + 0.7 * i as f32 / 48.0 },
    // Relieve con luz que se mueve: las sombras de las montañas cambian de lado.
    Clip { name: "rocky_bump", body: Body::Rocky, frames: 48, fps: 12.0, setup: |s| {
        grazing_light(s);
        s.paused = true;
        s.params.rocky.clouds = false;
    }, update: |s, i| s.light_azimuth = 0.3 + 1.3 * i as f32 / 48.0 },
    // La cámara orbita: el reflejo especular sigue a la vista, la tierra no cambia.
    Clip { name: "rocky_specular_orbit", body: Body::Rocky, frames: 48, fps: 12.0, setup: |s| {
        s.paused = true;
        s.light_azimuth = 0.0;
        s.params.rocky.clouds = false;
    }, update: |s, i| s.camera.yaw = -0.6 + 1.2 * i as f32 / 48.0 },
    // Bandas que fluyen y tormentas.
    Clip { name: "gas_flow", body: Body::GasGiant, frames: 72, fps: 15.0, setup: |s| {
        s.params.gas.flow_speed = 2.0;
    }, update: no_update },
];

/// Copia el framebuffer a una imagen de raylib y la guarda como PNG.
fn save_png(fb: &Framebuffer, path: &Path) {
    let mut image = Image::gen_image_color(WIDTH as i32, HEIGHT as i32, RlColor::BLACK);
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let c = fb.get_pixel(x, y);
            image.draw_pixel(x as i32, y as i32, RlColor::new(c.r, c.g, c.b, c.a));
        }
    }
    image.export_image(path.to_str().expect("ruta no válida"));
}

fn render_scene(fb: &mut Framebuffer, mesh: &Mesh, scene: &Scene) {
    fb.clear();
    render(fb, mesh, &scene.uniforms(fb.width, fb.height), scene.body);
}

/// Genera `dir/img/*.png` y, si `clips` es verdadero, `dir/frames/<clip>/*.png`. `only` filtra
/// por nombre.
pub fn export_all(dir: &str, mesh: &Mesh, model_scale: f32, only: Option<&str>, clips: bool) {
    raylib::core::logging::set_trace_log(raylib::prelude::TraceLogLevel::LOG_WARNING);
    let mut fb = Framebuffer::new(WIDTH, HEIGHT);
    let wanted = |name: &str| only.is_none_or(|o| name.contains(o));

    let img_dir = Path::new(dir).join("img");
    std::fs::create_dir_all(&img_dir).expect("no se pudo crear el directorio");
    for still in STILLS.iter().filter(|s| wanted(s.name)) {
        let mut scene = Scene::new(still.body, model_scale);
        (still.setup)(&mut scene);
        render_scene(&mut fb, mesh, &scene);
        let path = img_dir.join(format!("{}.png", still.name));
        save_png(&fb, &path);
        println!("{}", path.display());
    }

    for clip in CLIPS.iter().filter(|c| clips && wanted(c.name)) {
        let clip_dir = Path::new(dir).join("frames").join(clip.name);
        let _ = std::fs::remove_dir_all(&clip_dir);
        std::fs::create_dir_all(&clip_dir).expect("no se pudo crear el directorio");
        let mut scene = Scene::new(clip.body, model_scale);
        (clip.setup)(&mut scene);
        for i in 0..clip.frames {
            (clip.update)(&mut scene, i);
            render_scene(&mut fb, mesh, &scene);
            save_png(&fb, &clip_dir.join(format!("{i:04}.png")));
            scene.advance(1.0 / clip.fps);
        }
        // El script de GIF lee la velocidad de aquí.
        std::fs::write(clip_dir.join("fps.txt"), format!("{}\n", clip.fps)).expect("no se pudo escribir fps.txt");
        println!("{} ({} cuadros a {} fps)", clip_dir.display(), clip.frames, clip.fps);
    }
}
