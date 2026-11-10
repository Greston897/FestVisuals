//! FestVisuals — native launcher for the 26.2 client.
//!
//! Linux: `cargo build --release` (gcc, libfreetype, and libX11 to open a window).
//! Windows: the GitHub workflow builds FreeType and links it statically.
//! Without a display, `festvisuals --snapshot out.png` paints one frame.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod engine;
mod font;
mod netutil;
mod paint;
mod ui;
mod zip;
#[cfg(unix)]
mod x11;
#[cfg(windows)]
mod win32;

#[cfg(unix)]
use x11 as host;
#[cfg(windows)]
use win32 as host;

use std::io::{Read, Write};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use engine::{Engine, EngineEvent, LaunchRequest};
use paint::{Canvas, Image};
use ui::{Key, Ui, UiAction};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 820;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let path = args.get(i + 1).map(|s| s.as_str()).unwrap_or("preview.png");
        let page = args.iter().position(|a| a == "--page").and_then(|i| args.get(i + 1)).map(|s| s.as_str()).unwrap_or("home");
        if let Err(error) = snapshot(std::path::Path::new(path), page) {
            report(&error);
            std::process::exit(1);
        }
        return;
    }
    if headless() {
        let path = std::path::Path::new("preview.png");
        match snapshot(path, "home") {
            Ok(()) => eprintln!("нет графической сессии, кадр записан в {}", path.display()),
            Err(error) => {
                report(&error);
                std::process::exit(1);
            }
        }
        return;
    }
    if let Err(error) = run() {
        report(&error);
        std::process::exit(1);
    }
}

fn report(error: &str) {
    eprintln!("{error}");
    #[cfg(windows)]
    host::alert(error);
}

fn headless() -> bool {
    #[cfg(windows)]
    { false }
    #[cfg(not(windows))]
    { std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() }
}

fn snapshot(path: &std::path::Path, page: &str) -> Result<(), String> {
    let mut ui = boot()?;
    let mut canvas = Canvas::new(WIDTH as i32, HEIGHT as i32);
    ui.frame(6.2);
    ui.open_page(page);
    ui.frame(0.5);
    ui.paint(&mut canvas);
    ui.pointer(390.0, 700.0);
    ui.paint(&mut canvas);
    let ppm = path.with_extension("ppm");
    canvas.write_ppm(&ppm).map_err(|e| e.to_string())?;
    if path.extension().and_then(|s| s.to_str()) == Some("png") {
        let status = std::process::Command::new("convert").arg(&ppm).arg(path).status().map_err(|e| e.to_string())?;
        if !status.success() { return Err("convert не смог записать png".into()); }
        let _ = std::fs::remove_file(ppm);
    }
    Ok(())
}

fn boot() -> Result<Ui, String> {
    let font = font::Font::open(include_bytes!("../assets/Manrope.ttf")).ok_or("не открылся шрифт Manrope")?;
    let backdrop = Image::rgba(include_bytes!("../assets/world.rgba"), 1280, 820).ok_or("нет фона")?;
    let icon = Image::rgba(include_bytes!("../assets/icon.rgba"), 96, 96).ok_or("нет иконки")?;
    let mut ui = Ui::new(font, backdrop, icon);
    ui.warm();
    Ok(ui)
}

