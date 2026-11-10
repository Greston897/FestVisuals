//! Frameless window on Win32. The canvas is blitted as a top-down 32-bit DIB.
//! Layouts below are the x64 Windows ABI.

use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Mutex;

use crate::paint::Canvas;

const _: () = assert!(cfg!(target_pointer_width = "64"));

const WM_DESTROY: u32 = 0x0002;
const WM_SIZE: u32 = 0x0005;
const WM_PAINT: u32 = 0x000F;
const WM_CLOSE: u32 = 0x0010;
const WM_QUIT: u32 = 0x0012;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_KEYDOWN: u32 = 0x0100;
const WM_CHAR: u32 = 0x0102;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_NCLBUTTONDOWN: u32 = 0x00A1;
const WM_GETMINMAXINFO: u32 = 0x0024;
const WS_POPUP: u32 = 0x8000_0000;
const WS_THICKFRAME: u32 = 0x0004_0000;
const WS_MINIMIZEBOX: u32 = 0x0002_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_CLIPCHILDREN: u32 = 0x0200_0000;
const WS_EX_APPWINDOW: u32 = 0x0004_0000;
const CS_VREDRAW: u32 = 0x0001;
const CS_HREDRAW: u32 = 0x0002;
const SW_MINIMIZE: i32 = 6;
const PM_REMOVE: u32 = 1;
const SRCCOPY: u32 = 0x00CC_0020;
const HTCAPTION: usize = 2;
const VK_BACK: usize = 0x08;
const VK_RETURN: usize = 0x0D;
const VK_ESCAPE: usize = 0x1B;
const VK_LEFT: usize = 0x25;
const VK_RIGHT: usize = 0x27;
const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
const OFN_EXPLORER: u32 = 0x0008_0000;
const OFN_NOCHANGEDIR: u32 = 0x0000_0008;
const BIF_RETURNONLYFSDIRS: u32 = 0x0001;
const BIF_NEWDIALOGSTYLE: u32 = 0x0040;
const MB_ICONERROR: u32 = 0x0000_0010;

pub enum Ev {
    Close,
    Mouse { x: f32, y: f32, pressed: Option<bool> },
    Key { text: String, sym: u64 },
    Resize { w: u32, h: u32 },
    Expose,
}

struct Inner {
    hwnd: *mut std::ffi::c_void,
    min_w: i32,
    min_h: i32,
}

struct Frame {
    px: Vec<u8>,
    w: i32,
    h: i32,
}

static WINDOW: AtomicPtr<Inner> = AtomicPtr::new(std::ptr::null_mut());
static EVENTS: Mutex<Vec<Ev>> = Mutex::new(Vec::new());
static FRAME: Mutex<Frame> = Mutex::new(Frame { px: Vec::new(), w: 0, h: 0 });

pub struct Window {
    inner: *mut Inner,
}

