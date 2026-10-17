use std::collections::{HashMap, VecDeque};

use crate::bytes::{Reader, put_u32, u32_at};
use crate::hash::fnv1a32;
use crate::{Error, Result};

pub type Tag = [u8; 4];

pub const STRI: Tag = *b"stri";
const CHUNK: u32 = 0x80;
const OFFSET_MASK: u32 = 0x00ff_ffff;

fn align_of(ty: &Tag) -> usize {
    match ty {
        b"ui64" | b"si64" => 8,
        b"vec3" | b"vec4" | b"quat" | b"woid" | b"shnm" | b"mat4" => 16,
        b"bbox" => 32,
        _ => 4,
    }
}

fn default_bpd(ty: &Tag) -> u32 {
    match ty {
        b"stri" => 0,
        b"mat4" => 64,
        b"bbox" => 32,
        b"ui64" | b"si64" => 8,
        b"ui16" => 2,
        b"ui08" | b"bool" => 1,
        t if align_of(t) >= 16 => 16,
        _ => 4,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Raw(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct Val {
    pub ty: Tag,
    pub value: Value,
    slot: Option<(u32, u32)>,
}

impl Val {
    pub fn str(s: impl Into<String>) -> Self {
        Self {
            ty: STRI,
            value: Value::Str(s.into()),
            slot: None,
        }
    }

    pub fn raw(ty: Tag, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            ty,
            value: Value::Raw(bytes.into()),
            slot: None,
        }
    }

    pub fn u32(v: u32) -> Self {
        Self::raw(*b"ui32", v.to_le_bytes())
    }

    pub fn bool(v: bool) -> Self {
        Self::raw(*b"bool", [v as u8])
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            Value::Str(s) => Some(s),
            Value::Raw(_) => None,
        }
    }

    pub fn as_u32(&self) -> Option<u32> {
        match &self.value {
            Value::Raw(b) if b.len() >= 4 => Some(u32::from_le_bytes(b[..4].try_into().ok()?)),
            _ => None,
        }
    }

    pub fn set_str(&mut self, s: impl Into<String>) {
        self.value = Value::Str(s.into());
    }

    pub fn set_u32(&mut self, v: u32) {
        self.value = Value::Raw(v.to_le_bytes().to_vec());
    }
}

#[derive(Debug, Clone)]
pub enum Node {
    Val(Val),
    Obj(Obj),
    Arr(Arr),
    Iar([u8; 8]),
}

impl Node {
    pub fn as_obj(&self) -> Option<&Obj> {
        if let Node::Obj(o) = self { Some(o) } else { None }
    }

    pub fn as_obj_mut(&mut self) -> Option<&mut Obj> {
        if let Node::Obj(o) = self { Some(o) } else { None }
    }

    pub fn as_arr(&self) -> Option<&Arr> {
        if let Node::Arr(a) = self { Some(a) } else { None }
    }

    pub fn as_arr_mut(&mut self) -> Option<&mut Arr> {
        if let Node::Arr(a) = self { Some(a) } else { None }
    }

    pub fn as_val(&self) -> Option<&Val> {
        if let Node::Val(v) = self { Some(v) } else { None }
    }

    pub fn as_val_mut(&mut self) -> Option<&mut Val> {
        if let Node::Val(v) = self { Some(v) } else { None }
    }
}

#[derive(Debug, Clone)]
pub struct Obj {
    pub items: Vec<(Val, Node)>,
    pub version: u8,
}

impl Obj {
    pub fn new(items: Vec<(Val, Node)>) -> Self {
        Self { items, version: 1 }
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.items.iter().filter_map(|(k, _)| k.as_str())
    }

    pub fn get(&self, key: &str) -> Option<&Node> {
        self.items
            .iter()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut Node> {
        self.items
            .iter_mut()
            .find(|(k, _)| k.as_str() == Some(key))
            .map(|(_, v)| v)
    }

    pub fn set(&mut self, key: &str, node: Node) {
        match self.get_mut(key) {
            Some(slot) => *slot = node,
            None => self.items.push((Val::str(key), node)),
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<Node> {
        let i = self.items.iter().position(|(k, _)| k.as_str() == Some(key))?;
        Some(self.items.remove(i).1)
    }
}

#[derive(Debug, Clone)]
pub struct Arr {
    pub items: Vec<Node>,
    pub tag: Tag,
}

impl Arr {
    pub fn new(items: Vec<Node>) -> Self {
        Self { items, tag: *b"!ili" }
    }
}

#[derive(Debug, Clone)]
pub struct Table {
    pub ty: Tag,
    pub bpd: u32,
    pub raw: Vec<u8>,
    index: Option<HashMap<Value, u32>>,
}

impl std::hash::Hash for Value {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Value::Str(s) => s.hash(state),
            Value::Raw(b) => b.hash(state),
        }
    }
}

impl Eq for Value {}

impl Table {
    fn new(ty: Tag, bpd: u32, raw: Vec<u8>) -> Self {
        Self {
            ty,
            bpd,
            raw,
            index: None,
        }
    }

    pub fn count(&self) -> usize {
        if self.bpd == 0 {
            0
        } else {
            self.raw.len() / self.bpd as usize
        }
    }

    pub fn get(&self, idx: u32) -> Result<Value> {
        let idx = idx as usize;
        if self.ty == STRI {
            let n = *self.raw.get(idx).ok_or(Error::Eof(idx))? as usize;
            let s = self.raw.get(idx + 1..idx + 1 + n).ok_or(Error::Eof(idx))?;
            return Ok(Value::Str(String::from_utf8_lossy(s).into_owned()));
        }
        let bpd = self.bpd as usize;
        let b = self
            .raw
            .get(idx * bpd..(idx + 1) * bpd)
            .ok_or(Error::Eof(idx * bpd))?;
        Ok(Value::Raw(b.to_vec()))
    }

    fn build_index(&self) -> HashMap<Value, u32> {
        let mut index = HashMap::new();
        if self.ty == STRI {
            let mut o = 0;
            while o < self.raw.len() {
                let n = self.raw[o] as usize;
                let end = (o + 1 + n).min(self.raw.len());
                let s = String::from_utf8_lossy(&self.raw[o + 1..end]).into_owned();
                index.entry(Value::Str(s)).or_insert(o as u32);
                o += n + 2;
            }
        } else {
            for i in 0..self.count() {
                let bpd = self.bpd as usize;
                index
                    .entry(Value::Raw(self.raw[i * bpd..(i + 1) * bpd].to_vec()))
                    .or_insert(i as u32);
            }
        }
        index
    }

    fn intern(&mut self, value: &Value) -> Result<u32> {
        if self.index.is_none() {
            self.index = Some(self.build_index());
        }
        if let Some(&o) = self.index.as_ref().and_then(|i| i.get(value)) {
            return Ok(o);
        }
        let o = match value {
            Value::Str(s) => {
                let b = s.as_bytes();
                if b.len() > 255 {
                    return Err(Error::Format(format!("string longer than 255 bytes: {s}")));
                }
                let o = self.raw.len() as u32;
                self.raw.push(b.len() as u8);
                self.raw.extend_from_slice(b);
                self.raw.push(0);
                o
            }
            Value::Raw(b) => {
                let mut b = b.clone();
                b.resize(self.bpd as usize, 0);
                let i = self.count() as u32;
                self.raw.extend_from_slice(&b);
                i
            }
        };
        self.index.as_mut().expect("built above").insert(value.clone(), o);
        Ok(o)
    }
}

#[derive(Debug, Clone)]
pub struct Pkg {
    pub name: Tag,
    pub root: Node,
    pub vca: Vec<Table>,
}

pub fn is_pkg(data: &[u8]) -> bool {
    data.starts_with(b"!pkg")
}

impl Pkg {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if !is_pkg(data) {
            return Err(Error::Magic {
                offset: 0,
                expected: "!pkg",
            });
        }
        let name = data[4..8].try_into().expect("4 bytes");
        let root_ref = u32_at(data, 8)?;
        let vca_off = (u32_at(data, 12)? & OFFSET_MASK) as usize;
        if data.get(vca_off..vca_off + 4) != Some(b"!vca") {
            return Err(Error::Magic {
                offset: vca_off,
                expected: "!vca",
            });
        }
        let n = u32_at(data, vca_off + 4)? as usize;
        let mut vca = Vec::with_capacity(n);
        for i in 0..n {
            let o = (u32_at(data, vca_off + 8 + 4 * i)? & OFFSET_MASK) as usize;
            let mut r = Reader::new(data, o);
            match &r.array::<4>()? {
                b"!sbi" => {
                    let len = r.u32()? as usize;
                    vca.push(Table::new(STRI, 0, r.take(len)?.to_vec()));
                }
                b"!vbi" => {
                    let ty = r.array()?;
                    let count = r.u32()? as usize;
                    let bpd = r.u32()?;
                    vca.push(Table::new(ty, bpd, r.take(count * bpd as usize)?.to_vec()));
                }
                _ => {
                    return Err(Error::Magic {
                        offset: o,
                        expected: "!sbi or !vbi",
                    });
                }
            }
        }
        let parser = Parser { data, vca: &vca };
        let root = parser.chunk((root_ref & OFFSET_MASK) as usize)?;
        Ok(Self { name, root, vca })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut w = Writer {
            out: Vec::new(),
            vca: self.vca.clone(),
        };
        w.out.extend_from_slice(b"!pkg");
        w.out.extend_from_slice(&self.name);
        w.out.extend_from_slice(&[0; 8]);
        let mut queue = VecDeque::new();
        let root_off = w.emit(&self.root, &mut queue)?;
        while let Some((slot, node)) = queue.pop_front() {
            let off = w.emit(node, &mut queue)?;
            put_u32(&mut w.out, slot, 0x8000_0000 | off as u32);
        }
        w.pad(4);
        let vca_off = w.out.len();
        w.out.extend_from_slice(b"!vca");
        w.out.extend_from_slice(&(w.vca.len() as u32).to_le_bytes());
        let table = w.out.len();
        w.out.resize(table + 4 * w.vca.len(), 0);
        for i in 0..w.vca.len() {
            let ty = w.vca[i].ty;
            w.pad(align_of(&ty));
            let pos = w.out.len() as u32;
            put_u32(&mut w.out, table + 4 * i, 0x8000_0000 | pos);
            let t = &w.vca[i];
            let mut block = Vec::with_capacity(t.raw.len() + 16);
            if t.ty == STRI {
                block.extend_from_slice(b"!sbi");
                block.extend_from_slice(&(t.raw.len() as u32).to_le_bytes());
            } else {
                block.extend_from_slice(b"!vbi");
                block.extend_from_slice(&t.ty);
                block.extend_from_slice(&(t.count() as u32).to_le_bytes());
                block.extend_from_slice(&t.bpd.to_le_bytes());
            }
            block.extend_from_slice(&t.raw);
            w.out.extend_from_slice(&block);
        }
        w.pad(4);
        put_u32(&mut w.out, 8, 0x8000_0000 | root_off as u32);
        put_u32(&mut w.out, 12, 0x8000_0000 | vca_off as u32);
        Ok(w.out)
    }
}

struct Parser<'a> {
    data: &'a [u8],
    vca: &'a [Table],
}

