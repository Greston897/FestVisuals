//! From "Играть" to a running 26.2: Java, Fabric, mods, vanilla files, then the
//! game process. Mirrors launcher/electron/game.js so an install made by the
//! Electron launcher is reused and the other way round.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::netutil::{self, json::Value};

pub const VERSION: &str = "26.2";
const LOADER: &str = "0.19.5";
const MODS: &[&str] = &["fabric-api", "viafabricplus"];
const VERSION_MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const JAVA_RUNTIMES: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const RESOURCES: &str = "https://resources.download.minecraft.net";
const LIBRARIES: &str = "https://libraries.minecraft.net";
const FEED: &str = "https://github.com/Daniiltoptl/FestVisuals-Releases/releases/download/mod/";
const FULL_CHECK_EVERY_MS: u128 = 7 * 24 * 60 * 60 * 1000;
const REFRESH_EVERY_MS: u128 = 12 * 60 * 60 * 1000;
const PARALLEL: usize = 32;

const PHASES: &[(&str, &str)] = &[
    ("festvisuals", "FestVisuals"),
    ("java", "Java"),
    ("mods", "Моды"),
    ("version-jar", "Клиент"),
    ("assets", "Ресурсы"),
    ("classes", "Библиотеки"),
    ("natives", "Библиотеки"),
    ("instant", "Мгновенный запуск"),
];

#[derive(Clone, Debug)]
pub struct Settings {
    pub ram: u32,
    pub root: String,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub close_on_launch: bool,
    pub username: String,
    pub avatar: String,
    pub last_launch: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ram: 4096,
            root: String::new(),
            width: 1920,
            height: 1080,
            fullscreen: false,
            close_on_launch: false,
            username: "FestPlayer".into(),
            avatar: String::new(),
            last_launch: String::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LaunchRequest {
    pub version: String,
    pub ram: u32,
    pub username: String,
    pub root: String,
    pub close_on_launch: bool,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
}

#[derive(Clone, Debug)]
pub enum EngineEvent {
    Ready(Settings),
    RootChanged(String),
    Progress { label: String, percent: u8 },
    Starting,
    Running,
    Closed,
    Error(String),
}

pub struct Engine {
    sender: Sender<EngineEvent>,
    cancel: AtomicBool,
    child: Mutex<Option<Child>>,
    jarvis: Mutex<Option<Child>>,
}

impl Engine {
    pub fn new(sender: Sender<EngineEvent>) -> Self {
        Self { sender, cancel: AtomicBool::new(false), child: Mutex::new(None), jarvis: Mutex::new(None) }
    }

    pub fn root_dir(&self) -> PathBuf {
        usable_root(&self.load_settings().root)
    }

    pub fn instance_dir(&self, version: &str) -> PathBuf {
        self.root_dir().join("instances").join(version)
    }

    pub fn load_settings(&self) -> Settings {
        let mut settings = Settings::default();
        settings.root = default_root().display().to_string();
        let path = config_path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(value) = netutil::json::parse(&text) {
                if let Some(v) = value.get("ram").and_then(|v| v.as_u64()) { settings.ram = v as u32; }
                if let Some(v) = value.get("root").and_then(|v| v.as_str()) { if !v.is_empty() { settings.root = v.to_string(); } }
                if let Some(v) = value.get("width").and_then(|v| v.as_u64()) { settings.width = v as u32; }
                if let Some(v) = value.get("height").and_then(|v| v.as_u64()) { settings.height = v as u32; }
                if let Some(v) = value.get("fullscreen").and_then(|v| v.as_bool()) { settings.fullscreen = v; }
                if let Some(v) = value.get("close_on_launch").and_then(|v| v.as_bool()) { settings.close_on_launch = v; }
                if let Some(v) = value.get("username").and_then(|v| v.as_str()) { settings.username = v.to_string(); }
                if let Some(v) = value.get("avatar").and_then(|v| v.as_str()) { settings.avatar = v.to_string(); }
                if let Some(v) = value.get("last_launch").and_then(|v| v.as_str()) { settings.last_launch = v.to_string(); }
            }
        }
        settings
    }

    pub fn save_settings(&self, settings: &Settings) {
        if let Some(parent) = config_path().parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let text = format!(
            "{{\n  \"ram\": {},\n  \"root\": {},\n  \"width\": {},\n  \"height\": {},\n  \"fullscreen\": {},\n  \"close_on_launch\": {},\n  \"username\": {},\n  \"avatar\": {},\n  \"last_launch\": {}\n}}\n",
            settings.ram,
            netutil::json::escape(&settings.root),
            settings.width,
            settings.height,
            settings.fullscreen,
            settings.close_on_launch,
            netutil::json::escape(&settings.username),
            netutil::json::escape(&settings.avatar),
            netutil::json::escape(&settings.last_launch),
        );
        std::fs::write(config_path(), text).ok();
    }

    pub fn set_root(&self, path: &str) -> Result<(), String> {
        let root = usable_root(path);
        let mut settings = self.load_settings();
        settings.root = root.display().to_string();
        self.save_settings(&settings);
        Ok(())
    }