impl Window {
    pub fn open(title: &str, w: u32, h: u32) -> Result<Self, String> {
        unsafe {
            let _ = SetProcessDpiAwarenessContext(-4);
            let class = wide("FestVisualsLauncher");
            let cursor = LoadCursorW(std::ptr::null_mut(), 32512isize as *const u16);
            let instance = GetModuleHandleW(std::ptr::null());
            let wnd = WndClassW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfn: Some(wndproc),
                cb_cls_extra: 0,
                cb_wnd_extra: 0,
                h_instance: instance,
                h_icon: std::ptr::null_mut(),
                h_cursor: cursor,
                h_brush: std::ptr::null_mut(),
                menu: std::ptr::null(),
                class_name: class.as_ptr(),
            };
            if RegisterClassW(&wnd) == 0 && GetLastError() != 1410 {
                return Err(format!("не удалось зарегистрировать окно ({})", GetLastError()));
            }
            let style = WS_POPUP | WS_THICKFRAME | WS_MINIMIZEBOX | WS_VISIBLE | WS_CLIPCHILDREN;
            let ex = WS_EX_APPWINDOW;
            let (outer_w, outer_h) = outer_size(w as i32, h as i32, style, ex);
            let (min_w, min_h) = outer_size(1040, 720, style, ex);
            let screen_w = GetSystemMetrics(0);
            let screen_h = GetSystemMetrics(1);
            let x = ((screen_w - outer_w) / 2).max(0);
            let y = ((screen_h - outer_h) / 2).max(0);
            let inner = Box::into_raw(Box::new(Inner {
                hwnd: std::ptr::null_mut(),
                min_w,
                min_h,
            }));
            WINDOW.store(inner, Ordering::SeqCst);
            let title = wide(title);
            let hwnd = CreateWindowExW(
                ex,
                class.as_ptr(),
                title.as_ptr(),
                style,
                x,
                y,
                outer_w,
                outer_h,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null_mut(),
            );
            if hwnd.is_null() {
                let error = GetLastError();
                WINDOW.store(std::ptr::null_mut(), Ordering::SeqCst);
                drop(Box::from_raw(inner));
                return Err(format!("не удалось открыть окно ({error})"));
            }
            (*inner).hwnd = hwnd;
            ShowWindow(hwnd, 5);
            Ok(Self { inner })
        }
    }

    pub fn size(&self) -> (u32, u32) {
        unsafe {
            let mut rc = Rect { left: 0, top: 0, right: 0, bottom: 0 };
            GetClientRect((*self.inner).hwnd, &mut rc);
            ((rc.right - rc.left).max(1) as u32, (rc.bottom - rc.top).max(1) as u32)
        }
    }

    pub fn poll(&mut self) -> Option<Ev> {
        if let Some(ev) = pop_event() {
            return Some(ev);
        }
        unsafe {
            let mut msg = Msg::zero();
            if PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) == 0 {
                return None;
            }
            if msg.message == WM_QUIT {
                return Some(Ev::Close);
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        pop_event()
    }

    pub fn begin_move(&self, _x: f32, _y: f32) {
        let hwnd = unsafe { (*self.inner).hwnd };
        unsafe {
            ReleaseCapture();
            SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION, 0);
        }
    }

    pub fn minimize(&self) {
        unsafe { ShowWindow((*self.inner).hwnd, SW_MINIMIZE); }
    }

    pub fn present(&mut self, canvas: &Canvas) {
        if canvas.w <= 0 || canvas.h <= 0 { return; }
        let px = to_bgra(canvas);
        if let Ok(mut frame) = FRAME.lock() {
            frame.w = canvas.w;
            frame.h = canvas.h;
            frame.px = px;
        }
        unsafe {
            let hwnd = (*self.inner).hwnd;
            let hdc = GetDC(hwnd);
            blit(hwnd, hdc);
            ReleaseDC(hwnd, hdc);
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            if self.inner.is_null() { return; }
            let hwnd = (*self.inner).hwnd;
            WINDOW.store(std::ptr::null_mut(), Ordering::SeqCst);
            if !hwnd.is_null() { DestroyWindow(hwnd); }
            drop(Box::from_raw(self.inner));
            self.inner = std::ptr::null_mut();
        }
    }
}

pub fn alert(text: &str) {
    let text = wide(text);
    let title = wide("FestVisuals");
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), title.as_ptr(), MB_ICONERROR); }
}

pub fn pick_path(directory: bool) -> Option<String> {
    unsafe { let _ = CoInitializeEx(std::ptr::null_mut(), 2); }
    if directory { pick_dir() } else { pick_file() }
}

fn pick_file() -> Option<String> {
    let filter = wide_multi("Картинки\0*.png;*.jpg;*.jpeg;*.webp;*.bmp\0Все файлы\0*.*\0");
    let title = wide("Выбор файла");
    let mut file = vec![0u16; 1024];
    let mut ofn = [0u8; 152];
    ofn[0..4].copy_from_slice(&152u32.to_ne_bytes());
    let owner = owner_hwnd() as usize;
    ofn[8..16].copy_from_slice(&owner.to_ne_bytes());
    let filter_ptr = filter.as_ptr() as usize;
    ofn[24..32].copy_from_slice(&filter_ptr.to_ne_bytes());
    let file_ptr = file.as_mut_ptr() as usize;
    ofn[48..56].copy_from_slice(&file_ptr.to_ne_bytes());
    ofn[56..60].copy_from_slice(&1024u32.to_ne_bytes());
    let title_ptr = title.as_ptr() as usize;
    ofn[88..96].copy_from_slice(&title_ptr.to_ne_bytes());
    let flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_EXPLORER | OFN_NOCHANGEDIR;
    ofn[96..100].copy_from_slice(&flags.to_ne_bytes());
    let ok = unsafe { GetOpenFileNameW(ofn.as_mut_ptr()) };
    if ok == 0 { return None; }
    let path = from_wide(&file);
    if path.is_empty() { None } else { Some(path) }
}