impl Parser<'_> {
    fn val(&self, r: u32) -> Result<Node> {
        let (t, off) = (r >> 24, r & OFFSET_MASK);
        if t == CHUNK {
            return self.chunk(off as usize);
        }
        let table = self
            .vca
            .get(t as usize)
            .ok_or_else(|| Error::Format(format!("no value table {t}")))?;
        Ok(Node::Val(Val {
            ty: table.ty,
            value: table.get(off)?,
            slot: Some((t, off)),
        }))
    }

    fn chunk(&self, o: usize) -> Result<Node> {
        let mut r = Reader::new(self.data, o);
        let tag: Tag = r.array()?;
        match &tag {
            b"!idi" => {
                let h = r.u32()?;
                let (count, version) = ((h & OFFSET_MASK) as usize, (h >> 24) as u8);
                let es = if version == 1 { 12 } else { 8 };
                let mut items = Vec::with_capacity(count);
                for i in 0..count {
                    let p = o + 8 + es * i;
                    let key = match self.val(u32_at(self.data, p)?)? {
                        Node::Val(v) => v,
                        _ => return Err(Error::Format(format!("non-scalar key at {p:#x}"))),
                    };
                    items.push((key, self.val(u32_at(self.data, p + 4)?)?));
                }
                Ok(Node::Obj(Obj { items, version }))
            }
            b"!ili" | b"!vca" => {
                let count = r.u32()? as usize;
                let items = (0..count)
                    .map(|i| self.val(u32_at(self.data, o + 8 + 4 * i)?))
                    .collect::<Result<_>>()?;
                Ok(Node::Arr(Arr { items, tag }))
            }
            b"!iar" => Ok(Node::Iar(r.array()?)),
            _ => Err(Error::Format(format!(
                "unknown chunk {:?} at {o:#x}",
                String::from_utf8_lossy(&tag)
            ))),
        }
    }
}

