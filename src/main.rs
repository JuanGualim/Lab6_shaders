//! Punto de entrada: ventana interactiva o exportación de capturas sin ventana.

use std::time::Instant;

use lab6_shaders::export::export_all;
use lab6_shaders::framebuffer::{Framebuffer, HEIGHT, WIDTH};
use lab6_shaders::mesh::Mesh;
use lab6_shaders::pipeline::{RenderStats, render};
use lab6_shaders::scene::{Scene, param_list};
use lab6_shaders::shaders::{Body, Params};
use raylib::prelude::*;

const MODEL_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/sphere.obj");

const HELP: &str = "\
lab6_shaders — planetas y estrellas con shaders en CPU

USO:
    cargo run --release [-- opciones]

OPCIONES:
    (sin opciones)          Abre la ventana con la estrella
    --body star|rocky|gas   Cuerpo con el que arranca la ventana
    --export DIR            Sin ventana: guarda las capturas en DIR/img y los cuadros de los
                            clips en DIR/frames/<clip> (los GIF se arman con scripts/make_gifs.sh)
        --only TEXTO        Solo las capturas/clips cuyo nombre contiene TEXTO
        --stills            Solo las capturas (sin los cuadros de los clips)
    --bench                 Mide el tiempo por cuadro de cada cuerpo (sin ventana)
    --help                  Muestra esta ayuda

CONTROLES:
    1 / 2 / 3               Estrella / planeta rocoso / gigante gaseoso
    Arrastrar (raton) o flechas   Orbitar la camara
    Rueda o W / S           Acercar / alejar
    J / L, I / K            Mover la luz (azimut, elevacion);  O: la luz orbita sola
    Tab / Shift+Tab         Elegir parametro del shader
    Q / E                   Bajar / subir el parametro (o encender/apagar)
    R                       Restaurar los parametros del cuerpo
    Espacio                 Pausar la animacion
    F                       Media resolucion (mas rapido) / resolucion completa
    P                       Captura en screenshots/
    H                       Ocultar el panel
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value_of = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1).cloned())
    };

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{HELP}");
        return;
    }

    let mesh = Mesh::load(MODEL_PATH).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1);
    });
    // El OBJ tiene radio ~0.87: se escala a 1 para que los shaders trabajen en la esfera unitaria.
    let model_scale = 1.0 / mesh.bounding_radius();

    if let Some(dir) = value_of("--export") {
        let clips = !args.iter().any(|a| a == "--stills");
        export_all(&dir, &mesh, model_scale, value_of("--only").as_deref(), clips);
        return;
    }
    if args.iter().any(|a| a == "--bench") {
        bench(&mesh, model_scale);
        return;
    }

    let body = match value_of("--body") {
        Some(slug) => Body::from_slug(&slug).unwrap_or_else(|| {
            eprintln!("cuerpo desconocido {slug:?}: usa star, rocky o gas");
            std::process::exit(1);
        }),
        None => Body::Star,
    };
    run_window(&mesh, Scene::new(body, model_scale));
}

