//! The 26.2 cockpit. One client, liquid glass, the iridescent rim in motion.

use crate::engine::{EngineEvent, LaunchRequest, Settings};
use crate::font::Font;
use crate::paint::{wrap, Canvas, Image};

const PAPER: [u8; 4] = [246, 241, 234, 255];
const PAPER_DIM: [u8; 4] = [246, 241, 234, 150];
const EMBER: [u8; 3] = [255, 122, 24];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page { Home, Profile, Settings }

#[derive(Clone, Copy, PartialEq)]
enum Id {
    Drag,
    Close,
    Min,
    Nav(u8),
    Play,
    Mods,
    Banner(u8),
    Hello,
    Ram,
    Fullscreen,
    CloseOnLaunch,
    Folder,
    Root,
    Pick,
    Nick,
    Avatar,
    Res(u8),
    Dismiss,
}

#[derive(Clone, Copy)]
struct Rect { x: f32, y: f32, w: f32, h: f32 }

impl Rect {
    fn contains(self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.1 >= self.y && p.0 < self.x + self.w && p.1 < self.y + self.h
    }
}

#[derive(Clone, Debug)]
pub enum Phase {
    Idle,
    Busy { label: String, percent: u8 },
    Starting,
    Running,
}

#[derive(Clone, Debug)]
pub enum UiAction {
    None,
    Launch(LaunchRequest),
    Stop,
    OpenFolder,
    OpenRoot,
    PickDirectory,
    PickAvatar,
    SaveSettings(Settings),
    Quit,
    Minimize,
}

pub enum Key { Text(String), Backspace, Enter, Escape, Left, Right }

pub struct Ui {
    time: f32,
    page: Page,
    page_at: f32,
    mouse: (f32, f32),
    hover: Option<Id>,
    pressed: Option<Id>,
    settings: Settings,
    phase: Phase,
    error: Option<String>,
    note: Option<String>,
    focus: Option<Id>,
    caret: usize,
    scroll: f32,
    backdrop: Image,
    icon: Image,
    banners: [Image; 3],
    font: Font,
    instant: bool,
    hits: Vec<(Rect, Id)>,
    sliding: bool,
    indicator: f32,
    max_ram: u32,
    concealed: bool,
}

impl Ui {
    pub fn new(font: Font, mut backdrop: Image, icon: Image) -> Self {
        backdrop.ink();
        Self {
            time: 0.0,
            page: Page::Home,
            page_at: 0.0,
            mouse: (640.0, 400.0),
            hover: None,
            pressed: None,
            settings: Settings::default(),
            phase: Phase::Idle,
            error: None,
            note: None,
            focus: None,
            caret: 0,
            scroll: 0.0,
            backdrop,
            icon,
            banners: [
                banner(include_bytes!("../assets/plate-sakura.rgba"), 960, 540),
                banner(include_bytes!("../assets/plate-nether.rgba"), 640, 360),
                banner(include_bytes!("../assets/plate-end.rgba"), 640, 360),
            ],
            font,
            instant: false,
            hits: Vec::new(),
            sliding: false,
            indicator: 18.0,
            max_ram: mem_total(),
            concealed: false,
        }
    }

    pub fn animating(&self) -> bool { true }

    pub fn set_settings(&mut self, settings: Settings) {
        self.caret = settings.username.chars().count();
        self.settings = settings;
    }

    pub fn set_avatar(&mut self, path: String) {
        self.settings.avatar = path;
    }

    pub fn set_note(&mut self, note: String) { self.note = Some(note); }

    pub fn set_instant(&mut self, on: bool) { self.instant = on; }

    pub fn warm(&mut self) {
        for sample in ["FestVisuals", "26.2", "Запустить", "Пепельная цитадель", "Мгновенный запуск", "Главная", "Профиль", "Настройки", "ViaFabricPlus"] {
            let _ = self.font.measure(sample, 15, 650);
            let _ = self.font.measure(sample, 22, 740);
        }
        let _ = self.font.measure("26.2", 68, 760);
        let _ = self.font.measure("FestVisuals", 34, 760);
    }

    pub fn concealed(&self) -> bool { self.concealed }

    pub fn reveal(&mut self) { self.concealed = false; }

