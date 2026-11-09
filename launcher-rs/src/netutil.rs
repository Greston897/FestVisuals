//! HTTP client for the launcher.
//!
//! TLS is libssl (OpenSSL 3), which is present on the desktops this ships to
//! and is the only TLS stack reachable from the build sandbox — crates.io is
//! blocked there. The dynamic loader is used so the binary still starts, and
//! reports a clear error, on a machine without libssl. Bodies may be gzip or
//! deflate; those are inflated with puff (Mark Adler, public domain).

use std::ffi::CString;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::os::raw::{c_char, c_int, c_long, c_void};
use std::time::Duration;

const UA: &str = "FestVisuals/1.0";

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub final_url: String,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

#[derive(Debug)]
pub struct HttpError(pub String);

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl From<std::io::Error> for HttpError {
    fn from(value: std::io::Error) -> Self {
        HttpError(value.to_string())
    }
}
pub type Result<T> = std::result::Result<T, HttpError>;

#[derive(Clone, Copy)]
pub struct Progress {
    pub done: u64,
    pub total: Option<u64>,
}

pub fn get(url: &str) -> Result<Response> {
    exchange("GET", url, &[], None)
}

pub fn get_bearer(url: &str, token: &str) -> Result<Response> {
    let value = format!("Bearer {token}");
    exchange("GET", url, &[("Authorization", &value)], None)
}

fn exchange(method: &str, url: &str, headers: &[(&str, &str)], body: Option<&[u8]>) -> Result<Response> {
    let mut current = url.to_string();
    for _ in 0..6 {
        let response = transact(method, &current, headers, body)?;
        if matches!(response.status, 301 | 302 | 303 | 307 | 308) {
            let location = response.header("location").ok_or_else(|| HttpError("редирект без Location".into()))?.to_string();
            current = resolve(&current, &location);
            continue;
        }
        return Ok(Response { final_url: current, ..response });
    }
    Err(HttpError("слишком много редиректов".into()))
}

/// Download to disk, following redirects, reporting bytes of the final body.
pub fn download(url: &str, dest: &std::path::Path, progress: &mut dyn FnMut(Progress)) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension(format!("part-{}", std::process::id()));
    let response = exchange("GET", url, &[], None)?;
    if !response.ok() {
        let _ = std::fs::remove_file(&tmp);
        return Err(HttpError(format!("HTTP {} при загрузке {url}", response.status)));
    }
    std::fs::write(&tmp, &response.body)?;
    std::fs::rename(&tmp, dest)?;
    progress(Progress { done: response.body.len() as u64, total: Some(response.body.len() as u64) });
    Ok(())
}

fn transact(method: &str, url: &str, extra: &[(&str, &str)], body: Option<&[u8]>) -> Result<Response> {
    let parts = parse_url(url)?;
    let mut stream = connect(&parts)?;
    let ssl = if parts.tls { Some(tls::connect(&mut stream, &parts.host)?) } else { None };

    let path = if parts.query.is_empty() { parts.path.clone() } else { format!("{}?{}", parts.path, parts.query) };
    let mut head = format!(
        "{method} {path} HTTP/1.0\r\nHost: {}\r\nUser-Agent: {UA}\r\nAccept: */*\r\nAccept-Encoding: gzip, deflate\r\nConnection: close\r\n",
        parts.host_header()
    );
    for (key, value) in extra {
        head.push_str(&format!("{key}: {value}\r\n"));
    }
    if let Some(body) = body {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");

    let mut raw = Vec::new();
    if let Some(ssl) = &ssl {
        ssl.write(head.as_bytes())?;
        if let Some(body) = body {
            ssl.write(body)?;
        }
        let mut buf = [0u8; 32 * 1024];
        loop {
            match ssl.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => raw.extend_from_slice(&buf[..n]),
                Err(error) if raw.is_empty() => return Err(HttpError(format!("чтение TLS: {error}"))),
                Err(_) => break,
            }
        }
    } else {
        stream.write_all(head.as_bytes())?;
        if let Some(body) = body {
            stream.write_all(body)?;
        }
        stream.flush()?;
        stream.read_to_end(&mut raw)?;
    }
    drop(ssl);

    let (status, headers, offset) = parse_head(&raw)?;
    let mut payload = raw[offset..].to_vec();
    if let Some((_, encoding)) = headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-encoding")) {
        payload = decode_body(encoding, &payload)?;
    }
    Ok(Response { status, headers, body: payload, final_url: url.to_string() })
}