fn run() -> Result<(), String> {
    let (sender, events) = mpsc::channel();
    let engine = Arc::new(Engine::new(sender.clone()));
    let engine_http = Arc::clone(&engine);
    std::thread::spawn(move || control_server(engine_http));
    let engine_boot = Arc::clone(&engine);
    std::thread::spawn(move || {
        let settings = engine_boot.load_settings();
        let _ = sender.send(EngineEvent::Ready(settings));
    });

    let mut window = host::Window::open("FestVisuals", WIDTH, HEIGHT)?;
    let mut canvas = Canvas::new(WIDTH as i32, HEIGHT as i32);
    canvas.fill([7, 6, 11, 255]);
    window.present(&canvas);
    let mut ui = boot()?;
    ui.set_settings(engine.load_settings());
    ui.set_instant(engine.instant_ready());
    let mut last = Instant::now();
    let mut alive = true;
    let mut jarvis = false;
    while alive {
        while let Ok(event) = events.try_recv() {
            if matches!(event, EngineEvent::Ready(_)) {
                ui.set_instant(engine.instant_ready());
            }
            ui.apply_event(event);
        }
        while let Some(ev) = window.poll() {
            match ev {
                host::Ev::Close => alive = false,
                host::Ev::Resize { w, h } => canvas.resize(w as i32, h as i32),
                host::Ev::Expose => {}
                host::Ev::Mouse { x, y, pressed } => {
                    ui.pointer(x, y);
                    match pressed {
                        Some(true) => {
                            if ui.hit_test((x, y)) && !ui.over_control((x, y)) {
                                window.begin_move(x, y);
                            } else {
                                let action = ui.pointer_down(x, y);
                                dispatch(&engine, &mut ui, &mut window, action);
                            }
                        }
                        Some(false) => {
                            let action = ui.pointer_up(x, y);
                            dispatch(&engine, &mut ui, &mut window, action);
                        }
                        None => {}
                    }
                }
                host::Ev::Key { text, sym } => {
                    let action = match sym {
                        0xff08 => ui.key(Key::Backspace),
                        0xff0d | 0xff8d => ui.key(Key::Enter),
                        0xff1b => ui.key(Key::Escape),
                        0xff51 => ui.key(Key::Left),
                        0xff53 => ui.key(Key::Right),
                        _ if !text.is_empty() => ui.key(Key::Text(text)),
                        _ => UiAction::None,
                    };
                    dispatch(&engine, &mut ui, &mut window, action);
                }
            }
        }
        if ui.concealed() {
            // The game has the screen; the next Closed event maps us again by continuing the loop.
        }
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.05);
        last = now;
        ui.frame(dt);
        let (w, h) = window.size();
        if canvas.w != w as i32 || canvas.h != h as i32 {
            canvas.resize(w as i32, h as i32);
        }
        ui.paint(&mut canvas);
        window.present(&canvas);
        if !jarvis {
            jarvis = true;
            let voice = Arc::clone(&engine);
            std::thread::spawn(move || voice.jarvis_start());
        }
        let spent = last.elapsed();
        if spent < Duration::from_millis(33) {
            std::thread::sleep(Duration::from_millis(33) - spent);
        }
    }
    engine.jarvis_stop();
    Ok(())
}

fn dispatch(engine: &Arc<Engine>, ui: &mut Ui, window: &mut host::Window, action: UiAction) {
    match action {
        UiAction::None => {}
        UiAction::Quit => std::process::exit(0),
        UiAction::Minimize => window.minimize(),
        UiAction::Launch(request) => {
            let engine = Arc::clone(engine);
            std::thread::spawn(move || engine.launch(request));
        }
        UiAction::Stop => engine.stop(),
        UiAction::OpenFolder => open_path(&engine.instance_dir("26.2")),
        UiAction::OpenRoot => open_path(&engine.root_dir()),
        UiAction::PickDirectory => {
            if let Some(path) = pick(true) {
                let engine = Arc::clone(engine);
                if let Err(error) = engine.set_root(&path) {
                    ui.set_note(error);
                } else {
                    ui.apply_event(EngineEvent::RootChanged(path));
                }
            } else {
                ui.set_note("Выбор папки недоступен в этой сессии".into());
            }
        }
        UiAction::PickAvatar => {
            if let Some(path) = pick(false) {
                if let Some(saved) = engine.import_avatar(&path) {
                    ui.set_avatar(saved.clone());
                    let mut settings = engine.load_settings();
                    settings.avatar = saved;
                    ui.set_settings(settings);
                }
            }
        }
        UiAction::SaveSettings(settings) => engine.save_settings(&settings),
    }
}

fn open_path(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
}

fn pick(directory: bool) -> Option<String> {
    #[cfg(windows)]
    { return host::pick_path(directory); }
    #[cfg(not(windows))]
    {
        let attempts: &[(&str, &str)] = if directory {
            &[("--file-selection", "--directory"), ("--getexistingdirectory", "")]
        } else {
            &[("--file-selection", ""), ("--getopenfilename", "")]
        };
        for (flag, extra) in attempts {
            let program = if flag.starts_with("--get") { "kdialog" } else { "zenity" };
            let mut command = std::process::Command::new(program);
            command.arg(flag);
            if !extra.is_empty() { command.arg(extra); }
            if let Ok(output) = command.output() {
                if output.status.success() {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() { return Some(path); }
                }
            }
        }
        None
    }
}

fn control_server(engine: Arc<Engine>) {
    let listener = match std::net::TcpListener::bind("127.0.0.1:4567") {
        Ok(listener) => listener,
        Err(_) => return,
    };
    for mut stream in listener.incoming().flatten() {
        let mut buf = [0u8; 512];
        let n = stream.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]);
        let path = req.split_whitespace().nth(1).unwrap_or("");
        let (code, body) = match path {
            "/start-jarvis" => { engine.jarvis_start(); ("200 OK", "starting") }
            "/stop-jarvis" => { engine.jarvis_stop(); ("200 OK", "stopped") }
            _ => ("404 Not Found", ""),
        };
        let resp = format!("HTTP/1.0 {code}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
        let _ = stream.write_all(resp.as_bytes());
    }
}

#[allow(dead_code)]
fn _launch_type(request: LaunchRequest) { let _ = request; }