    pub fn apply_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::Ready(settings) => self.set_settings(settings),
            EngineEvent::RootChanged(path) => {
                self.settings.root = path;
                self.note = Some("Папка игры обновлена".into());
            }
            EngineEvent::Progress { label, percent } => self.phase = Phase::Busy { label, percent },
            EngineEvent::Starting => self.phase = Phase::Starting,
            EngineEvent::Running => {
                self.phase = Phase::Running;
                self.settings.last_launch = crate::engine::now_ms().to_string();
                if self.settings.close_on_launch { self.concealed = true; }
            }
            EngineEvent::Closed => {
                self.phase = Phase::Idle;
                self.concealed = false;
            }
            EngineEvent::Error(message) => {
                self.error = Some(message);
                self.phase = Phase::Idle;
                self.concealed = false;
            }
        }
    }

    pub fn pointer(&mut self, x: f32, y: f32) {
        self.mouse = (x, y);
        self.hover = self.hit(x, y);
        if self.sliding {
            self.settings.ram = ram_from_x(self.slider_rect(), x, self.max_ram);
        }
    }

    pub fn pointer_down(&mut self, x: f32, y: f32) -> UiAction {
        let hit = self.hit(x, y);
        self.pressed = hit;
        self.hover = hit;
        match hit {
            Some(Id::Nick) => {
                self.focus = Some(Id::Nick);
                self.caret = self.settings.username.chars().count();
                UiAction::None
            }
            Some(Id::Ram) => {
                self.sliding = true;
                self.settings.ram = ram_from_x(self.slider_rect(), x, self.max_ram);
                UiAction::None
            }
            Some(other) if !matches!(other, Id::Drag) => {
                self.focus = None;
                UiAction::None
            }
            _ => {
                self.focus = None;
                UiAction::None
            }
        }
    }

    pub fn pointer_up(&mut self, x: f32, y: f32) -> UiAction {
        let hit = self.hit(x, y);
        let pressed = self.pressed.take();
        self.sliding = false;
        if pressed != hit { return if matches!(pressed, Some(Id::Ram)) { self.saved() } else { UiAction::None }; }
        match hit {
            Some(Id::Close) | Some(Id::Nav(3)) => UiAction::Quit,
            Some(Id::Min) => UiAction::Minimize,
            Some(Id::Nav(0)) => self.goto(Page::Home),
            Some(Id::Nav(1)) | Some(Id::Hello) => self.goto(Page::Profile),
            Some(Id::Nav(2)) => self.goto(Page::Settings),
            Some(Id::Play) | Some(Id::Banner(0)) | Some(Id::Banner(1)) | Some(Id::Banner(2)) => self.launch_or_stop(),
            Some(Id::Mods) | Some(Id::Folder) => UiAction::OpenFolder,
            Some(Id::Root) => UiAction::OpenRoot,
            Some(Id::Pick) => UiAction::PickDirectory,
            Some(Id::Avatar) => UiAction::PickAvatar,
            Some(Id::Fullscreen) => { self.settings.fullscreen = !self.settings.fullscreen; self.saved() }
            Some(Id::CloseOnLaunch) => { self.settings.close_on_launch = !self.settings.close_on_launch; self.saved() }
            Some(Id::Res(i)) => {
                let (w, h) = PRESETS[i as usize];
                self.settings.width = w;
                self.settings.height = h;
                self.saved()
            }
            Some(Id::Dismiss) => { self.error = None; self.note = None; UiAction::None }
            Some(Id::Ram) => self.saved(),
            _ => UiAction::None,
        }
    }

    pub fn scroll(&mut self, dy: f32) {
        if self.page != Page::Home {
            self.scroll = (self.scroll + dy).clamp(0.0, 220.0);
        }
    }

    pub fn blur(&mut self) { self.pressed = None; self.sliding = false; }

    pub fn key(&mut self, key: Key) -> UiAction {
        if self.focus == Some(Id::Nick) {
            match key {
                Key::Text(text) => {
                    let mut chars: Vec<char> = self.settings.username.chars().collect();
                    let at = self.caret.min(chars.len());
                    for ch in text.chars().filter(|c| !c.is_control()) {
                        chars.insert(at + (self.caret - at), ch);
                        self.caret += 1;
                    }
                    self.settings.username = chars.into_iter().collect();
                    return self.saved();
                }
                Key::Backspace => {
                    let mut chars: Vec<char> = self.settings.username.chars().collect();
                    if self.caret > 0 && self.caret <= chars.len() {
                        chars.remove(self.caret - 1);
                        self.caret -= 1;
                        self.settings.username = chars.into_iter().collect();
                    }
                    return self.saved();
                }
                Key::Left => { self.caret = self.caret.saturating_sub(1); return UiAction::None; }
                Key::Right => { self.caret = (self.caret + 1).min(self.settings.username.chars().count()); return UiAction::None; }
                Key::Escape => { self.focus = None; return UiAction::None; }
                Key::Enter => { self.focus = None; return UiAction::None; }
            }
        }
        if matches!(key, Key::Enter) && self.page == Page::Home && matches!(self.phase, Phase::Idle) {
            return self.launch_or_stop();
        }
        UiAction::None
    }

    pub fn hit_test(&self, p: (f32, f32)) -> bool {
        matches!(self.hits.iter().rev().find(|(rect, _)| rect.contains(p)).map(|(_, id)| *id), Some(Id::Drag))
    }

    pub fn over_control(&self, p: (f32, f32)) -> bool {
        self.hits.iter().rev().any(|(rect, id)| rect.contains(p) && !matches!(id, Id::Drag))
    }

    fn hit(&self, x: f32, y: f32) -> Option<Id> {
        self.hits.iter().rev().find(|(rect, _)| rect.contains((x, y))).map(|(_, id)| *id)
    }

    fn goto(&mut self, page: Page) -> UiAction {
        if self.page != page {
            self.page = page;
            self.page_at = self.time;
            self.scroll = 0.0;
            self.focus = None;
        }
        UiAction::None
    }

    fn saved(&self) -> UiAction { UiAction::SaveSettings(self.settings.clone()) }

    fn launch_or_stop(&mut self) -> UiAction {
        if !matches!(self.phase, Phase::Idle) {
            return UiAction::Stop;
        }
        let name = {
            let trimmed = self.settings.username.trim();
            if trimmed.is_empty() { "FestPlayer".into() } else { trimmed.to_string() }
        };
        UiAction::Launch(LaunchRequest {
            version: "26.2".into(),
            ram: self.settings.ram.min(self.max_ram),
            username: name,
            root: self.settings.root.clone(),
            close_on_launch: self.settings.close_on_launch,
            width: self.settings.width,
            height: self.settings.height,
            fullscreen: self.settings.fullscreen,
        })
    }

    fn slider_rect(&self) -> Rect {
        // Filled during paint; fall back keeps the drag usable for one frame.
        self.hits.iter().find(|(_, id)| *id == Id::Ram).map(|(r, _)| *r).unwrap_or(Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 })
    }

    pub fn frame(&mut self, dt: f32) { self.time += dt; }

    pub fn open_page(&mut self, name: &str) {
        self.page = match name {
            "profile" => Page::Profile,
            "settings" => Page::Settings,
            _ => Page::Home,
        };
        self.page_at = self.time;
    }

    pub fn paint(&mut self, canvas: &mut Canvas) {
        self.hits.clear();
        let w = canvas.w as f32;
        let h = canvas.h as f32;
        self.push(Rect { x: 0.0, y: 0.0, w, h }, Id::Drag);
        self.scene(canvas, w, h);
        let stage = Rect { x: 28.0, y: 78.0, w: (w - 56.0).max(280.0), h: (h - 168.0).max(200.0) };
        let rise = self.rise();
        match self.page {
            Page::Home => self.paint_home(canvas, w, h, rise),
            Page::Profile => self.paint_profile(canvas, stage, rise),
            Page::Settings => self.paint_settings(canvas, stage, rise),
        }
        self.paint_dock(canvas, w, h);
        self.paint_topbar(canvas, w);
        if let Some(error) = self.error.clone() {
            let line = error.lines().next().unwrap_or(&error).to_string();
            self.toast(canvas, w, &line);
        } else if let Some(note) = self.note.clone() {
            self.toast(canvas, w, &note);
        }
    }

    fn rise(&self) -> f32 {
        let t = ((self.time - self.page_at) / 0.48).clamp(0.0, 1.0);
        (1.0 - ease_out(t)) * 18.0
    }

    fn scene(&mut self, canvas: &mut Canvas, w: f32, h: f32) {
        let zoom = 1.045 + (self.time * 0.08).sin() * 0.012;
        let ox = (self.mouse.0 / w - 0.5) * -20.0 + (self.time * 0.1).sin() * 7.0;
        let oy = (self.mouse.1 / h - 0.5) * -10.0;
        canvas.cover(&self.backdrop, zoom, ox, oy);
        canvas.freeze();
    }

    fn paint_topbar(&mut self, canvas: &mut Canvas, w: f32) {
        let label = "FESTVISUALS";
        let tw = self.font.measure(label, 12, 700) as f32;
        let bar = Rect { x: 22.0, y: 16.0, w: tw + 54.0, h: 38.0 };
        canvas.glass(bar.x, bar.y, bar.w, bar.h, 19.0, false, self.iris(), self.spot(), self.glare(bar));
        self.blit_icon(canvas, bar.x + 12.0, bar.y + 10.0, 18.0);
        canvas.text(&mut self.font, label, bar.x + 36.0, bar.y + 11.0, 12, 700, PAPER);
        self.push(bar, Id::Drag);
        self.push(Rect { x: bar.x + bar.w + 8.0, y: 0.0, w: (w - 120.0 - bar.x - bar.w).max(0.0), h: 62.0 }, Id::Drag);
        for (i, id) in [(0u8, Id::Min), (1, Id::Close)] {
            let r = Rect { x: w - 96.0 + i as f32 * 42.0, y: 16.0, w: 36.0, h: 36.0 };
            let hot = self.hover == Some(id);
            if id == Id::Close && hot {
                canvas.circle(r.x + 18.0, r.y + 18.0, 16.0, [255, 69, 58, 230]);
            } else {
                canvas.glass(r.x, r.y, r.w, r.h, 18.0, false, self.iris(), self.spot(), None);
            }
            if id == Id::Min {
                canvas.line(r.x + 12.0, r.y + 18.0, r.x + 24.0, r.y + 18.0, 1.6, PAPER);
            } else {
                canvas.line(r.x + 13.0, r.y + 13.0, r.x + 23.0, r.y + 23.0, 1.5, PAPER);
                canvas.line(r.x + 23.0, r.y + 13.0, r.x + 13.0, r.y + 23.0, 1.5, PAPER);
            }
            self.push(r, id);
        }
    }

    fn paint_dock(&mut self, canvas: &mut Canvas, w: f32, h: f32) {
        let labels = [("home", "Главная", 0u8), ("user", "Профиль", 1), ("gear", "Настройки", 2)];
        let slot = 148.0;
        let dock = Rect { x: (w - slot * 3.0) * 0.5, y: h - 74.0, w: slot * 3.0, h: 54.0 };
        canvas.glass(dock.x, dock.y, dock.w, dock.h, 27.0, false, self.iris(), (0.5, 0.35), None);
        let active = match self.page { Page::Home => 0.0, Page::Profile => 1.0, Page::Settings => 2.0 };
        let target = dock.x + 5.0 + active * slot;
        if (self.indicator - target).abs() > 400.0 { self.indicator = target; }
        self.indicator += (target - self.indicator) * 0.18;
        canvas.round_rect(self.indicator, dock.y + 5.0, slot - 10.0, dock.h - 10.0, 18.0, [255, 168, 72, 46]);
        for (kind, label, id) in labels {
            let r = Rect { x: dock.x + id as f32 * slot, y: dock.y, w: slot, h: dock.h };
            let on = match (id, self.page) {
                (0, Page::Home) | (1, Page::Profile) | (2, Page::Settings) => true,
                _ => false,
            };
            let color = if on { PAPER } else { [246, 241, 234, 150] };
            let tw = self.font.measure(label, 13, 680) as f32;
            let ix = r.x + (r.w - tw - 24.0) * 0.5;
            icon(canvas, kind, ix + 8.0, r.y + r.h * 0.5, color);
            canvas.text(&mut self.font, label, ix + 22.0, r.y + 17.0, 13, 680, color);
            self.push(r, Id::Nav(id));
        }
    }

    fn paint_home(&mut self, canvas: &mut Canvas, w: f32, h: f32, rise: f32) {
        let pad = 24.0;
        let top = 68.0;
        let gap = 12.0;
        let bar_h = 92.0;
        let bar = Rect { x: pad, y: top + rise * 0.15, w: w - pad * 2.0, h: bar_h };
        canvas.glass(bar.x, bar.y, bar.w, bar.h, 30.0, true, self.iris(), (0.18, 0.3), None);
        let x = bar.x + 26.0;
        let mut y = bar.y + 14.0;
        self.kicker(canvas, x, y, 260.0);
        y += 22.0;
        canvas.text_iris(&mut self.font, "26.2", x, y, 34, 760, self.time);
        canvas.text(&mut self.font, "FestVisuals", x + 92.0, y + 8.0, 18, 700, PAPER);
        let play = Rect { x: bar.x + bar.w - 214.0, y: bar.y + 18.0, w: 190.0, h: 56.0 };
        self.play_button(canvas, play);
        let name = {
            let trimmed = self.settings.username.trim();
            if trimmed.is_empty() { "FestPlayer" } else { trimmed }
        };
        let who = format!("игрок  {name}");
        let ww = canvas.text(&mut self.font, &who, x, y + 40.0, 13, 650, [246, 241, 234, 190]);
        self.push(Rect { x, y: y + 36.0, w: ww + 6.0, h: 22.0 }, Id::Hello);
        let mods = "папка модов";
        let mx = play.x - 16.0 - self.font.measure(mods, 13, 650) as f32;
        canvas.text(&mut self.font, mods, mx, y + 40.0, 13, 650, [246, 241, 234, 170]);
        let mw = self.font.measure(mods, 13, 650) as f32;
        self.push(Rect { x: mx, y: y + 36.0, w: mw + 8.0, h: 22.0 }, Id::Mods);

        let gy = bar.y + bar.h + gap;
        let gh = (h - 78.0 - gy).max(220.0);
        let left_w = ((w - pad * 2.0 - gap) * 0.62).max(360.0);
        self.plate(canvas, Rect { x: pad, y: gy, w: left_w, h: gh }, 0, "Сакура", "рассвет");
        let rx = pad + left_w + gap;
        let rw = (w - pad - rx).max(160.0);
        let sh = (gh - gap) * 0.5;
        self.plate(canvas, Rect { x: rx, y: gy, w: rw, h: sh }, 1, "Незер", "");
        self.plate(canvas, Rect { x: rx, y: gy + sh + gap, w: rw, h: gh - sh - gap }, 2, "Энд", "");
    }

    fn plate(&mut self, canvas: &mut Canvas, rect: Rect, image: usize, label: &str, sub: &str) {
        let id = Id::Banner(image as u8);
        let hot = self.hover == Some(id);
        let y = rect.y + if hot { -3.0 } else { 0.0 };
        let drift = (self.time * 0.05 + image as f32).sin() * 4.0;
        canvas.blit_round(&self.banners[image], rect.x, y, rect.w, rect.h, 26.0, drift, drift * 0.2);
        canvas.scrim(rect.x, y, rect.w, rect.h, 26.0);
        canvas.rim(rect.x, y, rect.w, rect.h, 26.0, self.iris() + image as f32 * 70.0, if hot { 1.0 } else { 0.7 });
        let x = rect.x + 16.0;
        canvas.text(&mut self.font, label, x, y + rect.h - 30.0, 16, 740, PAPER);
        if !sub.is_empty() {
            let lw = self.font.measure(label, 16, 740) as f32;
            canvas.text(&mut self.font, sub, x + lw + 10.0, y + rect.h - 28.0, 12, 600, [246, 241, 234, 170]);
        }
        self.push(rect, id);
    }
    fn kicker(&mut self, canvas: &mut Canvas, x: f32, y: f32, width: f32) {
        let (label, color) = match &self.phase {
            Phase::Running => ("В игре".to_string(), [96, 230, 150, 255]),
            Phase::Starting => ("Запускаю клиент".to_string(), [150, 206, 255, 255]),
            Phase::Busy { label, percent } => (format!("{label} · {percent}%"), [150, 206, 255, 255]),
            Phase::Idle if self.instant => ("Мгновенный запуск".to_string(), [255, 186, 96, 255]),
            Phase::Idle => ("Готов к запуску".to_string(), [255, 176, 90, 255]),
        };
        let pulse = 0.45 + (self.time * 2.6).sin().abs() * 0.4;
        canvas.circle(x + 5.0, y + 8.0, 4.0, color);
        canvas.stroke_circle(x + 5.0, y + 8.0, 7.0 + pulse * 3.0, 1.1, [color[0], color[1], color[2], (80.0 * (1.0 - pulse)) as u8]);
        let shown = ellipsize(&mut self.font, &label, 13, 650, (width - 70.0) as i32);
        canvas.text(&mut self.font, &shown, x + 18.0, y, 13, 650, PAPER_DIM);
        canvas.round_rect(x + width - 52.0, y - 2.0, 52.0, 18.0, 9.0, [255, 122, 24, 46]);
        canvas.text(&mut self.font, "FREE", x + width - 42.0, y + 1.0, 10, 800, [255, 206, 150, 255]);
    }

    fn play_button(&mut self, canvas: &mut Canvas, play: Rect) {
        let hot = self.hover == Some(Id::Play);
        let pressed = self.pressed == Some(Id::Play);
        let play = if pressed { Rect { x: play.x + 1.0, y: play.y + 1.0, w: play.w - 2.0, h: play.h - 2.0 } } else { play };
        let (label, fill) = match &self.phase {
            Phase::Running => ("В игре", 1.0),
            Phase::Busy { percent, .. } => ("Загрузка", *percent as f32 / 100.0),
            Phase::Starting => ("Секунду", 0.35),
            Phase::Idle => ("Запустить", 0.0),
        };
        canvas.round_rect(play.x, play.y + 6.0, play.w, play.h, 18.0, [0, 0, 0, 50]);
        canvas.shade_solid_gradient(play.x, play.y, play.w, play.h, 18.0, [255, 112, 28], [255, 196, 102]);
        if fill > 0.0 {
            canvas.round_rect(play.x, play.y, play.w * fill, play.h, 18.0, [255, 255, 255, 36]);
        }
        let sheen = ((self.time * 0.28).fract() * (play.w + 90.0)) - 40.0;
        canvas.shade(play.x, play.y, play.w, play.h, 18.0, |lx, ly, cov| {
            let d = (lx - sheen).abs();
            let top = (1.0 - ly / play.h).clamp(0.0, 1.0);
            let a = ((1.0 - d / 28.0).clamp(0.0, 1.0) * 80.0 + top * 28.0) * cov;
            [255, 255, 255, a as u8]
        });
        canvas.rim(play.x, play.y, play.w, play.h, 18.0, self.iris(), if hot { 0.7 } else { 0.35 });
        let color = [42, 16, 6, 255];
        let tw = self.font.measure(label, 16, 760) as f32;
        canvas.text(&mut self.font, label, play.x + (play.w - tw) * 0.5 + 8.0, play.y + 16.0, 16, 760, color);
        if matches!(self.phase, Phase::Running) {
            icon(canvas, "check", play.x + 26.0, play.y + 26.0, color);
        } else if !matches!(self.phase, Phase::Idle) {
            icon(canvas, "spin", play.x + 26.0, play.y + 26.0, color);
        } else {
            icon(canvas, "play", play.x + 26.0, play.y + 26.0, color);
            if play.w > 180.0 {
                let kw = self.font.measure("Enter", 11, 650) as f32;
                canvas.round_rect(play.x + play.w - kw - 22.0, play.y + 16.0, kw + 12.0, 20.0, 7.0, [40, 16, 6, 36]);
                canvas.text(&mut self.font, "Enter", play.x + play.w - kw - 16.0, play.y + 18.0, 11, 650, [42, 16, 6, 170]);
            }
        }
        self.push(play, Id::Play);
    }

    fn ghost_icon(&mut self, canvas: &mut Canvas, r: Rect, label: &str, id: Id) {
        let hot = self.hover == Some(id);
        canvas.round_rect(r.x, r.y, r.w, r.h, 18.0, [255, 255, 255, if hot { 28 } else { 14 }]);
        canvas.rim(r.x, r.y, r.w, r.h, 18.0, self.iris(), 0.28);
        icon(canvas, "folder", r.x + 22.0, r.y + r.h * 0.5, PAPER);
        canvas.text(&mut self.font, label, r.x + 36.0, r.y + 16.0, 14, 680, PAPER);
        self.push(r, id);
    }

    fn paint_profile(&mut self, canvas: &mut Canvas, stage: Rect, rise: f32) {
        self.sheet_head(canvas, stage, rise, "Аккаунт", "Профиль", "Ник уходит в игру как имя игрока. Аватар виден только в лаунчере.");
        let y = stage.y + 118.0 + rise - self.scroll;
        let left = Rect { x: stage.x, y, w: (stage.w * 0.42).clamp(280.0, 420.0), h: 280.0 };
        canvas.glass(left.x, left.y, left.w, left.h, 28.0, false, self.iris(), self.spot(), self.glare(left));
        let cx = left.x + left.w * 0.5;
        canvas.stroke_circle(cx, left.y + 92.0, 54.0, 2.0, [255, 186, 120, 180]);
        if self.avatar(canvas, cx - 46.0, left.y + 46.0, 92.0).is_none() {
            icon(canvas, "user", cx, left.y + 92.0, [246, 241, 234, 200]);
        }
        self.push(Rect { x: cx - 46.0, y: left.y + 46.0, w: 92.0, h: 92.0 }, Id::Avatar);
        let name = if self.settings.username.trim().is_empty() { "FestPlayer" } else { self.settings.username.trim() };
        let nw = self.font.measure(name, 22, 760) as f32;
        canvas.text(&mut self.font, name, cx - nw * 0.5, left.y + 168.0, 22, 760, PAPER);
        let role = "Пользователь · навсегда";
        let rw = self.font.measure(role, 13, 500) as f32;
        canvas.text(&mut self.font, role, cx - rw * 0.5, left.y + 202.0, 13, 500, PAPER_DIM);
        let right = Rect { x: left.x + left.w + 16.0, y, w: stage.w - left.w - 16.0, h: 280.0 };
        canvas.glass(right.x, right.y, right.w, right.h, 28.0, true, self.iris(), self.spot(), self.glare(right));
        canvas.text(&mut self.font, "Данные", right.x + 24.0, right.y + 22.0, 18, 720, PAPER);
        canvas.text(&mut self.font, "Ник", right.x + 24.0, right.y + 78.0, 14, 600, PAPER_DIM);
        let field = Rect { x: right.x + 24.0, y: right.y + 104.0, w: right.w - 48.0, h: 44.0 };
        let focused = self.focus == Some(Id::Nick);
        canvas.round_rect(field.x, field.y, field.w, field.h, 12.0, [255, 255, 255, if focused { 28 } else { 16 }]);
        let shown = if self.settings.username.is_empty() { "FestPlayer" } else { &self.settings.username };
        canvas.text(&mut self.font, shown, field.x + 14.0, field.y + 12.0, 16, 650, PAPER);
        if focused && (self.time * 2.0).fract() < 0.5 {
            let before: String = self.settings.username.chars().take(self.caret).collect();
            let cx = field.x + 14.0 + self.font.measure(&before, 16, 650) as f32;
            canvas.line(cx, field.y + 10.0, cx, field.y + 32.0, 1.4, EMBER_A);
        }
        self.push(field, Id::Nick);
        canvas.text(&mut self.font, "Роль", right.x + 24.0, right.y + 168.0, 14, 600, PAPER_DIM);
        canvas.text(&mut self.font, "Пользователь", right.x + 160.0, right.y + 166.0, 15, 700, [255, 177, 90, 255]);
        canvas.text(&mut self.font, "Подписка", right.x + 24.0, right.y + 200.0, 14, 600, PAPER_DIM);
        canvas.text(&mut self.font, "Навсегда", right.x + 160.0, right.y + 198.0, 15, 700, PAPER);
        canvas.text(&mut self.font, "Этот ник лаунчер передаёт в игру при запуске.", right.x + 24.0, right.y + 236.0, 13, 500, PAPER_DIM);
    }

    fn paint_settings(&mut self, canvas: &mut Canvas, stage: Rect, rise: f32) {
        self.sheet_head(canvas, stage, rise, "Клиент 26.2", "Настройки", "Память, окно и папка игры. Моды, миры и конфиги лежат в instances/26.2.");
        let y0 = stage.y + 118.0 + rise - self.scroll;
        let col_w = (stage.w - 16.0) * 0.5;
        let left = Rect { x: stage.x, y: y0, w: col_w, h: 196.0 };
        canvas.glass(left.x, left.y, left.w, left.h, 26.0, false, self.iris(), self.spot(), self.glare(left));
        canvas.text(&mut self.font, "Память", left.x + 22.0, left.y + 18.0, 18, 720, PAPER);
        let ram = self.settings.ram.min(self.max_ram);
        let label = ram_label(ram);
        let lw = self.font.measure(&label, 16, 750) as f32;
        canvas.text(&mut self.font, &label, left.x + left.w - lw - 22.0, left.y + 20.0, 16, 750, [255, 177, 90, 255]);
        canvas.text(&mut self.font, "Сколько оперативной памяти отдать игре.", left.x + 22.0, left.y + 52.0, 13, 500, PAPER_DIM);
        let track = Rect { x: left.x + 22.0, y: left.y + 118.0, w: left.w - 44.0, h: 28.0 };
        canvas.round_rect(track.x, track.y + 10.0, track.w, 6.0, 3.0, [255, 255, 255, 28]);
        let t = ram_t(ram, self.max_ram);
        canvas.round_rect(track.x, track.y + 10.0, track.w * t, 6.0, 3.0, [255, 138, 48, 255]);
        canvas.circle(track.x + track.w * t, track.y + 13.0, 9.0, [255, 236, 214, 255]);
        self.push(track, Id::Ram);
        canvas.text(&mut self.font, "1 ГБ", track.x, track.y + 28.0, 11, 600, PAPER_DIM);
        let max = ram_label(self.max_ram);
        let mw = self.font.measure(&max, 11, 600) as f32;
        canvas.text(&mut self.font, &max, track.x + track.w - mw, track.y + 28.0, 11, 600, PAPER_DIM);

        let right = Rect { x: left.x + left.w + 16.0, y: y0, w: stage.w - left.w - 16.0, h: 196.0 };
        canvas.glass(right.x, right.y, right.w, right.h, 26.0, false, self.iris(), self.spot(), self.glare(right));
        canvas.text(&mut self.font, "Окно", right.x + 22.0, right.y + 18.0, 18, 720, PAPER);
        let mut px = right.x + 22.0;
        let py = right.y + 64.0;
        for (i, (w, h)) in PRESETS.iter().enumerate() {
            let label = format!("{w}×{h}");
            let tw = self.font.measure(&label, 13, 650) as f32 + 20.0;
            let on = self.settings.width == *w && self.settings.height == *h;
            canvas.round_rect(px, py, tw, 32.0, 12.0, if on { [255, 122, 24, 70] } else { [255, 255, 255, 16] });
            canvas.text(&mut self.font, &label, px + 10.0, py + 8.0, 13, 650, if on { [255, 228, 200, 255] } else { PAPER });
            self.push(Rect { x: px, y: py, w: tw, h: 32.0 }, Id::Res(i as u8));
            px += tw + 8.0;
            if px > right.x + right.w - 120.0 && i == 1 { px = right.x + 22.0; }
        }
        self.toggle(canvas, Rect { x: right.x + 22.0, y: right.y + 116.0, w: right.w - 44.0, h: 28.0 }, "На весь экран", self.settings.fullscreen, Id::Fullscreen);
        self.toggle(canvas, Rect { x: right.x + 22.0, y: right.y + 150.0, w: right.w - 44.0, h: 28.0 }, "Скрывать лаунчер", self.settings.close_on_launch, Id::CloseOnLaunch);

        let folder = Rect { x: stage.x, y: y0 + 212.0, w: stage.w, h: 132.0 };
        canvas.glass(folder.x, folder.y, folder.w, folder.h, 26.0, true, self.iris(), self.spot(), self.glare(folder));
        canvas.text(&mut self.font, "Папка 26.2", folder.x + 22.0, folder.y + 18.0, 18, 720, PAPER);
        let path = format!("{}/instances/26.2", self.settings.root);
        let shown = ellipsize(&mut self.font, &path, 14, 500, (folder.w - 48.0) as i32);
        canvas.text(&mut self.font, &shown, folder.x + 22.0, folder.y + 50.0, 14, 500, PAPER_DIM);
        let b1 = Rect { x: folder.x + 22.0, y: folder.y + 82.0, w: 150.0, h: 34.0 };
        let b2 = Rect { x: b1.x + 162.0, y: b1.y, w: 130.0, h: 34.0 };
        let b3 = Rect { x: b2.x + 142.0, y: b1.y, w: 170.0, h: 34.0 };
        self.ghost(canvas, b1, "Открыть", Id::Folder);
        self.ghost(canvas, b2, "Корень", Id::Root);
        self.ghost(canvas, b3, "Выбрать папку", Id::Pick);
    }

    fn sheet_head(&mut self, canvas: &mut Canvas, stage: Rect, rise: f32, eyebrow: &str, title: &str, lede: &str) {
        let x = stage.x;
        let y = stage.y + rise;
        canvas.text(&mut self.font, eyebrow, x, y, 12, 700, [255, 186, 120, 200]);
        canvas.text(&mut self.font, title, x, y + 22.0, 36, 760, PAPER);
        let lines = wrap(&mut self.font, lede, 15, 500, (stage.w.min(680.0)) as i32);
        for (i, line) in lines.into_iter().take(2).enumerate() {
            canvas.text(&mut self.font, &line, x, y + 72.0 + i as f32 * 22.0, 15, 500, PAPER_DIM);
        }
    }

    fn toggle(&mut self, canvas: &mut Canvas, row: Rect, label: &str, on: bool, id: Id) {
        canvas.text(&mut self.font, label, row.x, row.y + 4.0, 14, 600, PAPER);
        let x = row.x + row.w - 46.0;
        let y = row.y + 2.0;
        canvas.round_rect(x, y, 42.0, 24.0, 12.0, if on { [255, 122, 24, 220] } else { [255, 255, 255, 28] });
        let knob = if on { x + 22.0 } else { x + 3.0 };
        canvas.circle(knob + 9.0, y + 12.0, 8.0, [255, 248, 240, 255]);
        self.push(Rect { x, y: y - 4.0, w: 46.0, h: 32.0 }, id);
    }

    fn ghost(&mut self, canvas: &mut Canvas, r: Rect, label: &str, id: Id) {
        let hot = self.hover == Some(id);
        canvas.round_rect(r.x, r.y, r.w, r.h, 12.0, [255, 255, 255, if hot { 32 } else { 16 }]);
        let tw = self.font.measure(label, 13, 680) as f32;
        canvas.text(&mut self.font, label, r.x + (r.w - tw) * 0.5, r.y + 8.0, 13, 680, PAPER);
        self.push(r, id);
    }

    fn toast(&mut self, canvas: &mut Canvas, w: f32, note: &str) {
        let tw = self.font.measure(note, 13, 600) as f32 + 28.0;
        let r = Rect { x: (w - tw) * 0.5, y: 64.0, w: tw, h: 32.0 };
        canvas.round_rect(r.x, r.y, r.w, r.h, 12.0, [20, 12, 10, 210]);
        canvas.text(&mut self.font, note, r.x + 14.0, r.y + 8.0, 13, 600, PAPER);
        self.push(r, Id::Dismiss);
    }

    fn avatar(&mut self, canvas: &mut Canvas, x: f32, y: f32, size: f32) -> Option<()> {
        let bytes = std::fs::read(&self.settings.avatar).ok()?;
        let image = Image::ppm(&bytes).or_else(|| decode_rgba_file(&self.settings.avatar))?;
        // Circle-clipped blit.
        let cx = x + size * 0.5;
        let cy = y + size * 0.5;
        let r = size * 0.5;
        for py in 0..size as i32 {
            for px in 0..size as i32 {
                let dx = px as f32 + 0.5 - r;
                let dy = py as f32 + 0.5 - r;
                if dx * dx + dy * dy > r * r { continue; }
                let u = px as f32 / size;
                let v = py as f32 / size;
                let sx = (u * image.w as f32) as i32;
                let sy = (v * image.h as f32) as i32;
                let s = ((sy * image.w + sx) * 4) as usize;
                if s + 3 < image.rgba.len() {
                    canvas.blend(cx as i32 - r as i32 + px, cy as i32 - r as i32 + py, [
                        image.rgba[s], image.rgba[s + 1], image.rgba[s + 2], 255,
                    ]);
                }
            }
        }
        Some(())
    }

    fn blit_icon(&mut self, canvas: &mut Canvas, x: f32, y: f32, size: f32) {
        let image = &self.icon;
        for py in 0..size as i32 {
            for px in 0..size as i32 {
                let sx = (px as f32 / size * image.w as f32) as i32;
                let sy = (py as f32 / size * image.h as f32) as i32;
                let s = ((sy * image.w + sx) * 4) as usize;
                let a = image.rgba[s + 3];
                if a == 0 { continue; }
                canvas.screen(x as i32 + px, y as i32 + py, [image.rgba[s], image.rgba[s + 1], image.rgba[s + 2], a]);
            }
        }
    }

    fn iris(&self) -> f32 { self.time * 22.5 }
    fn spot(&self) -> (f32, f32) {
        (0.08 + (self.time * 0.52).sin() * 0.16, 0.02 + (self.time * 0.37).cos() * 0.05)
    }
    fn glare(&self, r: Rect) -> Option<(f32, f32)> {
        if self.hover.is_some() && r.contains(self.mouse) {
            Some((self.mouse.0 - r.x, self.mouse.1 - r.y))
        } else {
            None
        }
    }
    fn push(&mut self, r: Rect, id: Id) { self.hits.push((r, id)); }
}

