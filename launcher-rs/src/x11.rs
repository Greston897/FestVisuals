//! A frameless window on libX11. The library is loaded at runtime so the binary
//! still explains itself on a machine without X.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_long, c_uint, c_ulong, c_void};

use crate::paint::Canvas;

const KEY_PRESS: i32 = 2;
const BUTTON_PRESS: i32 = 4;
const BUTTON_RELEASE: i32 = 5;
const MOTION: i32 = 6;
const EXPOSE: i32 = 12;
const CONFIGURE: i32 = 22;
const CLIENT_MESSAGE: i32 = 33;
const EVENT_MASK: c_long = (1 << 0) | (1 << 2) | (1 << 3) | (1 << 6) | (1 << 15) | (1 << 17);

pub enum Ev {
    Close,
    Mouse { x: f32, y: f32, pressed: Option<bool> },
    Key { text: String, sym: u64 },
    Resize { w: u32, h: u32 },
    Expose,
}

pub struct Window {
    dpy: *mut c_void,
    win: u64,
    root: u64,
    gc: *mut c_void,
    wm_delete: u64,
    moveresize: u64,
    width: u32,
    height: u32,
    red: u32,
    green: u32,
    blue: u32,
    ic: *mut c_void,
}

impl Window {
    pub fn open(title: &str, w: u32, h: u32) -> Result<Self, String> {
        unsafe {
            setlocale(6, b"\0".as_ptr() as *const c_char);
            let lib = lib()?;
            let dpy = (lib.open)(std::ptr::null());
            if dpy.is_null() {
                return Err("нет дисплея — запусти из графической сессии или собери кадр: festvisuals --snapshot preview.png".into());
            }
            let screen = (lib.default_screen)(dpy);
            let root = (lib.root)(dpy, screen);
            let depth = (lib.depth)(dpy, screen);
            let visual = (lib.visual)(dpy, screen);
            let black = (lib.black)(dpy, screen);
            let win = (lib.simple)(dpy, root, 80, 60, w as c_uint, h as c_uint, 0, black, black);
            (lib.select)(dpy, win, EVENT_MASK);
            let c_title = CString::new(title).unwrap_or_default();
            (lib.store_name)(dpy, win, c_title.as_ptr());
            let utf8 = (lib.intern)(dpy, b"UTF8_STRING\0".as_ptr() as *const c_char, 0);
            let net_name = (lib.intern)(dpy, b"_NET_WM_NAME\0".as_ptr() as *const c_char, 0);
            (lib.change)(dpy, win, net_name, utf8, 8, 0, c_title.as_ptr() as *const u8, title.len() as c_int);
            let motif = (lib.intern)(dpy, b"_MOTIF_WM_HINTS\0".as_ptr() as *const c_char, 0);
            let hints: [c_long; 5] = [2, 0, 0, 0, 0];
            (lib.change)(dpy, win, motif, motif, 32, 0, hints.as_ptr() as *const u8, 5);
            let wm_delete = (lib.intern)(dpy, b"WM_DELETE_WINDOW\0".as_ptr() as *const c_char, 0);
            let protocols = (lib.intern)(dpy, b"WM_PROTOCOLS\0".as_ptr() as *const c_char, 0);
            (lib.protocols)(dpy, win, &wm_delete, 1);
            let _ = protocols;
            let mut size = SizeHints { flags: 16, min_width: 1040, min_height: 720, width: w as i32, height: h as i32, ..SizeHints::zero() };
            size.flags = 16 | 8;
            (lib.normal_hints)(dpy, win, &size);
            let gc = (lib.gc)(dpy, win, 0, std::ptr::null());
            (lib.map)(dpy, win);
            (lib.flush)(dpy);
            let (red, green, blue) = masks(visual);
            let ic = open_ic(lib, dpy, win);
            let moveresize = (lib.intern)(dpy, b"_NET_WM_MOVERESIZE\0".as_ptr() as *const c_char, 0);
            Ok(Self { dpy, win, root, gc, wm_delete, moveresize, width: w, height: h, red, green, blue, ic })
        }
    }

