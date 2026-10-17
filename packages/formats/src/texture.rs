use crate::erp::Resource;
use crate::{Error, Result};

const BC7: u32 = 0x46;

pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn decode(surface: &Resource) -> Result<Image> {
    let bad = |why: &str| Error::Format(format!("{}: {why}", surface.name));
    let [head, pixels, ..] = surface.fragments.as_slice() else {
        return Err(bad("not a surface"));
    };
    let head = head.data()?;
    let word = |at: usize| {
        head.get(at..at + 4)
            .map(|b| u32::from_le_bytes(b.try_into().expect("4 bytes")))
    };
    let (Some(format), Some(width), Some(height)) = (word(8), word(12), word(16)) else {
        return Err(bad("short surface header"));
    };
    if format != BC7 {
        return Err(bad(&format!("unsupported surface format {format:#x}")));
    }
    let (bw, bh) = (width.div_ceil(4) as usize, height.div_ceil(4) as usize);
    let data = pixels.data()?;
    if data.len() < bw * bh * 16 {
        return Err(bad("surface data is shorter than its size"));
    }
    let pitch = bw * 16;
    let mut padded = vec![0; pitch * bh * 4];
    for (i, block) in data.chunks_exact(16).take(bw * bh).enumerate() {
        bcdec_rs::bc7(block, &mut padded[i / bw * 4 * pitch + i % bw * 16..], pitch);
    }
    let row = width as usize * 4;
    let rgba = padded
        .chunks(pitch)
        .take(height as usize)
        .flat_map(|r| &r[..row])
        .copied()
        .collect();
    Ok(Image { width, height, rgba })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::erp::Fragment;

    fn fragment(raw: Vec<u8>) -> Fragment {
        Fragment {
            name: *b"temp",
            offset: 0,
            size: raw.len() as u64,
            flags: 0,
            compression: 0,
            packed: raw.len() as u64,
            raw,
        }
    }

    fn surface(format: u32, width: u32, height: u32, data: Vec<u8>) -> Resource {
        let mut head = vec![0; 40];
        for (at, v) in [(8, format), (12, width), (16, height)] {
            head[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        Resource {
            name: "flag.tif.image".into(),
            kind: [0; 16],
            unk: vec![0; 4],
            fragments: vec![fragment(head), fragment(data)],
            hash: [0; 16],
        }
    }

    #[test]
    fn decodes_and_crops_bc7() {
        let block = vec![
            0x40, 0x20, 0x10, 0x02, 0xf9, 0xff, 0xff, 0xff, 0x01, 0, 0, 0, 0, 0, 0, 0,
        ];
        let image = decode(&surface(BC7, 3, 2, block)).unwrap();
        assert_eq!((image.width, image.height), (3, 2));
        assert_eq!(image.rgba, [0x81, 0x21, 0xff, 0xff].repeat(6));
    }

    #[test]
    fn rejects_other_formats() {
        assert!(decode(&surface(0x47, 4, 4, vec![0; 16])).is_err());
    }
}