fn parse_head(raw: &[u8]) -> Result<(u16, Vec<(String, String)>, usize)> {
    let end = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or_else(|| HttpError("нет конца заголовков".into()))?;
    let text = std::str::from_utf8(&raw[..end]).map_err(|_| HttpError("заголовки не UTF-8".into()))?;
    let mut lines = text.split("\r\n");
    let status: u16 = lines.next().unwrap_or("").split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut headers = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    Ok((status, headers, end + 4))
}

fn decode_body(encoding: &str, data: &[u8]) -> Result<Vec<u8>> {
    let encoding = encoding.to_ascii_lowercase();
    if encoding.contains("gzip") {
        inflate::gunzip(data).map_err(|e| HttpError(format!("gzip: {e}")))
    } else if encoding.contains("deflate") {
        inflate::inflate_zlib(data).or_else(|_| inflate::inflate_raw(data)).map_err(|e| HttpError(format!("deflate: {e}")))
    } else {
        Ok(data.to_vec())
    }
}

struct UrlParts {
    tls: bool,
    host: String,
    port: u16,
    path: String,
    query: String,
}

impl UrlParts {
    fn host_header(&self) -> String {
        let default = if self.tls { 443 } else { 80 };
        if self.port == default { self.host.clone() } else { format!("{}:{}", self.host, self.port) }
    }
}

fn parse_url(url: &str) -> Result<UrlParts> {
    let (tls, rest) = if let Some(rest) = url.strip_prefix("https://") {
        (true, rest)
    } else if let Some(rest) = url.strip_prefix("http://") {
        (false, rest)
    } else {
        return Err(HttpError(format!("неподдерживаемый URL: {url}")));
    };
    let (authority, later) = rest.split_once('/').unwrap_or((rest, ""));
    let authority = authority.split('@').next_back().unwrap_or(authority);
    let (host, port) = if let Some(rest) = authority.strip_prefix('[') {
        let (host, after) = rest.split_once(']').ok_or_else(|| HttpError("битый IPv6".into()))?;
        let port = after.strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or(if tls { 443 } else { 80 });
        (host.to_string(), port)
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        (host.to_string(), port.parse().map_err(|_| HttpError("битый порт".into()))?)
    } else {
        (authority.to_string(), if tls { 443 } else { 80 })
    };
    let (path_str, query) = later.split_once('?').unwrap_or((later, ""));
    let path = if path_str.is_empty() { "/".to_string() } else { format!("/{path_str}") };
    Ok(UrlParts { tls, host, port, path, query: query.to_string() })
}

fn resolve(base: &str, location: &str) -> String {
    if location.starts_with("http://") || location.starts_with("https://") {
        return location.to_string();
    }
    let Ok(parts) = parse_url(base) else { return location.to_string() };
    let scheme = if parts.tls { "https" } else { "http" };
    let origin = format!("{scheme}://{}", parts.host_header());
    if let Some(rest) = location.strip_prefix("//") {
        return format!("{scheme}://{rest}");
    }
    if location.starts_with('/') {
        return format!("{origin}{location}");
    }
    let dir = parts.path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    format!("{origin}{dir}/{location}")
}

fn connect(parts: &UrlParts) -> Result<TcpStream> {
    let mut last = None;
    for addr in (parts.host.as_str(), parts.port).to_socket_addrs()? {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(20)) {
            Ok(stream) => {
                stream.set_read_timeout(Some(Duration::from_secs(90))).ok();
                stream.set_write_timeout(Some(Duration::from_secs(30))).ok();
                stream.set_nodelay(true).ok();
                return Ok(stream);
            }
            Err(error) => last = Some(error),
        }
    }
    Err(HttpError(last.map(|e| e.to_string()).unwrap_or_else(|| format!("нет адреса для {}", parts.host))))
}

// ---------------------------------------------------------------- gzip/deflate

pub mod inflate {
    use std::os::raw::{c_int, c_ulong};
    extern "C" {
        fn puff(dest: *mut u8, destlen: *mut c_ulong, source: *const u8, sourcelen: *mut c_ulong) -> c_int;
    }