    pub fn size(&self) -> (u32, u32) { (self.width, self.height) }

    pub fn poll(&mut self) -> Option<Ev> {
        let lib = lib().ok()?;
        unsafe {
            if (lib.pending)(self.dpy) == 0 { return None; }
            let mut event = [0u8; 256];
            (lib.next)(self.dpy, event.as_mut_ptr() as *mut c_void);
            let kind = i32::from_ne_bytes(event[0..4].try_into().unwrap());
            match kind {
                CLIENT_MESSAGE => {
                    let atom = u64::from_ne_bytes(event[56..64].try_into().unwrap());
                    if atom == self.wm_delete { Some(Ev::Close) } else { None }
                }
                CONFIGURE => {
                    let w = u32::from_ne_bytes(event[56..60].try_into().unwrap());
                    let h = u32::from_ne_bytes(event[60..64].try_into().unwrap());
                    if w > 0 && h > 0 && (w != self.width || h != self.height) {
                        self.width = w;
                        self.height = h;
                        Some(Ev::Resize { w, h })
                    } else { None }
                }
                EXPOSE => Some(Ev::Expose),
                MOTION => {
                    let x = i32::from_ne_bytes(event[64..68].try_into().unwrap());
                    let y = i32::from_ne_bytes(event[68..72].try_into().unwrap());
                    Some(Ev::Mouse { x: x as f32, y: y as f32, pressed: None })
                }
                BUTTON_PRESS | BUTTON_RELEASE => {
                    let x = i32::from_ne_bytes(event[64..68].try_into().unwrap());
                    let y = i32::from_ne_bytes(event[68..72].try_into().unwrap());
                    Some(Ev::Mouse { x: x as f32, y: y as f32, pressed: Some(kind == BUTTON_PRESS) })
                }
                KEY_PRESS => Some(self.lookup(lib, event.as_mut_ptr())),
                _ => None,
            }
        }
    }

    fn lookup(&self, lib: &Lib, event: *mut u8) -> Ev {
        unsafe {
            let mut buf = [0u8; 64];
            let mut n = 0;
            if !self.ic.is_null() {
                if let Some(lookup) = lib.utf8 {
                    n = lookup(self.ic, event as *mut c_void, buf.as_mut_ptr() as *mut c_char, buf.len() as c_int, std::ptr::null_mut(), std::ptr::null_mut());
                }
            }
            if n <= 0 {
                n = (lib.lookup)(event as *mut c_void, buf.as_mut_ptr() as *mut c_char, buf.len() as c_int, std::ptr::null_mut(), std::ptr::null_mut());
            }
            let text = if n > 0 { String::from_utf8_lossy(&buf[..n as usize]).into_owned() } else { String::new() };
            let sym = (lib.keysym)(event as *mut c_void, 0);
            Ev::Key { text, sym }
        }
    }

    pub fn begin_move(&self, x: f32, y: f32) {
        let Ok(lib) = lib() else { return };
        unsafe {
            let mut ev = [0u8; 256];
            ev[0..4].copy_from_slice(&CLIENT_MESSAGE.to_ne_bytes());
            ev[32..40].copy_from_slice(&self.win.to_ne_bytes());
            ev[40..48].copy_from_slice(&self.moveresize.to_ne_bytes());
            ev[48..52].copy_from_slice(&32i32.to_ne_bytes());
            let root_x = x as c_long;
            let root_y = y as c_long;
            // data.l starts at offset 56. These are window-local; the WM wants root coords.
            // Query the pointer so the drag starts under the cursor.
            let (rx, ry) = pointer_root(lib, self.dpy, self.win, self.root);
            ev[56..64].copy_from_slice(&(rx as c_long).to_ne_bytes());
            ev[64..72].copy_from_slice(&(ry as c_long).to_ne_bytes());
            ev[72..80].copy_from_slice(&8i64.to_ne_bytes());
            ev[80..88].copy_from_slice(&1i64.to_ne_bytes());
            ev[88..96].copy_from_slice(&1i64.to_ne_bytes());
            let _ = (root_x, root_y);
            (lib.send)(self.dpy, self.root, 0, (1 << 19) | (1 << 20), ev.as_ptr() as *mut c_void);
            (lib.flush)(self.dpy);
        }
    }

