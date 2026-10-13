use std::fs;
use std::path::Path;

use crate::error::{AppError, AppResult};
use crate::models::LnkTarget;
use crate::security::expand_env;

const HEADER_SIZE: usize = 0x4C;
const LINK_CLSID: [u8; 16] = [
    0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
];
const HAS_LINK_TARGET_ID_LIST: u32 = 0x0000_0001;
const HAS_LINK_INFO: u32 = 0x0000_0002;
const HAS_NAME: u32 = 0x0000_0004;
const HAS_RELATIVE_PATH: u32 = 0x0000_0008;
const HAS_WORKING_DIR: u32 = 0x0000_0010;
const HAS_ARGUMENTS: u32 = 0x0000_0020;
const HAS_ICON_LOCATION: u32 = 0x0000_0040;
const IS_UNICODE: u32 = 0x0000_0080;

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn u16(&mut self) -> AppResult<u16> {
        let bytes = self.bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> AppResult<u32> {
        let bytes = self.bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn bytes(&mut self, len: usize) -> AppResult<&'a [u8]> {
        if self.remaining() < len {
            return Err(AppError::new("LNK", "Shortcut file ended early."));
        }
        let start = self.pos;
        self.pos += len;
        Ok(&self.data[start..self.pos])
    }

    fn skip(&mut self, len: usize) -> AppResult<()> {
        self.bytes(len).map(|_| ())
    }
}

pub fn parse_lnk_file(path: &Path) -> AppResult<LnkTarget> {
    let bytes = fs::read(path)
        .map_err(|err| AppError::new("LNK", format!("Could not read {}: {err}", path.display())))?;
    let mut target = parse_lnk_bytes(&bytes)?;
    if let Some(path) = target.path.as_mut() {
        *path = expand_env(path);
    }
    if let Some(dir) = target.working_directory.as_mut() {
        *dir = expand_env(dir);
    }
    Ok(target)
}

pub fn parse_lnk_bytes(data: &[u8]) -> AppResult<LnkTarget> {
    if data.len() < HEADER_SIZE {
        return Err(AppError::new("LNK", "Shortcut file is too small."));
    }
    let mut cursor = Cursor { data, pos: 0 };
    let header_size = cursor.u32()? as usize;
    if header_size < HEADER_SIZE {
        return Err(AppError::new("LNK", "Shortcut header size is invalid."));
    }
    let clsid = cursor.bytes(16)?;
    if clsid != LINK_CLSID {
        return Err(AppError::new("LNK", "File is not a Shell Link shortcut."));
    }
    let flags = cursor.u32()?;
    cursor.skip(header_size - 24)?;

    if flags & HAS_LINK_TARGET_ID_LIST != 0 {
        let size = cursor.u16()? as usize;
        cursor.skip(size)?;
    }

    let mut path = None;
    if flags & HAS_LINK_INFO != 0 {
        path = read_link_info(&mut cursor)?;
    }

    let unicode = flags & IS_UNICODE != 0;
    let name = read_string_flag(&mut cursor, flags & HAS_NAME != 0, unicode)?;
    let relative = read_string_flag(&mut cursor, flags & HAS_RELATIVE_PATH != 0, unicode)?;
    let working_directory = read_string_flag(&mut cursor, flags & HAS_WORKING_DIR != 0, unicode)?;
    let arguments = read_string_flag(&mut cursor, flags & HAS_ARGUMENTS != 0, unicode)?;
    let icon_location = read_string_flag(&mut cursor, flags & HAS_ICON_LOCATION != 0, unicode)?;
    if path.is_none() {
        path = relative;
    }

    Ok(LnkTarget {
        path,
        arguments,
        working_directory,
        icon_location,
        name,
    })
}

fn read_link_info(cursor: &mut Cursor<'_>) -> AppResult<Option<String>> {
    let start = cursor.pos;
    let size = cursor.u32()? as usize;
    if size < 0x1C || cursor.remaining() + 4 < size {
        return Err(AppError::new("LNK", "LinkInfo block is truncated."));
    }
    let header_size = cursor.u32()?;
    let flags = cursor.u32()?;
    let _volume_offset = cursor.u32()?;
    let local_offset = cursor.u32()? as usize;
    let _network_offset = cursor.u32()?;
    let _suffix_offset = cursor.u32()?;
    let mut unicode_offset = None;
    if header_size >= 0x24 {
        unicode_offset = Some(cursor.u32()? as usize);
        let _ = cursor.u32()?;
    }
    let end = start + size;
    if end > cursor.data.len() {
        return Err(AppError::new("LNK", "LinkInfo size exceeds the file."));
    }
    let path = if flags & 0x1 != 0 {
        if let Some(offset) = unicode_offset {
            if offset > 0 && start + offset < end {
                read_utf16z(&cursor.data[start + offset..end])
            } else {
                None
            }
        } else {
            None
        }
        .or_else(|| {
            if local_offset > 0 && start + local_offset < end {
                read_ansi_z(&cursor.data[start + local_offset..end])
            } else {
                None
            }
        })
    } else {
        None
    };
    cursor.pos = end;
    Ok(path)
}

fn read_string_flag(
    cursor: &mut Cursor<'_>,
    present: bool,
    unicode: bool,
) -> AppResult<Option<String>> {
    if !present {
        return Ok(None);
    }
    let count = cursor.u16()? as usize;
    if unicode {
        let bytes = cursor.bytes(count * 2)?;
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        Ok(Some(String::from_utf16_lossy(&units)))
    } else {
        let bytes = cursor.bytes(count)?;
        Ok(Some(String::from_utf8_lossy(bytes).into_owned()))
    }
}