    pub fn import_avatar(&self, source: &str) -> Option<String> {
        let bytes = std::fs::read(source).ok()?;
        let dest = config_path().parent()?.join("avatar.png");
        std::fs::create_dir_all(dest.parent()?).ok()?;
        std::fs::write(&dest, bytes).ok()?;
        let saved = dest.display().to_string();
        let mut settings = self.load_settings();
        settings.avatar = saved.clone();
        self.save_settings(&settings);
        Some(saved)
    }

    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        if let Ok(mut guard) = self.child.lock() {
            if let Some(child) = guard.as_mut() {
                let _ = child.kill();
            }
        }
    }

    pub fn launch(&self, request: LaunchRequest) {
        self.cancel.store(false, Ordering::Relaxed);
        if let Err(error) = self.launch_inner(&request) {
            self.emit(EngineEvent::Error(error));
            self.emit(EngineEvent::Closed);
        }
    }

    fn launch_inner(&self, request: &LaunchRequest) -> Result<(), String> {
        if request.version != VERSION {
            return Err(format!("Неизвестная версия {}", request.version));
        }
        let root = usable_root(&request.root);
        migrate_legacy_layout(&root)?;
        let game_dir = root.join("instances").join(VERSION);
        std::fs::create_dir_all(&game_dir).map_err(|e| e.to_string())?;

        let (java, plan) = if let Some(hit) = load_instant(&root).filter(|plan| instant_usable(&root, plan)) {
            self.progress("instant", 100);
            plan_from_instant(hit)
        } else {
            let version_json = ensure_version_json(&root)?;
            self.check()?;
            let component = version_json.get("javaVersion").and_then(|v| v.get("component")).and_then(|v| v.as_str()).unwrap_or("jre-legacy");
            let java = ensure_java(&root, component, &self.progress_sink("java"))?;
            self.check()?;
            let festvisuals = latest_festvisuals(&root, &self.progress_sink("festvisuals"))?;
            ensure_fabric_profile(&root)?;
            ensure_mods(&game_dir, festvisuals.as_deref(), &self.progress_sink("mods"))?;
            self.check()?;

            let removed = remove_corrupt_jars(&root);
            if removed > 0 {
                self.progress("classes", 0);
            }
            let fast = verified_recently(&root);
            let plan = prepare_vanilla(&root, &version_json, fast, &self.progress_sink("assets"), &self.progress_sink("classes"))?;
            self.check()?;
            save_instant(&root, &java, &plan);
            (java, plan)
        };

        let mut args = Vec::new();
        args.push(format!("-Xmx{}M", request.ram.max(512)));
        args.push(format!("-Xms{}M", (request.ram / 2).max(512)));
        if cfg!(target_os = "windows") {
            args.push("-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump".into());
        } else if cfg!(target_os = "linux") {
            args.push("-Xss1M".into());
        } else if cfg!(target_os = "macos") {
            args.push("-XstartOnFirstThread".into());
        }
        args.push("-Doshi.os.windows.perfproc.disabled=true".into());
        args.push("-Doshi.util.wmi.timeout=2000".into());
        if let Some(dir) = socket_tmp_dir() {
            args.push(format!("-Djdk.net.unixdomain.tmpdir={}", dir.display()));
        }
        args.push("-Djava.library.path".into());
        // -Djava.library.path=<natives> is one argument.
        args.pop();
        args.push(format!("-Djava.library.path={}", plan.natives.display()));
        args.push("-cp".into());
        let game = plan.game_args(&game_dir, &root, request);
        args.push(plan.classpath);
        args.push(plan.main_class);
        args.extend(game);

        let mut command = Command::new(&java);
        command.args(&args).current_dir(&game_dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|e| format!("Не удалось запустить Java: {e}"))?;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        self.emit(EngineEvent::Starting);
        if let Ok(mut guard) = self.child.lock() {
            *guard = Some(child);
        }

        let (tx, rx) = std::sync::mpsc::channel::<String>();
        for stream in [stdout.map(|s| Box::new(s) as Box<dyn Read + Send>), stderr.map(|s| Box::new(s) as Box<dyn Read + Send>)].into_iter().flatten() {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stream);
                for line in reader.lines().map_while(Result::ok) {
                    if tx.send(line).is_err() { break; }
                }
            });
        }
        drop(tx);

        let mut window_open = false;
        let mut last_lines = Vec::new();
        let mut exit_code = None;
        loop {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(line) => {
                    if last_lines.len() == 20 { last_lines.remove(0); }
                    last_lines.push(line.clone());
                    if !window_open && (line.contains("Backend library:") || line.contains("Sound engine started")) {
                        window_open = true;
                        let _ = std::fs::write(root.join(".festvisuals-verified"), now_ms().to_string());
                        let mut settings = self.load_settings();
                        settings.last_launch = now_ms().to_string();
                        self.save_settings(&settings);
                        self.emit(EngineEvent::Running);
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if let Some(code) = self.poll_exit() {
                        exit_code = Some(code);
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    exit_code = self.poll_exit();
                    break;
                }
            }
        }
        while let Ok(line) = rx.try_recv() {
            last_lines.push(line);
        }
        let code = exit_code.or_else(|| self.poll_exit());
        if !window_open {
            let _ = std::fs::remove_file(root.join(".festvisuals-verified"));
            if code != Some(0) {
                let tail = last_lines.split_off(last_lines.len().saturating_sub(6)).join("\n");
                return Err(format!("Игра закрылась (код {}).\n{tail}", code.unwrap_or(-1)));
            }
        }
        self.emit(EngineEvent::Closed);
        Ok(())
    }

    fn poll_exit(&self) -> Option<i32> {
        let mut guard = self.child.lock().ok()?;
        let child = guard.as_mut()?;
        match child.try_wait() {
            Ok(Some(status)) => {
                let code = status.code().unwrap_or(-1);
                *guard = None;
                Some(code)
            }
            _ => None,
        }
    }

    fn check(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Relaxed) { Err("Запуск отменён".into()) } else { Ok(()) }
    }

    fn emit(&self, event: EngineEvent) {
        let _ = self.sender.send(event);
    }

    fn progress(&self, kind: &str, percent: u8) {
        let label = PHASES.iter().find(|(key, _)| *key == kind).map(|(_, label)| *label).unwrap_or("Подготовка");
        self.emit(EngineEvent::Progress { label: label.into(), percent });
    }

    fn progress_sink(&self, kind: &str) -> impl Fn(u8) + '_ {
        let kind = kind.to_string();
        move |percent| self.progress(&kind, percent)
    }

    /// True when a warm install can skip the network and the jar walk.
    pub fn instant_ready(&self) -> bool {
        let root = usable_root(&self.load_settings().root);
        load_instant(&root).map(|plan| instant_usable(&root, &plan)).unwrap_or(false)
    }

    pub fn jarvis_start(&self) {
        if self.jarvis.lock().ok().and_then(|g| g.as_ref().map(|c| c.id())).is_some() {
            return;
        }
        let dir = jarvis_dir();
        let Some(dir) = dir else { return };
        let python = ["python3", "python"].into_iter().find(|bin| Command::new(bin).arg("--version").output().is_ok());
        let Some(python) = python else { return };
        let mut command = Command::new(python);
        command.arg("server.py").current_dir(&dir).env("JARVIS_HOST", "127.0.0.1").env("PYTHONIOENCODING", "utf-8")
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Ok(child) = command.spawn() {
            if let Ok(mut guard) = self.jarvis.lock() { *guard = Some(child); }
        }
    }

    pub fn jarvis_stop(&self) {
        if let Ok(mut guard) = self.jarvis.lock() {
            if let Some(child) = guard.as_mut() { let _ = child.kill(); }
            *guard = None;
        }
    }
}