fn pick_dir() -> Option<String> {
    let title = wide("Папка игры");
    let mut display = vec![0u16; 260];
    let mut info = [0u8; 64];
    let owner = owner_hwnd() as usize;
    info[0..8].copy_from_slice(&owner.to_ne_bytes());
    let display_ptr = display.as_mut_ptr() as usize;
    info[16..24].copy_from_slice(&display_ptr.to_ne_bytes());
    let title_ptr = title.as_ptr() as usize;
    info[24..32].copy_from_slice(&title_ptr.to_ne_bytes());
    let flags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
    info[32..36].copy_from_slice(&flags.to_ne_bytes());
    let pidl = unsafe { SHBrowseForFolderW(info.as_mut_ptr()) };
    if pidl.is_null() { return None; }
    let mut path = vec![0u16; 520];
    let ok = unsafe { SHGetPathFromIDListW(pidl, path.as_mut_ptr()) };
    unsafe { CoTaskMemFree(pidl); }
    if ok == 0 { return None; }
    let path = from_wide(&path);
    if path.is_empty() { None } else { Some(path) }
}

fn owner_hwnd() -> *mut std::ffi::c_void {
    let inner = WINDOW.load(Ordering::SeqCst);
    if inner.is_null() { std::ptr::null_mut() } else { unsafe { (*inner).hwnd } }
}

fn pop_event() -> Option<Ev> {
    EVENTS.lock().ok().and_then(|mut q| if q.is_empty() { None } else { Some(q.remove(0)) })
}

fn push_event(ev: Ev) {
    if let Ok(mut q) = EVENTS.lock() { q.push(ev); }
}

fn to_bgra(canvas: &Canvas) -> Vec<u8> {
    let mut out = vec![0u8; (canvas.w * canvas.h * 4) as usize];
    for (i, px) in canvas.px.chunks_exact(4).enumerate() {
        let d = i * 4;
        out[d] = px[2];
        out[d + 1] = px[1];
        out[d + 2] = px[0];
        out[d + 3] = 255;
    }
    out
}

fn blit(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) {
    if hdc.is_null() { return; }
    let Ok(frame) = FRAME.lock() else { return; };
    if frame.px.is_empty() || frame.w <= 0 || frame.h <= 0 { return; }
    let mut client = Rect { left: 0, top: 0, right: frame.w, bottom: frame.h };
    unsafe { GetClientRect(hwnd, &mut client); }
    let dest_w = (client.right - client.left).max(1);
    let dest_h = (client.bottom - client.top).max(1);
    let mut info = [0u8; 44];
    info[0..4].copy_from_slice(&40u32.to_ne_bytes());
    info[4..8].copy_from_slice(&frame.w.to_ne_bytes());
    info[8..12].copy_from_slice(&(-frame.h).to_ne_bytes());
    info[12..14].copy_from_slice(&1u16.to_ne_bytes());
    info[14..16].copy_from_slice(&32u16.to_ne_bytes());
    unsafe {
        StretchDIBits(
            hdc,
            0,
            0,
            dest_w,
            dest_h,
            0,
            0,
            frame.w,
            frame.h,
            frame.px.as_ptr() as *const std::ffi::c_void,
            info.as_ptr() as *const std::ffi::c_void,
            0,
            SRCCOPY,
        );
    }
}