fn run_window(mesh: &Mesh, mut scene: Scene) {
    let (mut rl, thread) = raylib::init()
        .size(WIDTH as i32, HEIGHT as i32)
        .title("Lab 6 - Planetas y estrellas con shaders")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();
    rl.set_target_fps(60);

    // `scale` = 1: resolución completa; 2: se renderiza a la mitad y la textura se estira (4 veces
    // menos fragmentos, para máquinas lentas).
    let mut scale = 1usize;
    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    let mut texture = new_texture(&mut rl, &thread, WIDTH, HEIGHT);

    let mut selected = 0usize;
    let mut show_hud = true;
    let mut render_ms = 0.0f64;
    let mut screenshot_count = 0;

    while !rl.window_should_close() {
        let dt = rl.get_frame_time().min(0.1);

        // --- Entrada ------------------------------------------------------------------------
        for (key, body) in [
            (KeyboardKey::KEY_ONE, Body::Star),
            (KeyboardKey::KEY_TWO, Body::Rocky),
            (KeyboardKey::KEY_THREE, Body::GasGiant),
        ] {
            if rl.is_key_pressed(key) && scene.body != body {
                scene.body = body;
                selected = 0;
            }
        }

        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            let delta = rl.get_mouse_delta();
            scene.camera.orbit(-delta.x * 0.008, delta.y * 0.008);
        }
        let orbit_speed = 1.5 * dt;
        if rl.is_key_down(KeyboardKey::KEY_LEFT) {
            scene.camera.orbit(-orbit_speed, 0.0);
        }
        if rl.is_key_down(KeyboardKey::KEY_RIGHT) {
            scene.camera.orbit(orbit_speed, 0.0);
        }
        if rl.is_key_down(KeyboardKey::KEY_UP) {
            scene.camera.orbit(0.0, orbit_speed);
        }
        if rl.is_key_down(KeyboardKey::KEY_DOWN) {
            scene.camera.orbit(0.0, -orbit_speed);
        }
        let wheel = rl.get_mouse_wheel_move();
        if wheel != 0.0 {
            scene.camera.zoom(1.0 - wheel * 0.1);
        }
        if rl.is_key_down(KeyboardKey::KEY_W) {
            scene.camera.zoom(1.0 - dt);
        }
        if rl.is_key_down(KeyboardKey::KEY_S) {
            scene.camera.zoom(1.0 + dt);
        }

        let light_speed = 1.2 * dt;
        if rl.is_key_down(KeyboardKey::KEY_J) {
            scene.light_azimuth -= light_speed;
        }
        if rl.is_key_down(KeyboardKey::KEY_L) {
            scene.light_azimuth += light_speed;
        }
        if rl.is_key_down(KeyboardKey::KEY_I) {
            scene.light_elevation = (scene.light_elevation + light_speed).min(1.5);
        }
        if rl.is_key_down(KeyboardKey::KEY_K) {
            scene.light_elevation = (scene.light_elevation - light_speed).max(-1.5);
        }
        if rl.is_key_pressed(KeyboardKey::KEY_O) {
            scene.light_orbit = !scene.light_orbit;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_SPACE) {
            scene.paused = !scene.paused;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_H) {
            show_hud = !show_hud;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_F) {
            scale = 3 - scale;
            framebuffer = Framebuffer::new(WIDTH / scale, HEIGHT / scale);
            texture = new_texture(&mut rl, &thread, WIDTH / scale, HEIGHT / scale);
            render_ms = 0.0;
        }
        if rl.is_key_pressed(KeyboardKey::KEY_R) {
            let defaults = Params::default();
            match scene.body {
                Body::Star => scene.params.star = defaults.star,
                Body::Rocky => scene.params.rocky = defaults.rocky,
                Body::GasGiant => scene.params.gas = defaults.gas,
            }
        }

        {
            let mut params = param_list(scene.body, &mut scene.params);
            let count = params.len();
            if rl.is_key_pressed(KeyboardKey::KEY_TAB) {
                let shift = rl.is_key_down(KeyboardKey::KEY_LEFT_SHIFT);
                selected = if shift { (selected + count - 1) % count } else { (selected + 1) % count };
            }
            if rl.is_key_pressed(KeyboardKey::KEY_E) || rl.is_key_pressed_repeat(KeyboardKey::KEY_E) {
                params[selected].adjust(1.0);
            }
            if rl.is_key_pressed(KeyboardKey::KEY_Q) || rl.is_key_pressed_repeat(KeyboardKey::KEY_Q) {
                params[selected].adjust(-1.0);
            }
        }

        scene.advance(dt);

        // --- Render en CPU ------------------------------------------------------------------
        framebuffer.clear();
        let start = Instant::now();
        let uniforms = scene.uniforms(framebuffer.width, framebuffer.height);
        let stats = render(&mut framebuffer, mesh, &uniforms, scene.body);
        let elapsed = start.elapsed().as_secs_f64() * 1e3;
        render_ms = if render_ms == 0.0 { elapsed } else { render_ms * 0.9 + elapsed * 0.1 };
        texture
            .update_texture(&framebuffer.to_rgba_bytes())
            .expect("tamano de textura incorrecto");

        let take_screenshot = rl.is_key_pressed(KeyboardKey::KEY_P);

        // --- Dibujo -------------------------------------------------------------------------
        let fps = rl.get_fps();
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(Color::BLACK);
        d.draw_texture_ex(&texture, Vector2::zero(), 0.0, scale as f32, Color::WHITE);
        if show_hud {
            draw_hud(&mut d, &mut scene, selected, stats, render_ms, fps, scale);
        }
        drop(d);

        if take_screenshot {
            std::fs::create_dir_all("screenshots").expect("no se pudo crear screenshots/");
            let path = format!("screenshots/{}_{screenshot_count:02}.png", scene.body.slug());
            rl.load_image_from_screen(&thread).export_image(&path);
            println!("captura: {path}");
            screenshot_count += 1;
        }
    }
}