pub(crate) fn now_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

fn default_root() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join(".festvisuals");
    }
    directories_home().join(".local").join("share").join(".festvisuals")
}

fn directories_home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn config_path() -> PathBuf {
    directories_home().join(".config").join("festvisuals").join("launcher.json")
}

fn usable_root(requested: &str) -> PathBuf {
    for candidate in [(!requested.is_empty()).then(|| PathBuf::from(requested)), Some(default_root())].into_iter().flatten() {
        if std::fs::create_dir_all(&candidate).is_ok() {
            return candidate;
        }
    }
    default_root()
}

fn profile_id() -> String {
    format!("fabric-loader-{LOADER}-{VERSION}")
}

fn ensure_version_json(root: &Path) -> Result<Value, String> {
    let file = root.join("versions").join(profile_id()).join(format!("{VERSION}.json"));
    if let Some(value) = std::fs::read_to_string(&file).ok().and_then(|text| netutil::json::parse(&text)) {
        if value.get("downloads").is_some() { return Ok(value); }
    }
    let manifest = get_json(VERSION_MANIFEST)?;
    let entry = manifest.get("versions").and_then(|v| v.as_array()).and_then(|list| {
        list.iter().find(|v| v.get("id").and_then(|v| v.as_str()) == Some(VERSION))
    }).ok_or_else(|| format!("Mojang не знает версию {VERSION}"))?;
    let url = entry.get("url").and_then(|v| v.as_str()).ok_or("нет ссылки на версию")?;
    let sha1 = entry.get("sha1").and_then(|v| v.as_str());
    download_checked(url, &file, sha1)?;
    std::fs::read_to_string(&file).ok().and_then(|text| netutil::json::parse(&text)).ok_or_else(|| "пустой манифест версии".into())
}

fn ensure_fabric_profile(root: &Path) -> Result<(), String> {
    let id = profile_id();
    let file = root.join("versions").join(&id).join(format!("{id}.json"));
    if file.exists() { return Ok(()); }
    let url = format!("https://meta.fabricmc.net/v2/versions/loader/{VERSION}/{LOADER}/profile/json");
    let response = netutil::get(&url).map_err(|e| e.to_string())?;
    if !response.ok() { return Err(format!("Fabric: HTTP {}", response.status)); }
    netutil::json::parse(&response.text()).ok_or("Fabric прислал не JSON")?;
    if let Some(parent) = file.parent() { std::fs::create_dir_all(parent).ok(); }
    std::fs::write(file, response.body).map_err(|e| e.to_string())
}

fn ensure_java(root: &Path, component: &str, progress: &dyn Fn(u8)) -> Result<PathBuf, String> {
    if !cfg!(target_os = "windows") {
        return Ok(PathBuf::from("java"));
    }
    let home = root.join("runtime").join(component);
    let javaw = home.join("bin").join("javaw.exe");
    if home.join(".complete").exists() && javaw.exists() { return Ok(javaw); }
    let platform = if std::env::consts::ARCH == "aarch64" { "windows-arm64" } else { "windows-x64" };
    let all = get_json(JAVA_RUNTIMES)?;
    let entry = all.get(platform).and_then(|v| v.get(component)).and_then(|v| v.as_array()).and_then(|a| a.first())
        .or_else(|| all.get("windows-x64").and_then(|v| v.get(component)).and_then(|v| v.as_array()).and_then(|a| a.first()))
        .ok_or_else(|| format!("Нет Java ({component}) для {platform}"))?;
    let manifest_url = entry.get("manifest").and_then(|v| v.get("url")).and_then(|v| v.as_str()).ok_or("нет манифеста Java")?;
    let manifest = get_json(manifest_url)?;
    let files = manifest.get("files").and_then(|v| v.as_object()).map(|items| items.to_vec()).unwrap_or_default();
    let files: Vec<_> = files.into_iter().filter(|(_, file)| file.get("type").and_then(|v| v.as_str()) == Some("file")).collect();
    progress(0);
    let mut finished = 0usize;
    for (relative, file) in &files {
        let raw = file.get("downloads").and_then(|v| v.get("raw"));
        let url = raw.and_then(|v| v.get("url")).and_then(|v| v.as_str()).unwrap_or("");
        let sha1 = raw.and_then(|v| v.get("sha1")).and_then(|v| v.as_str());
        let dest = home.join(relative);
        let size = raw.and_then(|v| v.get("size")).and_then(|v| v.as_u64()).unwrap_or(0);
        if !dest.exists() || std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0) != size {
            download_checked(url, &dest, sha1)?;
        }
        finished += 1;
        progress(((finished * 100) / files.len().max(1)) as u8);
    }
    std::fs::write(home.join(".complete"), component).ok();
    Ok(javaw)
}