const EMBER_A: [u8; 4] = [255, 177, 90, 255];
const PRESETS: [(u32, u32); 4] = [(1280, 720), (1600, 900), (1920, 1080), (2560, 1440)];

fn ram_label(mb: u32) -> String {
    let gb = mb as f32 / 1024.0;
    if mb >= 1024 && (gb - gb.round()).abs() < 0.12 {
        format!("{} ГБ", gb.round() as u32)
    } else if mb >= 1024 {
        format!("{gb:.1} ГБ")
    } else {
        format!("{mb} МБ")
    }
}

fn last_label(raw: &str) -> String {
    let Ok(ms) = raw.parse::<u128>() else { return "ещё нет".into() };
    if ms == 0 { return "ещё нет".into() }
    let secs = ms / 1000;
    let hour = (secs / 3600) % 24;
    let min = (secs / 60) % 60;
    format!("{:02}:{:02}", hour, min)
}

fn ram_t(ram: u32, max: u32) -> f32 {
    if max <= 1024 { return 1.0; }
    ((ram.clamp(1024, max) - 1024) as f32) / (max - 1024) as f32
}

fn ram_from_x(track: Rect, x: f32, max: u32) -> u32 {
    let t = ((x - track.x) / track.w).clamp(0.0, 1.0);
    let raw = 1024.0 + t * (max.max(1536) - 1024) as f32;
    let stepped = (raw / 512.0).round() * 512.0;
    stepped.clamp(1024.0, max as f32) as u32
}