    pub fn inflate_raw(data: &[u8]) -> std::result::Result<Vec<u8>, String> {
        let mut cap = (data.len().saturating_mul(5)).max(4096);
        for _ in 0..14 {
            let mut out = vec![0u8; cap];
            let mut dest_len = cap as c_ulong;
            let mut src_len = data.len() as c_ulong;
            let rc = unsafe { puff(out.as_mut_ptr(), &mut dest_len, data.as_ptr(), &mut src_len) };
            if rc == 0 {
                out.truncate(dest_len as usize);
                return Ok(out);
            }
            if rc != 1 {
                return Err(format!("код {rc}"));
            }
            cap = cap.saturating_mul(2).max(cap + 1024);
        }
        Err("результат слишком большой".into())
    }

    pub fn inflate_zlib(data: &[u8]) -> std::result::Result<Vec<u8>, String> {
        if data.len() < 6 || data[0] & 0x0f != 8 {
            return Err("не zlib".into());
        }
        inflate_raw(&data[2..data.len() - 4])
    }

    pub fn gunzip(data: &[u8]) -> std::result::Result<Vec<u8>, String> {
        if data.len() < 18 || data[0] != 0x1f || data[1] != 0x8b {
            return Err("не gzip".into());
        }
        let flags = data[3];
        let mut offset = 10usize;
        if flags & 4 != 0 {
            if data.len() < offset + 2 {
                return Err("битый gzip".into());
            }
            let xlen = u16::from_le_bytes([data[offset], data[offset + 1]]) as usize;
            offset += 2 + xlen;
        }
        for bit in [8u8, 16] {
            if flags & bit != 0 {
                let end = data[offset..].iter().position(|b| *b == 0).ok_or("битый gzip")?;
                offset += end + 1;
            }
        }
        if flags & 2 != 0 {
            offset += 2;
        }
        if data.len() < offset + 8 {
            return Err("битый gzip".into());
        }
        inflate_raw(&data[offset..data.len() - 8])
    }
}

// ---------------------------------------------------------------- TLS via libssl

mod tls {
    use super::*;

    pub struct Ssl {
        ssl: *mut c_void,
        ctx: *mut c_void,
    }
    impl Drop for Ssl {
        fn drop(&mut self) {
            unsafe {
                if !self.ssl.is_null() {
                    (lib().ssl_free)(self.ssl);
                }
                if !self.ctx.is_null() {
                    (lib().ctx_free)(self.ctx);
                }
            }
        }
    }
    unsafe impl Send for Ssl {}