struct Writer {
    out: Vec<u8>,
    vca: Vec<Table>,
}

impl Writer {
    fn pad(&mut self, align: usize) {
        while !self.out.len().is_multiple_of(align) {
            self.out.push(0);
        }
    }

    fn slot(&mut self, v: &Val) -> Result<u32> {
        if let Some((t, off)) = v.slot
            && let Some(table) = self.vca.get(t as usize)
            && table.ty == v.ty
            && table.get(off).ok().as_ref() == Some(&v.value)
        {
            return Ok(t << 24 | off);
        }
        let t = match self.vca.iter().position(|d| d.ty == v.ty) {
            Some(t) => t,
            None => {
                self.vca.push(Table::new(v.ty, default_bpd(&v.ty), Vec::new()));
                self.vca.len() - 1
            }
        };
        let off = self.vca[t].intern(&v.value)?;
        Ok((t as u32) << 24 | off)
    }

    fn emit<'n>(&mut self, node: &'n Node, queue: &mut VecDeque<(usize, &'n Node)>) -> Result<usize> {
        let start = self.out.len();
        match node {
            Node::Obj(obj) => {
                let es = if obj.version == 1 { 12 } else { 8 };
                self.out.extend_from_slice(b"!idi");
                self.out
                    .extend_from_slice(&(obj.items.len() as u32 | (obj.version as u32) << 24).to_le_bytes());
                let table = self.out.len();
                self.out.resize(table + es * obj.items.len(), 0);
                let mut items: Vec<&(Val, Node)> = obj.items.iter().collect();
                if obj.version == 1 {
                    items.sort_by_key(|(k, _)| fnv1a32(k.as_str().unwrap_or_default()));
                }
                for (i, (k, v)) in items.into_iter().enumerate() {
                    let p = table + es * i;
                    let ks = self.slot(k)?;
                    put_u32(&mut self.out, p, ks);
                    if obj.version == 1 {
                        put_u32(&mut self.out, p + 8, fnv1a32(k.as_str().unwrap_or_default()));
                    }
                    match v {
                        Node::Val(v) => {
                            let vs = self.slot(v)?;
                            put_u32(&mut self.out, p + 4, vs);
                        }
                        other => queue.push_back((p + 4, other)),
                    }
                }
            }
            Node::Arr(arr) => {
                self.out.extend_from_slice(&arr.tag);
                self.out
                    .extend_from_slice(&(arr.items.len() as u32).to_le_bytes());
                let table = self.out.len();
                self.out.resize(table + 4 * arr.items.len(), 0);
                for (i, v) in arr.items.iter().enumerate() {
                    match v {
                        Node::Val(v) => {
                            let vs = self.slot(v)?;
                            put_u32(&mut self.out, table + 4 * i, vs);
                        }
                        other => queue.push_back((table + 4 * i, other)),
                    }
                }
            }
            Node::Iar(raw) => {
                self.out.extend_from_slice(b"!iar");
                self.out.extend_from_slice(raw);
            }
            Node::Val(_) => return Err(Error::Format("a scalar cannot be a chunk".into())),
        }
        Ok(start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Pkg {
        let props = Obj::new(vec![
            (Val::str("name"), Node::Val(Val::str("fe_home"))),
            (Val::str("id"), Node::Val(Val::u32(0xdead_beef))),
            (Val::str("visible"), Node::Val(Val::bool(true))),
        ]);
        let root = Obj::new(vec![(
            Val::str("Screens"),
            Node::Arr(Arr::new(vec![Node::Arr(Arr::new(vec![
                Node::Obj(props),
                Node::Iar([7; 8]),
            ]))])),
        )]);
        Pkg {
            name: *b"test",
            root: Node::Obj(root),
            vca: Vec::new(),
        }
    }

    #[test]
    fn write_parse_write_is_identical() {
        let bytes = sample().to_bytes().unwrap();
        let back = Pkg::parse(&bytes).unwrap();
        assert_eq!(back.to_bytes().unwrap(), bytes);
        let screens = back
            .root
            .as_obj()
            .unwrap()
            .get("Screens")
            .unwrap()
            .as_arr()
            .unwrap();
        let props = screens.items[0].as_arr().unwrap().items[0].as_obj().unwrap();
        assert_eq!(
            props.get("name").unwrap().as_val().unwrap().as_str(),
            Some("fe_home")
        );
        assert_eq!(
            props.get("id").unwrap().as_val().unwrap().as_u32(),
            Some(0xdead_beef)
        );
    }

    #[test]
    fn edits_intern_new_values() {
        let bytes = sample().to_bytes().unwrap();
        let mut pkg = Pkg::parse(&bytes).unwrap();
        let screens = pkg
            .root
            .as_obj_mut()
            .unwrap()
            .get_mut("Screens")
            .unwrap()
            .as_arr_mut()
            .unwrap();
        let props = screens.items[0].as_arr_mut().unwrap().items[0]
            .as_obj_mut()
            .unwrap();
        props
            .get_mut("name")
            .unwrap()
            .as_val_mut()
            .unwrap()
            .set_str("mod_menu");
        props.set("tooltipsName", Node::Val(Val::str("mod_menu")));
        let out = pkg.to_bytes().unwrap();
        let back = Pkg::parse(&out).unwrap();
        let screens = back
            .root
            .as_obj()
            .unwrap()
            .get("Screens")
            .unwrap()
            .as_arr()
            .unwrap();
        let props = screens.items[0].as_arr().unwrap().items[0].as_obj().unwrap();
        assert_eq!(
            props.get("name").unwrap().as_val().unwrap().as_str(),
            Some("mod_menu")
        );
        assert_eq!(props.keys().count(), 4);
        assert_eq!(back.to_bytes().unwrap(), out);
    }

    #[test]
    fn version_one_keys_are_hash_sorted() {
        let bytes = sample().to_bytes().unwrap();
        let back = Pkg::parse(&bytes).unwrap();
        let screens = back
            .root
            .as_obj()
            .unwrap()
            .get("Screens")
            .unwrap()
            .as_arr()
            .unwrap();
        let props = screens.items[0].as_arr().unwrap().items[0].as_obj().unwrap();
        let hashes: Vec<u32> = props.keys().map(fnv1a32).collect();
        assert!(hashes.windows(2).all(|w| w[0] <= w[1]));
    }
}