fn mem_total() -> u32 {
    if let Ok(text) = std::fs::read_to_string("/proc/meminfo") {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kb: u64 = rest.split_whitespace().next().unwrap_or("0").parse().unwrap_or(0);
                return ((kb / 1024) as u32).max(2048);
            }
        }
    }
    8192
}

fn ease_out(t: f32) -> f32 { 1.0 - (1.0 - t).powi(3) }

struct HomeLayout { hero: Rect, featured: Rect, row: [Rect; 3] }

fn home_layout(w: f32, h: f32) -> HomeLayout {
    let pad = 22.0;
    let top = 70.0;
    let bottom = h - 102.0;
    let gap = 14.0;
    let span = (bottom - top).max(320.0);
    let row_h = (span * 0.34).clamp(146.0, 198.0);
    let top_h = (span - gap - row_h).max(220.0);
    let hero_w = (w * 0.34).clamp(328.0, 440.0);
    let hero = Rect { x: pad, y: top, w: hero_w, h: top_h };
    let featured = Rect {
        x: hero.x + hero.w + gap,
        y: top,
        w: (w - pad - hero.x - hero.w - gap).max(180.0),
        h: top_h,
    };
    let row_w = ((w - pad * 2.0 - gap * 2.0) / 3.0).max(120.0);
    let row_y = top + top_h + gap;
    let row = [0, 1, 2].map(|i| Rect {
        x: pad + i as f32 * (row_w + gap),
        y: row_y,
        w: row_w,
        h: row_h,
    });
    HomeLayout { hero, featured, row }
}