    impl Ssl {
        pub fn write(&self, data: &[u8]) -> Result<()> {
            let mut sent = 0;
            while sent < data.len() {
                let n = unsafe { (lib().ssl_write)(self.ssl, data[sent..].as_ptr() as *const c_void, (data.len() - sent) as c_int) };
                if n <= 0 {
                    return Err(HttpError(lib().last_error("запись TLS")));
                }
                sent += n as usize;
            }
            Ok(())
        }
        pub fn read(&self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = unsafe { (lib().ssl_read)(self.ssl, buf.as_mut_ptr() as *mut c_void, buf.len() as c_int) };
            if n > 0 {
                Ok(n as usize)
            } else if n == 0 {
                Ok(0)
            } else {
                Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, lib().last_error("чтение TLS")))
            }
        }
    }

    pub fn connect(stream: &mut TcpStream, host: &str) -> Result<Ssl> {
        let lib = lib();
        unsafe {
            // 0x0 loads nothing. OPENSSL_INIT_LOAD_SSL_STRINGS | OPENSSL_INIT_LOAD_CRYPTO_STRINGS
            // is what makes SSL_connect actually negotiate instead of returning EOF.
            if (lib.init)(0, std::ptr::null()) != 1 {
                return Err(HttpError("libssl не инициализировался".into()));
            }
            let ctx = (lib.ctx_new)((lib.client_method)());
            if ctx.is_null() {
                return Err(HttpError("libssl не создал контекст".into()));
            }
            (lib.set_verify)(ctx, 1, std::ptr::null_mut());
            if (lib.default_paths)(ctx) != 1 {
                (lib.ctx_free)(ctx);
                return Err(HttpError("нет системного хранилища сертификатов".into()));
            }
            let ssl = (lib.ssl_new)(ctx);
            (lib.set_fd)(ssl, stream_fd(stream));
            let c_host = CString::new(host).unwrap_or_default();
            (lib.ctrl)(ssl, 55, 0, c_host.as_ptr() as *mut c_void);
            (lib.set1_host)(ssl, c_host.as_ptr());
            if (lib.ssl_connect)(ssl) != 1 {
                let why = lib.last_error("рукопожатие TLS");
                (lib.ssl_free)(ssl);
                (lib.ctx_free)(ctx);
                return Err(HttpError(why));
            }
            let verify = (lib.verify_result)(ssl);
            if verify != 0 {
                (lib.ssl_free)(ssl);
                (lib.ctx_free)(ctx);
                return Err(HttpError(format!("сертификат {host} отклонён (X509 {verify})")));
            }
            Ok(Ssl { ssl, ctx })
        }
    }

    fn stream_fd(stream: &TcpStream) -> c_int {
        use std::os::fd::AsRawFd;
        stream.as_raw_fd()
    }

    struct Lib {
        init: unsafe extern "C" fn(u64, *const c_void) -> c_int,
        client_method: unsafe extern "C" fn() -> *const c_void,
        ctx_new: unsafe extern "C" fn(*const c_void) -> *mut c_void,
        ctx_free: unsafe extern "C" fn(*mut c_void),
        set_verify: unsafe extern "C" fn(*mut c_void, c_int, *mut c_void),
        default_paths: unsafe extern "C" fn(*mut c_void) -> c_int,
        ssl_new: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
        ssl_free: unsafe extern "C" fn(*mut c_void),
        set_fd: unsafe extern "C" fn(*mut c_void, c_int) -> c_int,
        ctrl: unsafe extern "C" fn(*mut c_void, c_int, c_long, *mut c_void) -> c_long,
        set1_host: unsafe extern "C" fn(*mut c_void, *const c_char) -> c_int,
        ssl_connect: unsafe extern "C" fn(*mut c_void) -> c_int,
        ssl_read: unsafe extern "C" fn(*mut c_void, *mut c_void, c_int) -> c_int,
        ssl_write: unsafe extern "C" fn(*mut c_void, *const c_void, c_int) -> c_int,
        verify_result: unsafe extern "C" fn(*mut c_void) -> c_long,
        err_get: unsafe extern "C" fn() -> u64,
        err_string: unsafe extern "C" fn(u64, *mut c_char, usize),
    }
    unsafe impl Send for Lib {}
    unsafe impl Sync for Lib {}

    impl Lib {
        fn last_error(&self, what: &str) -> String {
            unsafe {
                let code = (self.err_get)();
                if code == 0 {
                    return format!("{what}: соединение закрыто");
                }
                let mut buf = [0u8; 256];
                (self.err_string)(code, buf.as_mut_ptr() as *mut c_char, buf.len());
                let text = String::from_utf8_lossy(&buf).trim_matches('\0').to_string();
                format!("{what}: {text}")
            }
        }
    }

    fn lib() -> &'static Lib {
        use std::sync::OnceLock;
        static LIB: OnceLock<Lib> = OnceLock::new();
        LIB.get_or_init(|| unsafe { load() })
    }

    unsafe fn load() -> Lib {
        let ssl = dlopen("libssl.so.3\0");
        let crypto = dlopen("libcrypto.so.3\0");
        unsafe fn sym<T>(handle: *mut c_void, name: &str) -> T {
            let c = CString::new(name).unwrap();
            let ptr = libc_dlsym(handle, c.as_ptr());
            if ptr.is_null() {
                panic!("в libssl нет символа {name}");
            }
            std::mem::transmute_copy(&ptr)
        }
        Lib {
            init: sym(ssl, "OPENSSL_init_ssl"),
            client_method: sym(ssl, "TLS_client_method"),
            ctx_new: sym(ssl, "SSL_CTX_new"),
            ctx_free: sym(ssl, "SSL_CTX_free"),
            set_verify: sym(ssl, "SSL_CTX_set_verify"),
            default_paths: sym(ssl, "SSL_CTX_set_default_verify_paths"),
            ssl_new: sym(ssl, "SSL_new"),
            ssl_free: sym(ssl, "SSL_free"),
            set_fd: sym(ssl, "SSL_set_fd"),
            ctrl: sym(ssl, "SSL_ctrl"),
            set1_host: sym(ssl, "SSL_set1_host"),
            ssl_connect: sym(ssl, "SSL_connect"),
            ssl_read: sym(ssl, "SSL_read"),
            ssl_write: sym(ssl, "SSL_write"),
            verify_result: sym(ssl, "SSL_get_verify_result"),
            err_get: sym(crypto, "ERR_get_error"),
            err_string: sym(crypto, "ERR_error_string_n"),
        }
    }

    unsafe fn dlopen(name: &str) -> *mut c_void {
        extern "C" {
            fn dlopen(name: *const c_char, flags: c_int) -> *mut c_void;
        }
        let handle = dlopen(name.as_ptr() as *const c_char, 1);
        if handle.is_null() {
            panic!("не найден {name} — для лаунчера нужен OpenSSL 3 (libssl.so.3)");
        }
        handle
    }
    unsafe fn libc_dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void {
        extern "C" {
            fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
        }
        dlsym(handle, name)
    }
}