struct ResolvedMod {
    filename: String,
    url: String,
    sha1: String,
    size: u64,
    id: String,
}

fn ensure_mods(game_dir: &Path, festvisuals: Option<&Path>, progress: &dyn Fn(u8)) -> Result<(), String> {
    let mods_dir = game_dir.join("mods");
    std::fs::create_dir_all(&mods_dir).ok();
    let state_path = mods_dir.join(".festvisuals-mods.json");
    let mut state = std::fs::read_to_string(&state_path).ok().and_then(|text| netutil::json::parse(&text));
    let resolved_at = state.as_ref().and_then(|v| v.get("resolvedAt")).and_then(|v| v.as_u64()).unwrap_or(0) as u128;
    let mut resolved: Vec<(String, ResolvedMod)> = Vec::new();
    if let Some(state) = &state {
        if let Some(object) = state.get("resolved").and_then(|v| v.as_object()) {
            for (slug, value) in object {
                if let Some(file) = read_resolved(value) { resolved.push((slug.clone(), file)); }
            }
        }
    }
    let stale = now_ms().saturating_sub(resolved_at) > REFRESH_EVERY_MS || MODS.iter().any(|slug| resolved.iter().all(|(s, _)| s != slug));
    if stale {
        for slug in MODS {
            if let Ok(Some(file)) = resolve_latest(slug) {
                if let Some(slot) = resolved.iter_mut().find(|(s, _)| s == slug) { slot.1 = file; } else { resolved.push(((*slug).to_string(), file)); }
            }
        }
    }
    if resolved.iter().all(|(slug, _)| slug != "fabric-api") {
        return Err(format!("Не удалось получить Fabric API для {VERSION} — проверь интернет"));
    }
    let mut keep = Vec::new();
    let mut ids = Vec::new();
    let total = resolved.len().max(1);
    for (index, (_, file)) in resolved.iter_mut().enumerate() {
        let target = mods_dir.join(&file.filename);
        if !target.exists() || std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0) != file.size {
            progress(((index * 100) / total) as u8);
            download_checked(&file.url, &target, Some(&file.sha1))?;
        }
        if file.id.is_empty() { file.id = mod_id(&target).unwrap_or_default(); }
        if !file.id.is_empty() { ids.push(file.id.clone()); }
        keep.push(file.filename.clone());
    }
    progress(100);
    if let Some(state) = &state {
        if let Some(managed) = state.get("managed").and_then(|v| v.as_array()) {
            for name in managed.iter().filter_map(|v| v.as_str()) {
                if !keep.iter().any(|k| k == name) { let _ = std::fs::remove_file(mods_dir.join(name)); }
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(&mods_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".jar") || keep.iter().any(|k| k == &name) || name.to_ascii_lowercase().starts_with("festvisuals") { continue; }
            if let Some(id) = mod_id(&entry.path()) {
                if ids.iter().any(|known| known == &id) { let _ = std::fs::remove_file(entry.path()); }
            }
        }
    }
    if let Some(source) = festvisuals {
        if source.exists() {
            if let Ok(entries) = std::fs::read_dir(&mods_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.to_ascii_lowercase().starts_with("festvisuals") && name != "festvisuals.jar" { let _ = std::fs::remove_file(entry.path()); }
                }
            }
            let target = mods_dir.join("festvisuals.jar");
            let source_meta = std::fs::metadata(source).ok();
            let target_meta = std::fs::metadata(&target).ok();
            let replace = match (source_meta, target_meta) {
                (Some(source), Some(target)) => source.len() != target.len() || source.modified().ok() > target.modified().ok(),
                (Some(_), None) => true,
                _ => false,
            };
            if replace { let _ = std::fs::copy(source, &target); }
        }
    }
    let resolved_json = resolved.iter().map(|(slug, file)| format!(
        "    {}: {{ \"version\": \"\", \"filename\": {}, \"url\": {}, \"sha1\": {}, \"size\": {}, \"id\": {} }}",
        netutil::json::escape(slug), netutil::json::escape(&file.filename), netutil::json::escape(&file.url),
        netutil::json::escape(&file.sha1), file.size, netutil::json::escape(&file.id)
    )).collect::<Vec<_>>().join(",\n");
    let managed_json = keep.iter().map(|name| format!("    {}", netutil::json::escape(name))).collect::<Vec<_>>().join(",\n");
    let body = format!("{{\n  \"resolved\": {{\n{resolved_json}\n  }},\n  \"managed\": [\n{managed_json}\n  ],\n  \"resolvedAt\": {}\n}}\n", now_ms());
    std::fs::write(state_path, body).ok();
    let _ = state;
    Ok(())
}