unsafe extern "system" fn wndproc(hwnd: *mut std::ffi::c_void, msg: u32, wparam: usize, lparam: isize) -> isize {
    match msg {
        WM_CLOSE => {
            push_event(Ev::Close);
            0
        }
        WM_DESTROY => 0,
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps = [0u8; 128];
            let hdc = BeginPaint(hwnd, ps.as_mut_ptr() as *mut std::ffi::c_void);
            blit(hwnd, hdc);
            EndPaint(hwnd, ps.as_ptr() as *const std::ffi::c_void);
            push_event(Ev::Expose);
            0
        }
        WM_SIZE => {
            let w = (lparam as u32) & 0xffff;
            let h = ((lparam as u32) >> 16) & 0xffff;
            if w > 0 && h > 0 {
                push_event(Ev::Resize { w, h });
            }
            0
        }
        WM_GETMINMAXINFO => {
            let inner = WINDOW.load(Ordering::SeqCst);
            if !inner.is_null() && !lparam_is_null(lparam) {
                let min_w = (*inner).min_w;
                let min_h = (*inner).min_h;
                // MINMAXINFO.ptMinTrackSize is at offset 24 on x64.
                let base = lparam as *mut u8;
                std::ptr::write_unaligned(base.add(24) as *mut i32, min_w);
                std::ptr::write_unaligned(base.add(28) as *mut i32, min_h);
            }
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = point(lparam);
            push_event(Ev::Mouse { x, y, pressed: None });
            0
        }
        WM_LBUTTONDOWN => {
            SetCapture(hwnd);
            let (x, y) = point(lparam);
            push_event(Ev::Mouse { x, y, pressed: Some(true) });
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            let (x, y) = point(lparam);
            push_event(Ev::Mouse { x, y, pressed: Some(false) });
            0
        }
        WM_KEYDOWN => {
            let sym = match wparam {
                VK_BACK => 0xff08,
                VK_RETURN => 0xff0d,
                VK_ESCAPE => 0xff1b,
                VK_LEFT => 0xff51,
                VK_RIGHT => 0xff53,
                _ => 0,
            };
            if sym != 0 {
                push_event(Ev::Key { text: String::new(), sym });
            }
            0
        }
        WM_CHAR => {
            let code = wparam as u32;
            if code >= 32 && code != 127 {
                if let Some(ch) = char::from_u32(code) {
                    push_event(Ev::Key { text: ch.to_string(), sym: 0 });
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn lparam_is_null(lparam: isize) -> bool { lparam == 0 }

fn point(lparam: isize) -> (f32, f32) {
    let x = (lparam as i16) as f32;
    let y = ((lparam >> 16) as i16) as f32;
    (x, y)
}

fn outer_size(w: i32, h: i32, style: u32, ex: u32) -> (i32, i32) {
    let mut rc = Rect { left: 0, top: 0, right: w, bottom: h };
    unsafe { AdjustWindowRectEx(&mut rc, style, 0, ex); }
    ((rc.right - rc.left).max(w), (rc.bottom - rc.top).max(h))
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_multi(text: &str) -> Vec<u16> {
    let mut out: Vec<u16> = text.encode_utf16().collect();
    out.push(0);
    out
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[repr(C)]
struct WndClassW {
    style: u32,
    lpfn: Option<unsafe extern "system" fn(*mut std::ffi::c_void, u32, usize, isize) -> isize>,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: *mut std::ffi::c_void,
    h_icon: *mut std::ffi::c_void,
    h_cursor: *mut std::ffi::c_void,
    h_brush: *mut std::ffi::c_void,
    menu: *const u16,
    class_name: *const u16,
}

#[repr(C)]
struct Msg {
    hwnd: *mut std::ffi::c_void,
    message: u32,
    _pad: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt_x: i32,
    pt_y: i32,
    lprivate: u32,
}

impl Msg {
    fn zero() -> Self {
        Self {
            hwnd: std::ptr::null_mut(),
            message: 0,
            _pad: 0,
            wparam: 0,
            lparam: 0,
            time: 0,
            pt_x: 0,
            pt_y: 0,
            lprivate: 0,
        }
    }
}

#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const WndClassW) -> u16;
    fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: *mut std::ffi::c_void, menu: *mut std::ffi::c_void, instance: *mut std::ffi::c_void, param: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn DestroyWindow(hwnd: *mut std::ffi::c_void) -> i32;
    fn ShowWindow(hwnd: *mut std::ffi::c_void, cmd: i32) -> i32;
    fn DefWindowProcW(hwnd: *mut std::ffi::c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn PeekMessageW(msg: *mut Msg, hwnd: *mut std::ffi::c_void, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn GetClientRect(hwnd: *mut std::ffi::c_void, rect: *mut Rect) -> i32;
    fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
    fn BeginPaint(hwnd: *mut std::ffi::c_void, ps: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn EndPaint(hwnd: *mut std::ffi::c_void, ps: *const std::ffi::c_void) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn LoadCursorW(instance: *mut std::ffi::c_void, cursor: *const u16) -> *mut std::ffi::c_void;
    fn SetCapture(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ReleaseCapture() -> i32;
    fn SendMessageW(hwnd: *mut std::ffi::c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn AdjustWindowRectEx(rect: *mut Rect, style: u32, menu: i32, ex: u32) -> i32;
    fn MessageBoxW(hwnd: *mut std::ffi::c_void, text: *const u16, title: *const u16, kind: u32) -> i32;
    fn SetProcessDpiAwarenessContext(value: isize) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn StretchDIBits(hdc: *mut std::ffi::c_void, x: i32, y: i32, dw: i32, dh: i32, sx: i32, sy: i32, sw: i32, sh: i32, bits: *const std::ffi::c_void, info: *const std::ffi::c_void, usage: u32, rop: u32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut std::ffi::c_void;
    fn GetLastError() -> u32;
}

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(ofn: *mut u8) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn SHBrowseForFolderW(info: *mut u8) -> *mut std::ffi::c_void;
    fn SHGetPathFromIDListW(pidl: *mut std::ffi::c_void, path: *mut u16) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut std::ffi::c_void, coinit: u32) -> i32;
    fn CoTaskMemFree(pv: *mut std::ffi::c_void);
}