fn banner(bytes: &[u8], w: i32, h: i32) -> Image {
    Image::rgba(bytes, w, h).unwrap_or(Image { w: 1, h: 1, rgba: vec![18, 10, 8, 255] })
}

fn ellipsize(font: &mut Font, text: &str, size: i32, weight: i32, max: i32) -> String {
    if font.measure(text, size, weight) <= max { return text.to_string(); }
    let mut out = String::new();
    for ch in text.chars() {
        let trial = format!("{out}{ch}…");
        if font.measure(&trial, size, weight) > max { break; }
        out.push(ch);
    }
    format!("{out}…")
}

fn decode_rgba_file(path: &str) -> Option<Image> {
    // Avatars saved by the launcher are PPM; anything else is skipped rather than misread.
    let _ = path;
    None
}

fn icon(canvas: &mut Canvas, kind: &str, cx: f32, cy: f32, color: [u8; 4]) {
    let s = 8.0;
    match kind {
        "home" => {
            canvas.line(cx - s, cy - 1.0, cx, cy - s, 1.6, color);
            canvas.line(cx, cy - s, cx + s, cy - 1.0, 1.6, color);
            canvas.line(cx - s + 2.0, cy - 1.0, cx - s + 2.0, cy + s - 1.0, 1.6, color);
            canvas.line(cx + s - 2.0, cy - 1.0, cx + s - 2.0, cy + s - 1.0, 1.6, color);
            canvas.line(cx - s + 2.0, cy + s - 1.0, cx + s - 2.0, cy + s - 1.0, 1.6, color);
        }
        "user" => {
            canvas.stroke_circle(cx, cy - 4.0, 3.4, 1.5, color);
            canvas.line(cx - 6.5, cy + 6.5, cx - 4.0, cy + 2.5, 1.5, color);
            canvas.line(cx + 4.0, cy + 2.5, cx + 6.5, cy + 6.5, 1.5, color);
            canvas.line(cx - 6.5, cy + 6.5, cx + 6.5, cy + 6.5, 1.5, color);
        }
        "star" => {
            for i in 0..4 {
                let a = i as f32 * std::f32::consts::FRAC_PI_2 + 0.4;
                canvas.line(cx, cy, cx + a.cos() * 6.5, cy + a.sin() * 6.5, 1.6, color);
            }
            canvas.circle(cx, cy, 1.6, color);
        }
        "gear" => {
            canvas.stroke_circle(cx, cy, 4.0, 1.5, color);
            for i in 0..6 {
                let a = i as f32 * std::f32::consts::TAU / 6.0;
                canvas.line(cx + a.cos() * 6.5, cy + a.sin() * 6.5, cx + a.cos() * 9.0, cy + a.sin() * 9.0, 1.8, color);
            }
        }
        "exit" => {
            canvas.line(cx - 7.0, cy - 7.0, cx - 7.0, cy + 7.0, 1.6, color);
            canvas.line(cx - 7.0, cy - 7.0, cx + 1.0, cy - 7.0, 1.6, color);
            canvas.line(cx - 7.0, cy + 7.0, cx + 1.0, cy + 7.0, 1.6, color);
            canvas.line(cx - 2.0, cy, cx + 8.0, cy, 1.6, color);
            canvas.line(cx + 4.0, cy - 4.0, cx + 8.0, cy, 1.6, color);
            canvas.line(cx + 4.0, cy + 4.0, cx + 8.0, cy, 1.6, color);
        }
        "folder" => {
            canvas.line(cx - 8.0, cy - 3.0, cx - 2.0, cy - 3.0, 1.5, color);
            canvas.line(cx - 2.0, cy - 3.0, cx, cy - 6.0, 1.5, color);
            canvas.line(cx, cy - 6.0, cx + 8.0, cy - 6.0, 1.5, color);
            canvas.line(cx + 8.0, cy - 6.0, cx + 8.0, cy + 6.0, 1.5, color);
            canvas.line(cx - 8.0, cy + 6.0, cx + 8.0, cy + 6.0, 1.5, color);
            canvas.line(cx - 8.0, cy - 3.0, cx - 8.0, cy + 6.0, 1.5, color);
        }
        "play" => {
            canvas.line(cx - 4.0, cy - 6.0, cx + 6.0, cy, 1.8, color);
            canvas.line(cx + 6.0, cy, cx - 4.0, cy + 6.0, 1.8, color);
            canvas.line(cx - 4.0, cy + 6.0, cx - 4.0, cy - 6.0, 1.8, color);
        }
        "check" => {
            canvas.line(cx - 6.0, cy, cx - 2.0, cy + 5.0, 1.8, color);
            canvas.line(cx - 2.0, cy + 5.0, cx + 7.0, cy - 5.0, 1.8, color);
        }
        "spin" => {
            let a = std::f32::consts::TAU * (canvas.w as f32 * 0.0 + 0.15);
            let _ = a;
            canvas.stroke_circle(cx, cy, 6.0, 1.5, [color[0], color[1], color[2], 80]);
            canvas.circle(cx + 5.0, cy - 3.0, 2.0, color);
        }
        _ => {}
    }
}

// Canvas needs a gradient fill used by the play button. Implemented here via shade.
trait Gel {
    fn shade_solid_gradient(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, a: [u8; 3], b: [u8; 3]);
}
impl Gel for Canvas {
    fn shade_solid_gradient(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, a: [u8; 3], b: [u8; 3]) {
        self.shade(x, y, w, h, r, |lx, _, cov| {
            let t = (lx / w).clamp(0.0, 1.0);
            let c = [
                (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
                (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
                (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
            ];
            [c[0], c[1], c[2], (cov * 255.0) as u8]
        });
    }
}

pub fn engine_now_unused() {}