    pub fn minimize(&self) {
        if let Ok(lib) = lib() {
            unsafe { (lib.iconify)(self.dpy, self.win, (lib.default_screen)(self.dpy)); }
        }
    }

    pub fn present(&mut self, canvas: &Canvas) {
        let Ok(lib) = lib() else { return };
        if canvas.w <= 0 || canvas.h <= 0 { return; }
        self.width = canvas.w as u32;
        self.height = canvas.h as u32;
        unsafe {
            let stride = canvas.w * 4;
            let bytes = (stride * canvas.h) as usize;
            let data = malloc(bytes);
            if data.is_null() { return; }
            let buf = std::slice::from_raw_parts_mut(data as *mut u8, bytes);
            for y in 0..canvas.h {
                for x in 0..canvas.w {
                    let s = ((y * canvas.w + x) * 4) as usize;
                    let px = pack(canvas.px[s], canvas.px[s + 1], canvas.px[s + 2], self.red, self.green, self.blue);
                    let d = ((y * canvas.w + x) * 4) as usize;
                    buf[d..d + 4].copy_from_slice(&px.to_ne_bytes());
                }
            }
            let visual = (lib.visual)(self.dpy, (lib.default_screen)(self.dpy));
            let depth = (lib.depth)(self.dpy, (lib.default_screen)(self.dpy)) as c_uint;
            let image = (lib.create_image)(self.dpy, visual, depth, 2, 0, data as *mut c_char, canvas.w as c_uint, canvas.h as c_uint, 32, stride);
            if !image.is_null() {
                (lib.put_image)(self.dpy, self.win, self.gc, image, 0, 0, 0, 0, canvas.w as c_uint, canvas.h as c_uint);
                (lib.destroy_image)(image);
            } else {
                free(data);
            }
            (lib.flush)(self.dpy);
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Ok(lib) = lib() {
            unsafe {
                (lib.destroy)(self.dpy, self.win);
                (lib.close)(self.dpy);
            }
        }
    }
}

fn pointer_root(lib: &Lib, dpy: *mut c_void, win: u64, root: u64) -> (i32, i32) {
    unsafe {
        let mut root_ret = root;
        let mut child = 0u64;
        let mut rx = 0;
        let mut ry = 0;
        let mut wx = 0;
        let mut wy = 0;
        let mut mask = 0;
        (lib.query)(dpy, win, &mut root_ret, &mut child, &mut rx, &mut ry, &mut wx, &mut wy, &mut mask);
        (rx, ry)
    }
}

fn masks(visual: *mut c_void) -> (u32, u32, u32) {
    if visual.is_null() { return (0x00ff_0000, 0x0000_ff00, 0x0000_00ff); }
    unsafe {
        let p = visual as *const u8;
        let red = *(p.add(24) as *const u64) as u32;
        let green = *(p.add(32) as *const u64) as u32;
        let blue = *(p.add(40) as *const u64) as u32;
        if red == 0 { (0x00ff_0000, 0x0000_ff00, 0x0000_00ff) } else { (red, green, blue) }
    }
}

fn pack(r: u8, g: u8, b: u8, red: u32, green: u32, blue: u32) -> u32 {
    shift(r, red) | shift(g, green) | shift(b, blue)
}

fn shift(channel: u8, mask: u32) -> u32 {
    if mask == 0 { return 0; }
    let bits = mask.count_ones();
    ((channel as u32) >> (8 - bits)) << mask.trailing_zeros()
}

fn open_ic(lib: &Lib, dpy: *mut c_void, win: u64) -> *mut c_void {
    unsafe {
        let Some(open_im) = lib.open_im else { return std::ptr::null_mut() };
        let Some(create_ic) = lib.create_ic else { return std::ptr::null_mut() };
        let im = open_im(dpy, std::ptr::null_mut(), std::ptr::null(), std::ptr::null());
        if im.is_null() { return std::ptr::null_mut(); }
        create_ic(
            im,
            b"inputStyle\0".as_ptr() as *const c_char,
            (0x0008u64 | 0x0400) as c_ulong,
            b"clientWindow\0".as_ptr() as *const c_char,
            win,
            std::ptr::null::<c_char>(),
        )
    }
}

#[repr(C)]
struct SizeHints {
    flags: c_long,
    x: c_int, y: c_int,
    width: c_int, height: c_int,
    min_width: c_int, min_height: c_int,
    max_width: c_int, max_height: c_int,
    width_inc: c_int, height_inc: c_int,
    min_aspect_x: c_int, min_aspect_y: c_int,
    max_aspect_x: c_int, max_aspect_y: c_int,
    base_width: c_int, base_height: c_int,
    win_gravity: c_int,
}
impl SizeHints {
    fn zero() -> Self {
        Self { flags: 0, x: 0, y: 0, width: 0, height: 0, min_width: 0, min_height: 0, max_width: 0, max_height: 0, width_inc: 0, height_inc: 0, min_aspect_x: 0, min_aspect_y: 0, max_aspect_x: 0, max_aspect_y: 0, base_width: 0, base_height: 0, win_gravity: 0 }
    }
}

struct Lib {
    open: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    close: unsafe extern "C" fn(*mut c_void) -> c_int,
    default_screen: unsafe extern "C" fn(*mut c_void) -> c_int,
    root: unsafe extern "C" fn(*mut c_void, c_int) -> u64,
    depth: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
    visual: unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void,
    black: unsafe extern "C" fn(*mut c_void, c_int) -> c_ulong,
    simple: unsafe extern "C" fn(*mut c_void, u64, c_int, c_int, c_uint, c_uint, c_uint, c_ulong, c_ulong) -> u64,
    select: unsafe extern "C" fn(*mut c_void, u64, c_long) -> c_int,
    store_name: unsafe extern "C" fn(*mut c_void, u64, *const c_char) -> c_int,
    intern: unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> u64,
    change: unsafe extern "C" fn(*mut c_void, u64, u64, u64, c_int, c_int, *const u8, c_int) -> c_int,
    protocols: unsafe extern "C" fn(*mut c_void, u64, *const u64, c_int) -> c_int,
    normal_hints: unsafe extern "C" fn(*mut c_void, u64, *const SizeHints),
    gc: unsafe extern "C" fn(*mut c_void, u64, c_ulong, *const c_void) -> *mut c_void,
    map: unsafe extern "C" fn(*mut c_void, u64) -> c_int,
    flush: unsafe extern "C" fn(*mut c_void) -> c_int,
    pending: unsafe extern "C" fn(*mut c_void) -> c_int,
    next: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int,
    lookup: unsafe extern "C" fn(*mut c_void, *mut c_char, c_int, *mut u64, *mut c_void) -> c_int,
    keysym: unsafe extern "C" fn(*mut c_void, c_int) -> u64,
    utf8: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_char, c_int, *mut u64, *mut c_int) -> c_int>,
    open_im: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *const c_char, *const c_char) -> *mut c_void>,
    create_ic: Option<unsafe extern "C" fn(*mut c_void, ...) -> *mut c_void>,
    send: unsafe extern "C" fn(*mut c_void, u64, c_int, c_long, *mut c_void) -> c_int,
    iconify: unsafe extern "C" fn(*mut c_void, u64, c_int) -> c_int,
    query: unsafe extern "C" fn(*mut c_void, u64, *mut u64, *mut u64, *mut c_int, *mut c_int, *mut c_int, *mut c_int, *mut c_uint) -> c_int,
    create_image: unsafe extern "C" fn(*mut c_void, *mut c_void, c_uint, c_int, c_int, *mut c_char, c_uint, c_uint, c_int, c_int) -> *mut c_void,
    put_image: unsafe extern "C" fn(*mut c_void, u64, *mut c_void, *mut c_void, c_int, c_int, c_int, c_int, c_uint, c_uint) -> c_int,
    destroy_image: unsafe extern "C" fn(*mut c_void) -> c_int,
    destroy: unsafe extern "C" fn(*mut c_void, u64) -> c_int,
}

