use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::bytes::Reader;
use crate::{Error, Result};
use md5::{Digest, Md5};

pub const MAGIC: &[u8; 4] = b"ERPK";
pub const HEADER_LEN: usize = 0x30;
pub const ZSTD: u8 = 0x11;
pub const STORED: u8 = 0x91;
pub const ZSTD_LEVEL: i32 = 19;
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xb5, 0x2f, 0xfd];

#[derive(Debug, Clone)]
pub struct Fragment {
    pub name: [u8; 4],
    pub offset: u64,
    pub size: u64,
    pub flags: u32,
    pub compression: u8,
    pub packed: u64,
    pub raw: Vec<u8>,
}

impl Fragment {
    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim_end_matches('\0')
            .to_owned()
    }

    pub fn data(&self) -> Result<Vec<u8>> {
        if self.compression == ZSTD || self.compression != STORED && self.raw.starts_with(&ZSTD_MAGIC) {
            Ok(zstd::decode_all(self.raw.as_slice())?)
        } else {
            Ok(self.raw.clone())
        }
    }
}

#[derive(Debug, Clone)]
pub struct Resource {
    pub name: String,
    pub kind: [u8; 16],
    pub unk: Vec<u8>,
    pub fragments: Vec<Fragment>,
    pub hash: [u8; 16],
}

impl Resource {
    pub fn kind(&self) -> String {
        String::from_utf8_lossy(&self.kind)
            .trim_end_matches('\0')
            .to_owned()
    }

    pub fn content_hash(&self) -> Result<[u8; 16]> {
        let mut h = Md5::new();
        for f in &self.fragments {
            h.update(f.data()?);
        }
        Ok(h.finalize().into())
    }

    pub fn like(name: &str, template: &Resource, data: &[Vec<u8>]) -> Result<Self> {
        check_name(name)?;
        if data.len() != template.fragments.len() {
            return Err(Error::Format(format!(
                "{name}: {} fragment(s) given, a {} like {} has {}",
                data.len(),
                template.kind(),
                template.name,
                template.fragments.len()
            )));
        }
        let mut res = Resource {
            name: name.to_owned(),
            kind: template.kind,
            unk: template.unk.clone(),
            fragments: Vec::with_capacity(data.len()),
            hash: [0; 16],
        };
        for (t, d) in template.fragments.iter().zip(data) {
            let mut f = Fragment {
                name: t.name,
                offset: 0,
                size: 0,
                flags: t.flags,
                compression: t.compression,
                packed: 0,
                raw: Vec::new(),
            };
            fill(&mut f, d)?;
            res.fragments.push(f);
        }
        res.hash = res.content_hash()?;
        Ok(res)
    }

    pub fn set_fragment(&mut self, index: usize, data: &[u8]) -> Result<()> {
        let f = self
            .fragments
            .get_mut(index)
            .ok_or_else(|| Error::Format(format!("{}: no fragment {index}", self.name)))?;
        fill(f, data)?;
        self.hash = self.content_hash()?;
        Ok(())
    }
}

fn fill(f: &mut Fragment, data: &[u8]) -> Result<()> {
    f.raw = if f.compression == ZSTD {
        compress(data)?
    } else {
        data.to_vec()
    };
    f.size = data.len() as u64;
    f.packed = f.raw.len() as u64;
    Ok(())
}

pub fn check_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains('\0') || name.len() >= u16::MAX as usize {
        return Err(Error::Format(format!(
            "{name:?} is not a resource name: it must be 1 to {} bytes without NUL",
            u16::MAX - 1
        )));
    }
    Ok(())
}

pub fn kind(name: &str) -> Result<[u8; 16]> {
    let b = name.as_bytes();
    if b.is_empty() || b.len() >= 16 || b.contains(&0) {
        return Err(Error::Format(format!(
            "{name:?} is not a resource type: it must be 1 to 15 bytes without NUL"
        )));
    }
    let mut out = [0; 16];
    out[..b.len()].copy_from_slice(b);
    Ok(out)
}

pub fn compress(data: &[u8]) -> Result<Vec<u8>> {
    compress_at(data, ZSTD_LEVEL)
}

pub fn compress_at(data: &[u8], level: i32) -> Result<Vec<u8>> {
    let mut enc = zstd::Encoder::new(Vec::new(), level)?;
    enc.include_checksum(true)?;
    enc.multithread(1)?;
    std::io::Write::write_all(&mut enc, data)?;
    Ok(enc.finish()?)
}

#[derive(Debug, Clone)]
pub struct Erp {
    pub header: [u8; HEADER_LEN],
    pub version: u32,
    pub resources: Vec<Resource>,
}

