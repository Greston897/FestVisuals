//! Just enough ZIP to read `fabric.mod.json` and unpack native libraries.
//! Deflate entries go through puff (Mark Adler, public domain).

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub fn read_entry(path: &Path, name: &str) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    let eocd = find_eocd(&bytes)?;
    let mut cursor = eocd.offset;
    for _ in 0..eocd.count {
        if bytes.get(cursor..cursor + 4) != Some(&[0x50, 0x4b, 0x01, 0x02]) { return None; }
        let method = u16_at(&bytes, cursor + 10)?;
        let compressed = u32_at(&bytes, cursor + 20)? as usize;
        let name_len = u16_at(&bytes, cursor + 28)? as usize;
        let extra_len = u16_at(&bytes, cursor + 30)? as usize;
        let comment_len = u16_at(&bytes, cursor + 32)? as usize;
        let local = u32_at(&bytes, cursor + 42)? as usize;
        let entry = std::str::from_utf8(bytes.get(cursor + 46..cursor + 46 + name_len)?).ok()?;
        cursor += 46 + name_len + extra_len + comment_len;
        if entry != name { continue; }
        return inflate_local(&bytes, local, method, compressed);
    }
    None
}

pub fn extract_all(archive: &Path, dest: &Path) -> Option<()> {
    let bytes = std::fs::read(archive).ok()?;
    let eocd = find_eocd(&bytes)?;
    let mut cursor = eocd.offset;
    for _ in 0..eocd.count {
        if bytes.get(cursor..cursor + 4) != Some(&[0x50, 0x4b, 0x01, 0x02]) { return None; }
        let method = u16_at(&bytes, cursor + 10)?;
        let compressed = u32_at(&bytes, cursor + 20)? as usize;
        let name_len = u16_at(&bytes, cursor + 28)? as usize;
        let extra_len = u16_at(&bytes, cursor + 30)? as usize;
        let comment_len = u16_at(&bytes, cursor + 32)? as usize;
        let local = u32_at(&bytes, cursor + 42)? as usize;
        let name = std::str::from_utf8(bytes.get(cursor + 46..cursor + 46 + name_len)?).unwrap_or("").to_string();
        cursor += 46 + name_len + extra_len + comment_len;
        if name.ends_with('/') || name.contains("..") { continue; }
        let Some(data) = inflate_local(&bytes, local, method, compressed) else { continue };
        let target = dest.join(&name);
        if let Some(parent) = target.parent() { std::fs::create_dir_all(parent).ok()?; }
        std::fs::write(target, data).ok()?;
    }
    Some(())
}

struct Eocd { count: usize, offset: usize }

fn find_eocd(bytes: &[u8]) -> Option<Eocd> {
    let start = bytes.len().saturating_sub(65557);
    for index in (start..=bytes.len().saturating_sub(22)).rev() {
        if bytes.get(index..index + 4) == Some(&[0x50, 0x4b, 0x05, 0x06]) {
            return Some(Eocd { count: u16_at(bytes, index + 10)? as usize, offset: u32_at(bytes, index + 16)? as usize });
        }
    }
    None
}

fn inflate_local(bytes: &[u8], local: usize, method: u16, compressed: usize) -> Option<Vec<u8>> {
    if bytes.get(local..local + 4) != Some(&[0x50, 0x4b, 0x03, 0x04]) { return None; }
    let name_len = u16_at(bytes, local + 26)? as usize;
    let extra_len = u16_at(bytes, local + 28)? as usize;
    let start = local + 30 + name_len + extra_len;
    let raw = bytes.get(start..start + compressed)?;
    match method {
        0 => Some(raw.to_vec()),
        8 => crate::netutil::inflate::inflate_raw(raw).ok(),
        _ => None,
    }
}

fn u16_at(bytes: &[u8], index: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(index..index + 2)?.try_into().ok()?))
}
fn u32_at(bytes: &[u8], index: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(index..index + 4)?.try_into().ok()?))
}

#[allow(dead_code)]
fn _seek_marker(path: &Path) -> bool {
    std::fs::File::open(path).and_then(|mut file| {
        file.seek(SeekFrom::End(-22))?;
        let mut buf = [0u8; 4];
        file.read_exact(&mut buf)?;
        Ok(buf == [0x50, 0x4b, 0x05, 0x06])
    }).unwrap_or(false)
}