fn read_resolved(value: &Value) -> Option<ResolvedMod> {
    Some(ResolvedMod {
        filename: value.get("filename")?.as_str()?.to_string(),
        url: value.get("url")?.as_str()?.to_string(),
        sha1: value.get("sha1").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        size: value.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
        id: value.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
    })
}

fn resolve_latest(slug: &str) -> Result<Option<ResolvedMod>, String> {
    let url = format!("https://api.modrinth.com/v2/project/{slug}/version?loaders=[\"fabric\"]&game_versions=[\"{VERSION}\"]");
    let value = get_json(&url)?;
    let latest = value.as_array().and_then(|list| list.first());
    let Some(latest) = latest else { return Ok(None) };
    let files = latest.get("files").and_then(|v| v.as_array()).ok_or("нет файла мода")?;
    let file = files.iter().find(|f| f.get("primary").and_then(|v| v.as_bool()) == Some(true)).or_else(|| files.first()).ok_or("нет файла мода")?;
    Ok(Some(ResolvedMod {
        filename: file.get("filename").and_then(|v| v.as_str()).unwrap_or("mod.jar").to_string(),
        url: file.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        sha1: file.get("hashes").and_then(|v| v.get("sha1")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
        size: file.get("size").and_then(|v| v.as_u64()).unwrap_or(0),
        id: String::new(),
    }))
}

fn latest_festvisuals(root: &Path, progress: &dyn Fn(u8)) -> Result<Option<PathBuf>, String> {
    let dir = root.join("cache").join("festvisuals");
    let jar = dir.join("festvisuals.jar");
    let meta_file = dir.join("mod.json");
    let response = match netutil::get(&format!("{FEED}mod.json")) { Ok(response) if response.ok() => response, _ => return Ok(jar.exists().then_some(jar)) };
    let meta = netutil::json::parse(&response.text()).ok_or("битый mod.json")?;
    let sha1 = meta.get("sha1").and_then(|v| v.as_str()).unwrap_or("");
    let file = meta.get("file").and_then(|v| v.as_str()).unwrap_or("festvisuals.jar");
    let local = std::fs::read_to_string(&meta_file).ok().and_then(|text| netutil::json::parse(&text));
    let same = local.as_ref().and_then(|v| v.get("sha1")).and_then(|v| v.as_str()) == Some(sha1);
    if !jar.exists() || !same {
        progress(0);
        download_checked(&format!("{FEED}{file}"), &jar, Some(sha1))?;
        std::fs::write(&meta_file, response.body).ok();
        progress(100);
    }
    Ok(Some(jar))
}

struct VanillaPlan {
    classpath: String,
    natives: PathBuf,
    main_class: String,
    asset_index: String,
    arguments: Vec<String>,
}

impl VanillaPlan {
    fn game_args(&self, game_dir: &Path, root: &Path, request: &LaunchRequest) -> Vec<String> {
        let mut fields = std::collections::HashMap::new();
        fields.insert("${auth_player_name}", request.username.clone());
        fields.insert("${auth_session}", "0".into());
        fields.insert("${auth_access_token}", "0".into());
        fields.insert("${auth_uuid}", "00000000-0000-0000-0000-000000000000".into());
        fields.insert("${auth_xuid}", "0".into());
        fields.insert("${user_properties}", "{}".into());
        fields.insert("${user_type}", "offline".into());
        fields.insert("${version_name}", VERSION.into());
        fields.insert("${version_type}", "release".into());
        fields.insert("${assets_index_name}", self.asset_index.clone());
        fields.insert("${assets_root}", root.join("assets").display().to_string());
        fields.insert("${game_assets}", root.join("assets").display().to_string());
        fields.insert("${game_directory}", game_dir.display().to_string());
        fields.insert("${clientid}", "0".into());
        fields.insert("${resolution_width}", request.width.to_string());
        fields.insert("${resolution_height}", request.height.to_string());
        let mut args: Vec<String> = self.arguments.iter().map(|arg| fields.get(arg.as_str()).cloned().unwrap_or_else(|| arg.clone())).collect();
        if request.fullscreen { args.push("--fullscreen".into()); }
        else { args.extend(["--width".into(), request.width.to_string(), "--height".into(), request.height.to_string()]); }
        args
    }
}

fn prepare_vanilla(root: &Path, version: &Value, fast: bool, assets_progress: &dyn Fn(u8), libs_progress: &dyn Fn(u8)) -> Result<VanillaPlan, String> {
    let id = profile_id();
    let profile: Value = std::fs::read_to_string(root.join("versions").join(&id).join(format!("{id}.json"))).ok()
        .and_then(|text| netutil::json::parse(&text)).ok_or("нет профиля Fabric")?;
    let client = version.get("downloads").and_then(|v| v.get("client")).ok_or("нет ссылки на клиент")?;
    let client_path = root.join("versions").join(&id).join(format!("{VERSION}.jar"));
    if !client_path.exists() {
        download_checked(client.get("url").and_then(|v| v.as_str()).unwrap_or(""), &client_path, client.get("sha1").and_then(|v| v.as_str()))?;
    }
    let mut classpath = Vec::new();
    classpath.push(client_path.display().to_string());
    let libraries = merged_libraries(version, &profile);
    let total = libraries.len().max(1);
    for (index, library) in libraries.iter().enumerate() {
        if !library.allowed() { continue; }
        let dest = root.join("libraries").join(&library.path);
        if !acceptable(&dest, library.sha1.as_deref(), library.size, fast) {
            download_checked(&library.url, &dest, library.sha1.as_deref())?;
        }
        classpath.push(dest.display().to_string());
        if index % 8 == 0 { libs_progress(((index * 100) / total) as u8); }
    }
    libs_progress(100);

    let natives = extract_natives(root, version)?;
    let asset_index = version.get("assetIndex").and_then(|v| v.get("id")).and_then(|v| v.as_str()).unwrap_or(VERSION);
    download_assets(root, version, fast, assets_progress)?;
    let arguments = game_arguments(&profile).or_else(|| game_arguments(version)).unwrap_or_default();
    let main_class = profile.get("mainClass").and_then(|v| v.as_str())
        .or_else(|| version.get("mainClass").and_then(|v| v.as_str())).unwrap_or("net.minecraft.client.main.Main").to_string();
    Ok(VanillaPlan {
        classpath: classpath.join(if cfg!(target_os = "windows") { ";" } else { ":" }),
        natives,
        main_class,
        asset_index: asset_index.to_string(),
        arguments,
    })
}

struct Library {
    path: String,
    url: String,
    sha1: Option<String>,
    size: u64,
    rules: Vec<Rule>,
}

impl Library {
    fn allowed(&self) -> bool {
        if self.rules.is_empty() { return true; }
        let mut allow = false;
        for rule in &self.rules {
            if rule.matches() { allow = rule.allow; }
        }
        allow
    }
}

struct Rule {
    allow: bool,
    os: Option<String>,
    arch: Option<String>,
}

impl Rule {
    fn matches(&self) -> bool {
        let os_ok = self.os.as_deref().map(|name| name == current_os()).unwrap_or(true);
        let arch_ok = self.arch.as_deref().map(|arch| arch == current_arch()).unwrap_or(true);
        os_ok && arch_ok
    }
}

fn current_os() -> &'static str {
    if cfg!(target_os = "windows") { "windows" } else if cfg!(target_os = "macos") { "osx" } else { "linux" }
}