fn new_texture(rl: &mut RaylibHandle, thread: &RaylibThread, width: usize, height: usize) -> Texture2D {
    let blank = Image::gen_image_color(width as i32, height as i32, Color::BLACK);
    rl.load_texture_from_image(thread, &blank)
        .expect("no se pudo crear la textura")
}

fn draw_hud(
    d: &mut RaylibDrawHandle,
    scene: &mut Scene,
    selected: usize,
    stats: RenderStats,
    render_ms: f64,
    fps: u32,
    scale: usize,
) {
    let accent = Color::new(255, 200, 120, 255);
    let body = scene.body;
    let time = scene.time;
    let paused = scene.paused;
    let light_orbit = scene.light_orbit;
    let params = param_list(body, &mut scene.params);

    let panel_height = 78 + params.len() as i32 * 14;
    d.draw_rectangle(8, 8, 300, panel_height, Color::new(0, 0, 0, 160));
    d.draw_text(body.name(), 16, 14, 20, Color::WHITE);
    d.draw_text(
        &format!(
            "{fps} fps | render {render_ms:.1} ms | {} fragmentos{}",
            stats.fragments,
            if scale > 1 { " | 1/2 res." } else { "" }
        ),
        16,
        38,
        10,
        Color::LIGHTGRAY,
    );
    d.draw_text(
        &format!(
            "t = {time:.1} s{}{}",
            if paused { "  (pausa)" } else { "" },
            if light_orbit { "  luz orbitando" } else { "" }
        ),
        16,
        52,
        10,
        Color::LIGHTGRAY,
    );
    for (i, param) in params.iter().enumerate() {
        let (mark, color) = if i == selected { (">", accent) } else { (" ", Color::WHITE) };
        d.draw_text(&format!("{mark} {}", param.label()), 16, 70 + i as i32 * 14, 10, color);
    }

    d.draw_rectangle(0, HEIGHT as i32 - 22, WIDTH as i32, 22, Color::new(0, 0, 0, 160));
    d.draw_text(
        "1/2/3 cuerpo | raton/flechas camara | J/L I/K luz, O orbita | Tab/Q/E parametros | R reset | Espacio pausa | F 1/2 res. | P captura | H panel",
        8,
        HEIGHT as i32 - 16,
        10,
        Color::GRAY,
    );
}

/// Tiempo medio de un cuadro de cada cuerpo con la vista por defecto.
fn bench(mesh: &Mesh, model_scale: f32) {
    const FRAMES: u32 = 20;
    let mut framebuffer = Framebuffer::new(WIDTH, HEIGHT);
    println!("| Cuerpo | Fragmentos | ms por cuadro | fps |");
    println!("|---|---:|---:|---:|");
    for body in Body::ALL {
        let mut scene = Scene::new(body, model_scale);
        let mut fragments = 0;
        let start = Instant::now();
        for _ in 0..FRAMES {
            framebuffer.clear();
            fragments = render(&mut framebuffer, mesh, &scene.uniforms(WIDTH, HEIGHT), body).fragments;
            scene.advance(1.0 / 30.0);
        }
        let ms = start.elapsed().as_secs_f64() * 1e3 / FRAMES as f64;
        println!("| {} | {fragments} | {ms:.1} | {:.1} |", body.name(), 1000.0 / ms);
    }
}