pub fn sha1_file(path: &std::path::Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    Ok(sha1(&bytes))
}

pub fn sha1(data: &[u8]) -> String {
    sha1x::hex(data)
}

/// SHA-1 (FIPS 180-1). Minecraft publishes SHA-1 for every library and asset.
mod sha1x {
    pub fn hex(data: &[u8]) -> String {
        let digest = sha1(data);
        digest.iter().map(|b| format!("{b:02x}")).collect()
    }
    fn sha1(data: &[u8]) -> [u8; 20] {
        let mut h = [0x67452301u32, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
        let bit_len = (data.len() as u64) * 8;
        let mut msg = data.to_vec();
        msg.push(0x80);
        while (msg.len() % 64) != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bit_len.to_be_bytes());
        for chunk in msg.chunks(64) {
            let mut w = [0u32; 80];
            for i in 0..16 {
                w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
            }
            for i in 16..80 {
                w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
            }
            let mut a = h;
            for i in 0..80 {
                let (f, k) = match i {
                    0..=19 => ((a[1] & a[2]) | (!a[1] & a[3]), 0x5A827999),
                    20..=39 => (a[1] ^ a[2] ^ a[3], 0x6ED9EBA1),
                    40..=59 => ((a[1] & a[2]) | (a[1] & a[3]) | (a[2] & a[3]), 0x8F1BBCDC),
                    _ => (a[1] ^ a[2] ^ a[3], 0xCA62C1D6),
                };
                let temp = a[0].rotate_left(5).wrapping_add(f).wrapping_add(a[4]).wrapping_add(k).wrapping_add(w[i]);
                a[4] = a[3];
                a[3] = a[2];
                a[2] = a[1].rotate_left(30);
                a[1] = a[0];
                a[0] = temp;
            }
            for i in 0..5 {
                h[i] = h[i].wrapping_add(a[i]);
            }
        }
        let mut out = [0u8; 20];
        for (i, value) in h.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&value.to_be_bytes());
        }
        out
    }
}

/// Minimal JSON reader — enough for the manifests the launcher consumes.
pub mod json {
    #[derive(Clone, Debug)]
    pub enum Value {
        Null,
        Bool(bool),
        Number(f64),
        String(String),
        Array(Vec<Value>),
        Object(Vec<(String, Value)>),
    }