fn current_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" }
}

fn merged_libraries(version: &Value, profile: &Value) -> Vec<Library> {
    let mut out = Vec::new();
    for source in [version, profile] {
        let Some(list) = source.get("libraries").and_then(|v| v.as_array()) else { continue };
        for library in list {
            if let Some(parsed) = parse_library(library) {
                if let Some(slot) = out.iter_mut().find(|existing: &&mut Library| existing.path == parsed.path) { *slot = parsed; }
                else { out.push(parsed); }
            }
        }
    }
    out
}

fn parse_library(value: &Value) -> Option<Library> {
    let artifact = value.get("downloads").and_then(|v| v.get("artifact"));
    let path = artifact.and_then(|v| v.get("path")).and_then(|v| v.as_str())
        .map(|s| s.to_string()).or_else(|| maven_path(value.get("name")?.as_str()?))?;
    let url = artifact.and_then(|v| v.get("url")).and_then(|v| v.as_str()).map(|s| s.to_string()).or_else(|| {
        let base = value.get("url").and_then(|v| v.as_str()).unwrap_or(LIBRARIES);
        Some(format!("{}{}", base.trim_end_matches('/'), format!("/{path}")))
    })?;
    let rules = value.get("rules").and_then(|v| v.as_array()).map(|rules| {
        rules.iter().filter_map(|rule| Some(Rule {
            allow: rule.get("action")?.as_str() == Some("allow"),
            os: rule.get("os").and_then(|v| v.get("name")).and_then(|v| v.as_str()).map(|s| s.to_string()),
            arch: rule.get("os").and_then(|v| v.get("arch")).and_then(|v| v.as_str()).map(|s| s.to_string()),
        })).collect()
    }).unwrap_or_default();
    Some(Library {
        path,
        url,
        sha1: artifact.and_then(|v| v.get("sha1")).and_then(|v| v.as_str()).map(|s| s.to_string()),
        size: artifact.and_then(|v| v.get("size")).and_then(|v| v.as_u64()).unwrap_or(0),
        rules,
    })
}

fn maven_path(name: &str) -> Option<String> {
    let mut parts = name.split(':');
    let group = parts.next()?.replace('.', "/");
    let artifact = parts.next()?;
    let version = parts.next()?;
    let classifier = parts.next();
    let file = match classifier {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.jar"),
        None => format!("{artifact}-{version}.jar"),
    };
    Some(format!("{group}/{artifact}/{version}/{file}"))
}