fn read_utf16z(data: &[u8]) -> Option<String> {
    let mut units = Vec::new();
    for chunk in data.chunks_exact(2) {
        let unit = u16::from_le_bytes([chunk[0], chunk[1]]);
        if unit == 0 {
            break;
        }
        units.push(unit);
    }
    if units.is_empty() {
        None
    } else {
        Some(String::from_utf16_lossy(&units))
    }
}

fn read_ansi_z(data: &[u8]) -> Option<String> {
    let end = data
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(data.len());
    if end == 0 {
        return None;
    }
    Some(String::from_utf8_lossy(&data[..end]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_u16(buf: &mut Vec<u8>, value: u16) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn push_u32(buf: &mut Vec<u8>, value: u32) {
        buf.extend_from_slice(&value.to_le_bytes());
    }

    fn push_counted_utf16(buf: &mut Vec<u8>, text: &str) {
        let units: Vec<u16> = text.encode_utf16().collect();
        push_u16(buf, units.len() as u16);
        for unit in units {
            push_u16(buf, unit);
        }
    }

    fn header(flags: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        push_u32(&mut buf, HEADER_SIZE as u32);
        buf.extend_from_slice(&LINK_CLSID);
        push_u32(&mut buf, flags);
        buf.resize(HEADER_SIZE, 0);
        buf
    }

    fn link_info(path: &str, unicode_path: Option<&str>) -> Vec<u8> {
        let mut info = vec![0u8; 0x1C];
        let volume = {
            let mut volume = Vec::new();
            push_u32(&mut volume, 18);
            push_u32(&mut volume, 3);
            push_u32(&mut volume, 0);
            push_u32(&mut volume, 16);
            volume.extend_from_slice(b"C\0");
            volume
        };
        let volume_offset = if unicode_path.is_some() { 0x24 } else { 0x1C };
        info.resize(volume_offset, 0);
        let local_offset = info.len() + volume.len();
        info.extend_from_slice(&volume);
        let path_bytes = {
            let mut bytes = path.as_bytes().to_vec();
            bytes.push(0);
            bytes
        };
        info.extend_from_slice(&path_bytes);
        let suffix_offset = info.len();
        info.push(0);
        let unicode_offset = if unicode_path.is_some() {
            let offset = info.len();
            if let Some(text) = unicode_path {
                for unit in text.encode_utf16() {
                    info.extend_from_slice(&unit.to_le_bytes());
                }
                info.extend_from_slice(&0u16.to_le_bytes());
            }
            Some(offset)
        } else {
            None
        };
        if let Some(offset) = unicode_offset {
            info.resize(info.len().max(0x24), 0);
            info[4..8].copy_from_slice(&0x24u32.to_le_bytes());
            let end = info.len() as u32;
            info[28..32].copy_from_slice(&(offset as u32).to_le_bytes());
            info[32..36].copy_from_slice(&end.to_le_bytes());
        } else {
            info[4..8].copy_from_slice(&0x1Cu32.to_le_bytes());
        }
        info[8..12].copy_from_slice(&1u32.to_le_bytes());
        info[12..16].copy_from_slice(&(volume_offset as u32).to_le_bytes());
        info[16..20].copy_from_slice(&(local_offset as u32).to_le_bytes());
        info[20..24].copy_from_slice(&0u32.to_le_bytes());
        info[24..28].copy_from_slice(&(suffix_offset as u32).to_le_bytes());
        let size = info.len() as u32;
        info[0..4].copy_from_slice(&size.to_le_bytes());
        info
    }

    #[test]
    fn parses_ascii_shortcut_with_arguments() {
        let mut data = header(
            HAS_LINK_INFO | HAS_WORKING_DIR | HAS_ARGUMENTS | HAS_ICON_LOCATION | IS_UNICODE,
        );
        data.extend(link_info(r"C:\Tools\Pulse Cut.exe", None));
        push_counted_utf16(&mut data, r"C:\Tools");
        push_counted_utf16(&mut data, "--project \"My App\"");
        push_counted_utf16(&mut data, r"C:\Tools\app.ico");
        let parsed = parse_lnk_bytes(&data).unwrap();
        assert_eq!(parsed.path.as_deref(), Some(r"C:\Tools\Pulse Cut.exe"));
        assert_eq!(parsed.arguments.as_deref(), Some("--project \"My App\""));
        assert_eq!(parsed.working_directory.as_deref(), Some(r"C:\Tools"));
        assert_eq!(parsed.icon_location.as_deref(), Some(r"C:\Tools\app.ico"));
    }

    #[test]
    fn parses_unicode_local_path_and_skips_id_list() {
        let mut data = header(HAS_LINK_TARGET_ID_LIST | HAS_LINK_INFO | IS_UNICODE);
        push_u16(&mut data, 4);
        data.extend_from_slice(&[1, 2, 3, 4]);
        data.extend(link_info(
            "C:\\fallback.exe",
            Some("C:\\Users\\\u{0418}\u{0432}\u{0430}\u{043D}\\app.exe"),
        ));
        let parsed = parse_lnk_bytes(&data).unwrap();
        assert_eq!(
            parsed.path.as_deref(),
            Some("C:\\Users\\\u{0418}\u{0432}\u{0430}\u{043D}\\app.exe")
        );
    }

    #[test]
    fn rejects_truncated_and_foreign_files() {
        assert!(parse_lnk_bytes(b"not a link").is_err());
        let mut data = header(HAS_LINK_INFO);
        data.truncate(20);
        assert!(parse_lnk_bytes(&data).is_err());
    }
}