impl Erp {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let (mut erp, data_off) = Self::parse_index(data)?;
        for res in &mut erp.resources {
            for f in &mut res.fragments {
                f.raw = Reader::new(data, data_off + f.offset as usize)
                    .take(f.packed as usize)?
                    .to_vec();
            }
        }
        Ok(erp)
    }

    pub fn read_matching(path: &Path, want: impl Fn(&str) -> bool) -> Result<Vec<Resource>> {
        let file = ErpFile::open(path)?;
        file.erp
            .resources
            .iter()
            .filter(|r| want(&r.name))
            .map(|r| file.load(r))
            .collect()
    }

    fn parse_index(data: &[u8]) -> Result<(Self, usize)> {
        let mut r = Reader::new(data, 0);
        let header: [u8; HEADER_LEN] = r.array()?;
        if &header[..4] != MAGIC {
            return Err(Error::Magic {
                offset: 0,
                expected: "ERPK",
            });
        }
        let version = u32::from_le_bytes(header[4..8].try_into().expect("4 bytes"));
        let data_off = u64::from_le_bytes(header[0x20..0x28].try_into().expect("8 bytes")) as usize;
        let count = r.u32()?;
        let _fragments = r.u32()?;
        let mut resources = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let _entry_len = r.u32()?;
            let name_len = r.u16()? as usize;
            let name = String::from_utf8_lossy(r.take(name_len)?)
                .trim_end_matches('\0')
                .to_owned();
            let kind = r.array()?;
            let unk = r.take(if version >= 4 { 6 } else { 4 })?.to_vec();
            let n = r.u8()?;
            let mut fragments = Vec::with_capacity(n as usize);
            for _ in 0..n {
                fragments.push(Fragment {
                    name: r.array()?,
                    offset: r.u64()?,
                    size: r.u64()?,
                    flags: r.u32()?,
                    compression: r.u8()?,
                    packed: r.u64()?,
                    raw: Vec::new(),
                });
            }
            resources.push(Resource {
                name,
                kind,
                unk,
                fragments,
                hash: r.array()?,
            });
        }
        if r.pos != data_off {
            return Err(Error::Format(format!(
                "index ends at {:#x}, data starts at {data_off:#x}",
                r.pos
            )));
        }
        Ok((
            Self {
                header,
                version,
                resources,
            },
            data_off,
        ))
    }

    pub fn find(&self, name: &str) -> Result<&Resource> {
        let mut hits = self.resources.iter().filter(|r| r.name == name);
        match (hits.next(), hits.next()) {
            (Some(r), None) => Ok(r),
            (None, _) => Err(Error::Format(format!("{name}: not found"))),
            _ => Err(Error::Format(format!("{name}: more than one resource"))),
        }
    }

    pub fn find_mut(&mut self, name: &str) -> Result<&mut Resource> {
        self.find(name)?;
        Ok(self
            .resources
            .iter_mut()
            .find(|r| r.name == name)
            .expect("found above"))
    }

    pub fn to_bytes(&mut self) -> Vec<u8> {
        let mut out = self.layout();
        for res in &self.resources {
            for f in &res.fragments {
                out.extend_from_slice(&f.raw);
            }
        }
        out
    }

    fn layout(&mut self) -> Vec<u8> {
        let mut index = Vec::new();
        let mut fragments = 0u32;
        let mut offset = 0u64;
        for res in &mut self.resources {
            let mut e = Vec::new();
            let name_len = res.name.len() + 1;
            e.extend_from_slice(&(name_len as u16).to_le_bytes());
            e.extend_from_slice(res.name.as_bytes());
            e.push(0);
            e.extend_from_slice(&res.kind);
            e.extend_from_slice(&res.unk);
            e.push(res.fragments.len() as u8);
            for f in &mut res.fragments {
                f.offset = offset;
                offset += f.packed;
                e.extend_from_slice(&f.name);
                e.extend_from_slice(&f.offset.to_le_bytes());
                e.extend_from_slice(&f.size.to_le_bytes());
                e.extend_from_slice(&f.flags.to_le_bytes());
                e.push(f.compression);
                e.extend_from_slice(&f.packed.to_le_bytes());
                fragments += 1;
            }
            e.extend_from_slice(&res.hash);
            index.extend_from_slice(&(e.len() as u32).to_le_bytes());
            index.extend_from_slice(&e);
        }
        let info_len = 8 + index.len() as u64;
        let mut header = self.header;
        header[0x10..0x18].copy_from_slice(&(HEADER_LEN as u64).to_le_bytes());
        header[0x18..0x20].copy_from_slice(&info_len.to_le_bytes());
        header[0x20..0x28].copy_from_slice(&(HEADER_LEN as u64 + info_len).to_le_bytes());
        let mut out = Vec::with_capacity(HEADER_LEN + info_len as usize + offset as usize);
        out.extend_from_slice(&header);
        out.extend_from_slice(&(self.resources.len() as u32).to_le_bytes());
        out.extend_from_slice(&fragments.to_le_bytes());
        out.extend_from_slice(&index);
        out
    }
}