    impl Value {
        pub fn get(&self, key: &str) -> Option<&Value> {
            match self {
                Value::Object(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
        pub fn as_str(&self) -> Option<&str> {
            match self { Value::String(s) => Some(s), _ => None }
        }
        pub fn as_array(&self) -> Option<&[Value]> {
            match self { Value::Array(a) => Some(a), _ => None }
        }
        pub fn as_object(&self) -> Option<&[(String, Value)]> {
            match self { Value::Object(o) => Some(o), _ => None }
        }
        pub fn as_f64(&self) -> Option<f64> {
            match self { Value::Number(n) => Some(*n), _ => None }
        }
        pub fn as_bool(&self) -> Option<bool> {
            match self { Value::Bool(b) => Some(*b), _ => None }
        }
        pub fn as_u64(&self) -> Option<u64> {
            self.as_f64().map(|n| n as u64)
        }
    }

    pub fn parse(input: &str) -> Option<Value> {
        let mut parser = Parser { bytes: input.as_bytes(), index: 0 };
        parser.skip();
        let value = parser.value()?;
        Some(value)
    }

    struct Parser<'a> {
        bytes: &'a [u8],
        index: usize,
    }
    impl Parser<'_> {
        fn skip(&mut self) {
            while matches!(self.bytes.get(self.index), Some(b' ' | b'\n' | b'\r' | b'\t')) {
                self.index += 1;
            }
        }
        fn value(&mut self) -> Option<Value> {
            self.skip();
            match self.bytes.get(self.index)? {
                b'{' => self.object(),
                b'[' => self.array(),
                b'"' => Some(Value::String(self.string()?)),
                b't' => self.literal(b"true", Value::Bool(true)),
                b'f' => self.literal(b"false", Value::Bool(false)),
                b'n' => self.literal(b"null", Value::Null),
                b'-' | b'0'..=b'9' => self.number(),
                _ => None,
            }
        }
        fn literal(&mut self, text: &[u8], value: Value) -> Option<Value> {
            if self.bytes[self.index..].starts_with(text) {
                self.index += text.len();
                Some(value)
            } else {
                None
            }
        }
        fn object(&mut self) -> Option<Value> {
            self.index += 1;
            let mut items = Vec::new();
            loop {
                self.skip();
                if self.bytes.get(self.index) == Some(&b'}') {
                    self.index += 1;
                    break;
                }
                if !items.is_empty() {
                    if self.bytes.get(self.index) != Some(&b',') { return None; }
                    self.index += 1;
                    self.skip();
                }
                let key = self.string()?;
                self.skip();
                if self.bytes.get(self.index) != Some(&b':') { return None; }
                self.index += 1;
                items.push((key, self.value()?));
            }
            Some(Value::Object(items))
        }
        fn array(&mut self) -> Option<Value> {
            self.index += 1;
            let mut items = Vec::new();
            loop {
                self.skip();
                if self.bytes.get(self.index) == Some(&b']') {
                    self.index += 1;
                    break;
                }
                if !items.is_empty() {
                    if self.bytes.get(self.index) != Some(&b',') { return None; }
                    self.index += 1;
                }
                items.push(self.value()?);
            }
            Some(Value::Array(items))
        }
        fn string(&mut self) -> Option<String> {
            self.skip();
            if self.bytes.get(self.index) != Some(&b'"') { return None; }
            self.index += 1;
            let mut out = String::new();
            while let Some(&byte) = self.bytes.get(self.index) {
                self.index += 1;
                match byte {
                    b'"' => return Some(out),
                    b'\\' => {
                        let esc = *self.bytes.get(self.index)?;
                        self.index += 1;
                        match esc {
                            b'"' | b'\\' | b'/' => out.push(esc as char),
                            b'b' => out.push('\u{0008}'),
                            b'f' => out.push('\u{000c}'),
                            b'n' => out.push('\n'),
                            b'r' => out.push('\r'),
                            b't' => out.push('\t'),
                            b'u' => {
                                let hex = self.bytes.get(self.index..self.index + 4)?;
                                self.index += 4;
                                let code = u32::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
                                out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                            }
                            _ => return None,
                        }
                    }
                    byte => {
                        let width = if byte < 0x80 { 1 } else if byte & 0xe0 == 0xc0 { 2 } else if byte & 0xf0 == 0xe0 { 3 } else if byte & 0xf8 == 0xf0 { 4 } else { 1 };
                        let start = self.index - 1;
                        let end = (start + width).min(self.bytes.len());
                        match std::str::from_utf8(&self.bytes[start..end]) {
                            Ok(text) => out.push_str(text),
                            Err(_) => out.push('\u{FFFD}'),
                        }
                        self.index = end;
                    }
                }
            }
            None
        }
        fn number(&mut self) -> Option<Value> {
            let start = self.index;
            if self.bytes.get(self.index) == Some(&b'-') { self.index += 1; }
            while matches!(self.bytes.get(self.index), Some(b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-')) {
                self.index += 1;
            }
            let text = std::str::from_utf8(&self.bytes[start..self.index]).ok()?;
            Some(Value::Number(text.parse().ok()?))
        }
    }

    pub fn escape(text: &str) -> String {
        let mut out = String::from("\"");
        for ch in text.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_of_empty() {
        assert_eq!(sha1(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn sha1_abc() {
        assert_eq!(sha1(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn parses_nested_json() {
        let value = json::parse(r#"{"a":[1,{"b":"ok"}],"c":true}"#).unwrap();
        assert_eq!(value.get("a").unwrap().as_array().unwrap()[1].get("b").unwrap().as_str(), Some("ok"));
        assert_eq!(value.get("c").unwrap().as_bool(), Some(true));
    }
}