fn game_arguments(value: &Value) -> Option<Vec<String>> {
    if let Some(text) = value.get("minecraftArguments").and_then(|v| v.as_str()) {
        return Some(text.split_whitespace().map(|s| s.to_string()).collect());
    }
    let args = value.get("arguments").and_then(|v| v.get("game")).and_then(|v| v.as_array())?;
    let mut out = Vec::new();
    for arg in args {
        match arg {
            Value::String(text) => out.push(text.clone()),
            Value::Object(_) => {
                let rules = arg.get("rules").and_then(|v| v.as_array()).unwrap_or(&[]);
                let features = rules.iter().any(|rule| rule.get("features").is_some());
                if features { continue; }
                let allowed = rules.is_empty() || rules.iter().any(|rule| {
                    rule.get("action").and_then(|v| v.as_str()) == Some("allow") && rule.get("os").map(|os| {
                        os.get("name").and_then(|v| v.as_str()).map(|name| name == current_os()).unwrap_or(true)
                    }).unwrap_or(true)
                });
                if allowed {
                    match arg.get("value") {
                        Some(Value::String(text)) => out.push(text.clone()),
                        Some(Value::Array(list)) => out.extend(list.iter().filter_map(|v| v.as_str().map(|s| s.to_string()))),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Some(out)
}

fn extract_natives(root: &Path, version: &Value) -> Result<PathBuf, String> {
    let dir = root.join("natives").join(VERSION);
    std::fs::create_dir_all(&dir).ok();
    let Some(libraries) = version.get("libraries").and_then(|v| v.as_array()) else { return Ok(dir) };
    for library in libraries {
        let classifiers = library.get("downloads").and_then(|v| v.get("classifiers"));
        let Some(classifiers) = classifiers else { continue };
        let key = format!("natives-{}", current_os());
        let native = classifiers.get(&key).or_else(|| if current_os() == "osx" { classifiers.get("natives-macos") } else { None });
        let Some(native) = native else { continue };
        if !rules_allow(library) { continue; }
        let url = native.get("url").and_then(|v| v.as_str()).unwrap_or("");
        let name = url.rsplit('/').next().unwrap_or("native.jar");
        let jar = dir.join(name);
        if !jar.exists() { download_checked(url, &jar, native.get("sha1").and_then(|v| v.as_str()))?; }
        let _ = crate::zip::extract_all(&jar, &dir);
        let _ = std::fs::remove_file(&jar);
    }
    Ok(dir)
}

fn rules_allow(library: &Value) -> bool {
    parse_library(library).map(|l| l.allowed()).unwrap_or(true)
}

fn download_assets(root: &Path, version: &Value, fast: bool, progress: &dyn Fn(u8)) -> Result<(), String> {
    let index = version.get("assetIndex").ok_or("нет индекса ресурсов")?;
    let id = index.get("id").and_then(|v| v.as_str()).unwrap_or(VERSION);
    let index_path = root.join("assets").join("indexes").join(format!("{id}.json"));
    if !index_path.exists() {
        download_checked(index.get("url").and_then(|v| v.as_str()).unwrap_or(""), &index_path, index.get("sha1").and_then(|v| v.as_str()))?;
    }
    let parsed = std::fs::read_to_string(&index_path).ok().and_then(|text| netutil::json::parse(&text)).ok_or("битый индекс ресурсов")?;
    let objects = parsed.get("objects").and_then(|v| v.as_object()).map(|items| items.to_vec()).unwrap_or_default();
    let total = objects.len().max(1);
    progress(0);
    for (index, (_, object)) in objects.iter().enumerate() {
        let hash = object.get("hash").and_then(|v| v.as_str()).unwrap_or("");
        if hash.len() < 2 { continue; }
        let dest = root.join("assets").join("objects").join(&hash[..2]).join(hash);
        if !acceptable(&dest, Some(hash), object.get("size").and_then(|v| v.as_u64()).unwrap_or(0), fast) {
            download_checked(&format!("{RESOURCES}/{}/{hash}", &hash[..2]), &dest, Some(hash))?;
        }
        if index % 32 == 0 { progress(((index * 100) / total) as u8); }
    }
    progress(100);
    Ok(())
}

fn acceptable(path: &Path, sha1: Option<&str>, size: u64, fast: bool) -> bool {
    let Ok(meta) = std::fs::metadata(path) else { return false };
    if meta.len() == 0 || (size > 0 && meta.len() != size) { return false; }
    if fast || sha1.is_none() { return true; }
    netutil::sha1_file(path).ok().as_deref() == sha1
}

fn download_checked(url: &str, dest: &Path, sha1: Option<&str>) -> Result<(), String> {
    let mut last = String::from("не скачалось");
    for attempt in 0..4 {
        match netutil::get(url) {
            Ok(response) if response.ok() => {
                if let Some(expected) = sha1 {
                    if !expected.is_empty() && netutil::sha1(&response.body) != expected {
                        last = format!("повреждённый файл {}", dest.file_name().and_then(|s| s.to_str()).unwrap_or(""));
                        std::thread::sleep(Duration::from_millis(300 * (attempt + 1)));
                        continue;
                    }
                }
                if let Some(parent) = dest.parent() { std::fs::create_dir_all(parent).ok(); }
                let tmp = dest.with_extension("part");
                std::fs::write(&tmp, &response.body).map_err(|e| e.to_string())?;
                std::fs::rename(&tmp, dest).map_err(|e| e.to_string())?;
                return Ok(());
            }
            Ok(response) => last = format!("HTTP {} для {url}", response.status),
            Err(error) => last = error.to_string(),
        }
        std::thread::sleep(Duration::from_millis(300 * (attempt + 1)));
    }
    Err(last)
}

fn get_json(url: &str) -> Result<Value, String> {
    let response = netutil::get(url).map_err(|e| e.to_string())?;
    if !response.ok() { return Err(format!("HTTP {} для {url}", response.status)); }
    netutil::json::parse(&response.text()).ok_or_else(|| format!("не JSON: {url}"))
}

struct InstantPlan {
    java: PathBuf,
    classpath: String,
    natives: PathBuf,
    main_class: String,
    asset_index: String,
    arguments: Vec<String>,
    stamp: u128,
}

fn plan_from_instant(hit: InstantPlan) -> (PathBuf, VanillaPlan) {
    let java = hit.java;
    let plan = VanillaPlan {
        classpath: hit.classpath,
        natives: hit.natives,
        main_class: hit.main_class,
        asset_index: hit.asset_index,
        arguments: hit.arguments,
    };
    (java, plan)
}

fn instant_path(root: &Path) -> PathBuf {
    root.join("cache").join("instant-26.2.plan")
}

fn save_instant(root: &Path, java: &Path, plan: &VanillaPlan) {
    if plan.classpath.contains('\n') || plan.arguments.iter().any(|arg| arg.contains('\n')) {
        return;
    }
    let path = instant_path(root);
    let _ = std::fs::create_dir_all(path.parent().unwrap_or(root));
    let mut body = String::new();
    body.push_str("INSTANT1\n");
    body.push_str(&now_ms().to_string());
    body.push('\n');
    body.push_str(&java.display().to_string());
    body.push('\n');
    body.push_str(&plan.natives.display().to_string());
    body.push('\n');
    body.push_str(&plan.main_class);
    body.push('\n');
    body.push_str(&plan.asset_index);
    body.push('\n');
    body.push_str(&plan.arguments.len().to_string());
    body.push('\n');
    for arg in &plan.arguments {
        body.push_str(arg);
        body.push('\n');
    }
    body.push_str(&plan.classpath);
    body.push('\n');
    let _ = std::fs::write(path, body);
}

fn load_instant(root: &Path) -> Option<InstantPlan> {
    let text = std::fs::read_to_string(instant_path(root)).ok()?;
    let mut lines = text.lines();
    if lines.next()? != "INSTANT1" { return None; }
    let stamp = lines.next()?.parse().ok()?;
    let java = PathBuf::from(lines.next()?);
    let natives = PathBuf::from(lines.next()?);
    let main_class = lines.next()?.to_string();
    let asset_index = lines.next()?.to_string();
    let count: usize = lines.next()?.parse().ok()?;
    let mut arguments = Vec::with_capacity(count);
    for _ in 0..count {
        arguments.push(lines.next()?.to_string());
    }
    let classpath = lines.next()?.to_string();
    if classpath.is_empty() || main_class.is_empty() { return None; }
    Some(InstantPlan { java, classpath, natives, main_class, asset_index, arguments, stamp })
}

fn instant_usable(root: &Path, plan: &InstantPlan) -> bool {
    if !plan.java.is_file() || !plan.natives.is_dir() { return false; }
    let sep = if cfg!(target_os = "windows") { ';' } else { ':' };
    let client = plan.classpath.split(sep).next().unwrap_or("");
    if client.is_empty() || !Path::new(client).is_file() { return false; }
    let age = now_ms().saturating_sub(plan.stamp);
    if age > FULL_CHECK_EVERY_MS { return false; }
    let mods = root.join("instances").join(VERSION).join("mods");
    if let Ok(meta) = std::fs::metadata(mods) {
        if let Some(modified) = meta.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()) {
            if modified.as_millis() > plan.stamp.saturating_add(2_000) { return false; }
        }
    }
    true
}

fn verified_recently(root: &Path) -> bool {
    std::fs::metadata(root.join(".festvisuals-verified")).ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.elapsed().ok())
        .map(|age| age.as_millis() < FULL_CHECK_EVERY_MS)
        .unwrap_or(false)
}

fn is_whole_zip(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let Ok(meta) = file.metadata() else { return false };
    if meta.len() < 22 { return false; }
    let span = meta.len().min(65557) as usize;
    let start = meta.len() - span as u64;
    use std::io::{Seek, SeekFrom};
    if file.seek(SeekFrom::Start(start)).is_err() { return false; }
    let mut buf = vec![0u8; span];
    if file.read_exact(&mut buf).is_err() { return false; }
    buf.windows(4).any(|w| w == [0x50, 0x4b, 0x05, 0x06])
}

fn remove_corrupt_jars(root: &Path) -> usize {
    let mut removed = 0;
    for base in [root.join("libraries"), root.join("versions")] {
        removed += walk_jars(&base);
    }
    removed
}

fn walk_jars(dir: &Path) -> usize {
    let mut removed = 0;
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() { removed += walk_jars(&path); }
        else if path.extension().and_then(|s| s.to_str()) == Some("jar") && !is_whole_zip(&path) {
            if std::fs::remove_file(&path).is_ok() { removed += 1; }
        }
    }
    removed
}

fn migrate_legacy_layout(root: &Path) -> Result<(), String> {
    let target = root.join("instances").join(VERSION);
    if target.exists() { return Ok(()); }
    let mods = root.join("mods");
    let ours = root.join("FestVisuals").exists() || std::fs::read_dir(&mods).ok().into_iter().flatten().flatten()
        .any(|entry| entry.file_name().to_string_lossy().to_ascii_lowercase().starts_with("festvisuals"));
    if !ours { return Ok(()); }
    std::fs::create_dir_all(&target).ok();
    let shared = ["assets", "libraries", "versions", "runtime", "natives", "cache", "instances", ".festvisuals-verified", "launcher_profiles.json"];
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if shared.contains(&name.as_str()) { continue; }
            let _ = std::fs::rename(entry.path(), target.join(&name));
        }
    }
    Ok(())
}

fn socket_tmp_dir() -> Option<PathBuf> {
    if !cfg!(target_os = "windows") { return None; }
    let dir = PathBuf::from(std::env::var("ProgramData").unwrap_or_else(|_| "C:\\ProgramData".into())).join("FestVisuals").join("tmp");
    if !dir.display().to_string().is_ascii() { return None; }
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn mod_id(jar: &Path) -> Option<String> {
    let bytes = crate::zip::read_entry(jar, "fabric.mod.json")?;
    let text = String::from_utf8(bytes).ok()?;
    netutil::json::parse(&text)?.get("id")?.as_str().map(|s| s.to_string())
}

fn jarvis_dir() -> Option<PathBuf> {
    let candidates = [
        std::env::var_os("FESTVISUALS_JARVIS_DIR").map(PathBuf::from),
        std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join("jarvis"))),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("jarvis-server")),
    ];
    candidates.into_iter().flatten().find(|dir| dir.join("server.py").exists())
}

#[allow(dead_code)]
const _PARALLEL: usize = PARALLEL;