pub enum Entry<'a> {
    Copy(&'a Resource),
    Owned(Resource),
}

#[derive(Debug, Clone)]
pub struct ErpFile {
    pub path: std::path::PathBuf,
    pub erp: Erp,
    pub data_off: u64,
}

impl ErpFile {
    pub fn open(path: &Path) -> Result<Self> {
        let mut file = std::fs::File::open(path)?;
        let mut index = vec![0; HEADER_LEN];
        file.read_exact(&mut index)?;
        if &index[..4] != MAGIC {
            return Err(Error::Magic {
                offset: 0,
                expected: "ERPK",
            });
        }
        let data_off = u64::from_le_bytes(index[0x20..0x28].try_into().expect("8 bytes"));
        index.resize(data_off as usize, 0);
        file.read_exact(&mut index[HEADER_LEN..])?;
        Ok(Self {
            path: path.to_path_buf(),
            erp: Erp::parse_index(&index)?.0,
            data_off,
        })
    }

    pub fn load(&self, res: &Resource) -> Result<Resource> {
        let mut file = std::fs::File::open(&self.path)?;
        let mut out = res.clone();
        for f in &mut out.fragments {
            file.seek(SeekFrom::Start(self.data_off + f.offset))?;
            f.raw = vec![0; f.packed as usize];
            file.read_exact(&mut f.raw)?;
        }
        Ok(out)
    }

