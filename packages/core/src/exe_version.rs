use std::path::Path;

fn u16le(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}

fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

fn rva_to_offset(d: &[u8], sections: usize, count: u16, rva: u32) -> Option<usize> {
    for i in 0..count as usize {
        let s = sections + i * 40;
        let va = u32le(d, s + 12)?;
        let size = u32le(d, s + 16)?;
        let raw = u32le(d, s + 20)?;
        if rva >= va && rva < va + size {
            return Some((raw + (rva - va)) as usize);
        }
    }
    None
}

/// The game's FileVersion from its PE VERSIONINFO resource, as `major.minor.build.revision`.
pub fn read(exe: &Path) -> Option<String> {
    let d = std::fs::read(exe).ok()?;
    if d.get(0..2)? != b"MZ" {
        return None;
    }
    let pe = u32le(&d, 0x3c)? as usize;
    if d.get(pe..pe + 4)? != b"PE\0\0" {
        return None;
    }
    let count = u16le(&d, pe + 6)?;
    let opt_size = u16le(&d, pe + 20)? as usize;
    let opt = pe + 24;
    let magic = u16le(&d, opt)?;
    // The resource directory is data-directory entry 2; its RVA sits after the
    // directories that precede it, which start further into the 32- vs 64-bit header.
    let dirs = match magic {
        0x20b => opt + 112, // PE32+
        0x10b => opt + 96,  // PE32
        _ => return None,
    };
    let res_rva = u32le(&d, dirs + 2 * 8)?;
    let sections = opt + opt_size;
    let res = rva_to_offset(&d, sections, count, res_rva)?;

    // Walk the resource tree: root -> type RT_VERSION (16) -> name -> language.
    let name_dir = find_entry(&d, res, res, 16)?;
    let lang_dir = res + (find_dir_first(&d, name_dir)? & 0x7fff_ffff) as usize;
    let data_entry = res + (find_dir_first(&d, lang_dir)? & 0x7fff_ffff) as usize;
    let data_rva = u32le(&d, data_entry)?;
    let data = rva_to_offset(&d, sections, count, data_rva)?;
    let len = u32le(&d, data_entry + 4)? as usize;
    let info = d.get(data..data + len)?;

    // VS_FIXEDFILEINFO begins at the signature 0xFEEF04BD.
    let sig = 0xFEEF_04BDu32.to_le_bytes();
    let fixed = info.windows(4).position(|w| w == sig)?;
    let ms = u32le(info, fixed + 8)?;
    let ls = u32le(info, fixed + 12)?;
    Some(format!(
        "{}.{}.{}.{}",
        ms >> 16,
        ms & 0xffff,
        ls >> 16,
        ls & 0xffff
    ))
}

// A resource directory: 16-byte header then entries of 8 bytes each.
fn find_entry(d: &[u8], res: usize, dir: usize, id: u32) -> Option<usize> {
    let named = u16le(d, dir + 12)? as usize;
    let ids = u16le(d, dir + 14)? as usize;
    let first = dir + 16 + named * 8;
    for i in 0..ids {
        let e = first + i * 8;
        if u32le(d, e)? == id {
            let off = u32le(d, e + 4)?;
            if off & 0x8000_0000 != 0 {
                return Some(res + (off & 0x7fff_ffff) as usize);
            }
        }
    }
    None
}

fn find_dir_first(d: &[u8], dir: usize) -> Option<u32> {
    let named = u16le(d, dir + 12)? as usize;
    u32le(d, dir + 16 + named * 8 + 4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_non_pe_file_has_no_version() {
        let dir = std::env::temp_dir().join(format!("swofty-exever-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("not.exe");
        std::fs::write(&f, b"hello").unwrap();
        assert_eq!(read(&f), None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