fn lib() -> Result<&'static Lib, String> {
    use std::sync::OnceLock;
    static LIB: OnceLock<Result<Lib, String>> = OnceLock::new();
    match LIB.get_or_init(|| unsafe { load() }) {
        Ok(lib) => Ok(lib),
        Err(e) => Err(e.clone()),
    }
}

unsafe fn load() -> Result<Lib, String> {
    let handle = dlopen(b"libX11.so.6\0");
    if handle.is_null() {
        return Err("нет libX11.so.6".into());
    }
    unsafe fn must<T>(handle: *mut c_void, name: &str) -> T {
        let c = CString::new(name).unwrap();
        let ptr = dlsym(handle, c.as_ptr());
        if ptr.is_null() { panic!("в libX11 нет {name}"); }
        std::mem::transmute_copy(&ptr)
    }
    unsafe fn maybe<T>(handle: *mut c_void, name: &str) -> Option<T> {
        let c = CString::new(name).unwrap();
        let ptr = dlsym(handle, c.as_ptr());
        if ptr.is_null() { None } else { Some(std::mem::transmute_copy(&ptr)) }
    }
    Ok(Lib {
        open: must(handle, "XOpenDisplay"),
        close: must(handle, "XCloseDisplay"),
        default_screen: must(handle, "XDefaultScreen"),
        root: must(handle, "XRootWindow"),
        depth: must(handle, "XDefaultDepth"),
        visual: must(handle, "XDefaultVisual"),
        black: must(handle, "XBlackPixel"),
        simple: must(handle, "XCreateSimpleWindow"),
        select: must(handle, "XSelectInput"),
        store_name: must(handle, "XStoreName"),
        intern: must(handle, "XInternAtom"),
        change: must(handle, "XChangeProperty"),
        protocols: must(handle, "XSetWMProtocols"),
        normal_hints: must(handle, "XSetWMNormalHints"),
        gc: must(handle, "XCreateGC"),
        map: must(handle, "XMapWindow"),
        flush: must(handle, "XFlush"),
        pending: must(handle, "XPending"),
        next: must(handle, "XNextEvent"),
        lookup: must(handle, "XLookupString"),
        keysym: must(handle, "XLookupKeysym"),
        utf8: maybe(handle, "Xutf8LookupString"),
        open_im: maybe(handle, "XOpenIM"),
        create_ic: maybe(handle, "XCreateIC"),
        send: must(handle, "XSendEvent"),
        iconify: must(handle, "XIconifyWindow"),
        query: must(handle, "XQueryPointer"),
        create_image: must(handle, "XCreateImage"),
        put_image: must(handle, "XPutImage"),
        destroy_image: must(handle, "XDestroyImage"),
        destroy: must(handle, "XDestroyWindow"),
    })
}

unsafe fn dlopen(name: &[u8]) -> *mut c_void {
    extern "C" { fn dlopen(name: *const c_char, flags: c_int) -> *mut c_void; }
    dlopen(name.as_ptr() as *const c_char, 1)
}
unsafe fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void {
    extern "C" { fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void; }
    dlsym(handle, name)
}
extern "C" {
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}