    pub fn write(&self, entries: Vec<Entry<'_>>, dst: &Path) -> Result<()> {
        let mut spans = Vec::new();
        let mut resources = Vec::with_capacity(entries.len());
        for e in entries {
            match e {
                Entry::Copy(r) => {
                    let mut r = r.clone();
                    for f in &mut r.fragments {
                        spans.push(Some((self.data_off + f.offset, f.packed)));
                        f.raw = Vec::new();
                    }
                    resources.push(r);
                }
                Entry::Owned(r) => {
                    spans.extend(r.fragments.iter().map(|_| None));
                    resources.push(r);
                }
            }
        }
        let mut erp = Erp {
            header: self.erp.header,
            version: self.erp.version,
            resources,
        };
        let head = erp.layout();
        let tmp = dst.with_extension("erp.tmp");
        let mut src = std::fs::File::open(&self.path)?;
        let mut out = std::io::BufWriter::with_capacity(1 << 20, std::fs::File::create(&tmp)?);
        std::io::Write::write_all(&mut out, &head)?;
        let mut buf = vec![0; 1 << 20];
        let frags = erp.resources.iter().flat_map(|r| &r.fragments);
        for (f, span) in frags.zip(spans) {
            match span {
                None => std::io::Write::write_all(&mut out, &f.raw)?,
                Some((at, mut left)) => {
                    src.seek(SeekFrom::Start(at))?;
                    while left > 0 {
                        let n = left.min(buf.len() as u64) as usize;
                        src.read_exact(&mut buf[..n])?;
                        std::io::Write::write_all(&mut out, &buf[..n])?;
                        left -= n as u64;
                    }
                }
            }
        }
        std::io::Write::flush(&mut out)?;
        drop(out);
        std::fs::rename(&tmp, dst)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Erp {
        let mut header = [0u8; HEADER_LEN];
        header[..4].copy_from_slice(MAGIC);
        header[4..8].copy_from_slice(&4u32.to_le_bytes());
        let text = b"!pkg sample payload".to_vec();
        let packed = compress(&text).unwrap();
        let mut kind = [0u8; 16];
        kind[..2].copy_from_slice(b"UI");
        let mut res = Resource {
            name: "eaid://ui_package/screens/sample.ui".into(),
            kind,
            unk: vec![0; 6],
            fragments: vec![
                Fragment {
                    name: *b"temp",
                    offset: 0,
                    size: text.len() as u64,
                    flags: 0,
                    compression: ZSTD,
                    packed: packed.len() as u64,
                    raw: packed,
                },
                Fragment {
                    name: *b"syst",
                    offset: 0,
                    size: 3,
                    flags: 0,
                    compression: 0,
                    packed: 3,
                    raw: b"abc".to_vec(),
                },
            ],
            hash: [0; 16],
        };
        res.hash = res.content_hash().unwrap();
        Erp {
            header,
            version: 4,
            resources: vec![res],
        }
    }

    #[test]
    fn write_parse_write_is_identical() {
        let bytes = sample().to_bytes();
        let mut back = Erp::parse(&bytes).unwrap();
        assert_eq!(back.to_bytes(), bytes);
        let res = back.find("eaid://ui_package/screens/sample.ui").unwrap();
        assert_eq!(res.fragments[0].data().unwrap(), b"!pkg sample payload");
        assert_eq!(res.kind(), "UI");
    }

    #[test]
    fn set_fragment_updates_hash() {
        let mut erp = sample();
        let res = erp.find_mut("eaid://ui_package/screens/sample.ui").unwrap();
        res.set_fragment(0, b"changed").unwrap();
        let hash = res.hash;
        let bytes = erp.to_bytes();
        let back = Erp::parse(&bytes).unwrap();
        let res = &back.resources[0];
        assert_eq!(res.fragments[0].data().unwrap(), b"changed");
        assert_eq!(res.content_hash().unwrap(), hash);
    }

    #[test]
    fn like_takes_the_type_argument_and_fragment_layout_of_a_template() {
        let erp = sample();
        let t = &erp.resources[0];
        let res = Resource::like("eaid://ui_package/x.ui", t, &[b"one".to_vec(), b"two".to_vec()]).unwrap();
        assert_eq!((res.kind, &res.unk), (t.kind, &t.unk));
        for (f, s) in res.fragments.iter().zip(&t.fragments) {
            assert_eq!((f.name, f.flags, f.compression), (s.name, s.flags, s.compression));
        }
        assert_eq!(res.fragments[0].data().unwrap(), b"one");
        assert_ne!(res.fragments[0].raw, b"one");
        assert_eq!(
            (res.fragments[0].size, res.fragments[0].packed),
            (3, res.fragments[0].raw.len() as u64)
        );
        assert_eq!(res.fragments[1].raw, b"two");
        assert_eq!(res.hash, res.content_hash().unwrap());
        assert!(Resource::like("eaid://ui_package/x.ui", t, &[b"one".to_vec()]).is_err());
        assert!(Resource::like("", t, &[b"one".to_vec(), b"two".to_vec()]).is_err());
        assert_eq!(&kind("GfxSurfaceRes").unwrap(), b"GfxSurfaceRes\0\0\0");
        assert!(kind("AIBrakeSettings").is_ok());
        assert!(kind("SixteenBytesLong").is_err());
        assert!(kind("").is_err());
    }

    #[test]
    fn reads_matching_resources_from_a_file() {
        let path = std::env::temp_dir().join(format!("swofty-erp-{}.erp", std::process::id()));
        std::fs::write(&path, sample().to_bytes()).unwrap();
        let found = Erp::read_matching(&path, |n| n.ends_with("sample.ui")).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].fragments[0].data().unwrap(), b"!pkg sample payload");
        assert!(Erp::read_matching(&path, |_| false).unwrap().is_empty());
        std::fs::remove_file(&path).ok();
    }

    fn two() -> Erp {
        let mut erp = sample();
        let mut res = erp.resources[0].clone();
        res.name = "eaid://ui_package/screens/other.ui".into();
        res.set_fragment(0, b"second payload").unwrap();
        erp.resources.push(res);
        erp
    }

    #[test]
    fn streamed_copy_of_every_resource_is_identical() {
        let dir = std::env::temp_dir().join(format!("swofty-erpfile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.erp");
        let bytes = two().to_bytes();
        std::fs::write(&src, &bytes).unwrap();
        let file = ErpFile::open(&src).unwrap();
        let all = file.erp.resources.iter().map(Entry::Copy).collect();
        file.write(all, &dir.join("all.erp")).unwrap();
        assert_eq!(std::fs::read(dir.join("all.erp")).unwrap(), bytes);

        let mut owned = file.load(&file.erp.resources[0]).unwrap();
        owned.name = "eaid://ui_package/screens/renamed.ui".into();
        file.write(
            vec![Entry::Copy(&file.erp.resources[1]), Entry::Owned(owned)],
            &dir.join("subset.erp"),
        )
        .unwrap();
        let back = Erp::parse(&std::fs::read(dir.join("subset.erp")).unwrap()).unwrap();
        let names: Vec<&str> = back.resources.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "eaid://ui_package/screens/other.ui",
                "eaid://ui_package/screens/renamed.ui"
            ]
        );
        assert_eq!(back.resources[0].fragments[0].data().unwrap(), b"second payload");
        assert_eq!(
            back.resources[1].fragments[0].data().unwrap(),
            b"!pkg sample payload"
        );
        assert_eq!(back.resources[1].fragments[1].data().unwrap(), b"abc");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bytes = sample().to_bytes();
        bytes[0] = b'X';
        assert!(matches!(Erp::parse(&bytes), Err(Error::Magic { .. })));
    }
}
