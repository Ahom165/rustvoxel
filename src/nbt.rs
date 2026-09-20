//! NBT (Named Binary Tag) — lecteur + écrivain, zéro dépendance.
//! Utilisé pour : heightmaps + registres (configuration), composants de chat système,
//! block entities. Le "NBT réseau" a une racine compound SANS nom (un seul octet 0x0A).

#[derive(Debug, Clone, PartialEq)]
pub enum Nbt {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    Str(String),
    List(Vec<Nbt>),
    Compound(Vec<(String, Nbt)>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

pub const TAG_END: u8 = 0;
pub const TAG_BYTE: u8 = 1;
pub const TAG_SHORT: u8 = 2;
pub const TAG_INT: u8 = 3;
pub const TAG_LONG: u8 = 4;
pub const TAG_FLOAT: u8 = 5;
pub const TAG_DOUBLE: u8 = 6;
pub const TAG_BYTE_ARRAY: u8 = 7;
pub const TAG_STRING: u8 = 8;
pub const TAG_LIST: u8 = 9;
pub const TAG_COMPOUND: u8 = 10;
pub const TAG_INT_ARRAY: u8 = 11;
pub const TAG_LONG_ARRAY: u8 = 12;

impl Nbt {
    pub fn compound() -> Nbt {
        Nbt::Compound(Vec::new())
    }
    pub fn set(&mut self, key: &str, v: Nbt) {
        if let Nbt::Compound(m) = self {
            if let Some(e) = m.iter_mut().find(|(k, _)| k == key) {
                e.1 = v;
            } else {
                m.push((key.to_string(), v));
            }
        }
    }
    pub fn get(&self, key: &str) -> Option<&Nbt> {
        match self {
            Nbt::Compound(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Nbt::Byte(v) => Some(*v as i64),
            Nbt::Short(v) => Some(*v as i64),
            Nbt::Int(v) => Some(*v as i64),
            Nbt::Long(v) => Some(*v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Nbt::Str(s) => Some(s),
            _ => None,
        }
    }

    fn tag_id(&self) -> u8 {
        match self {
            Nbt::Byte(_) => TAG_BYTE,
            Nbt::Short(_) => TAG_SHORT,
            Nbt::Int(_) => TAG_INT,
            Nbt::Long(_) => TAG_LONG,
            Nbt::Float(_) => TAG_FLOAT,
            Nbt::Double(_) => TAG_DOUBLE,
            Nbt::ByteArray(_) => TAG_BYTE_ARRAY,
            Nbt::Str(_) => TAG_STRING,
            Nbt::List(_) => TAG_LIST,
            Nbt::Compound(_) => TAG_COMPOUND,
            Nbt::IntArray(_) => TAG_INT_ARRAY,
            Nbt::LongArray(_) => TAG_LONG_ARRAY,
        }
    }

    /// Écrit le tag avec son nom (pour les entrées de compound).
    fn write_named(&self, out: &mut Vec<u8>, name: &str) {
        out.push(self.tag_id());
        write_string(out, name);
        self.write_payload(out);
    }

    fn write_payload(&self, out: &mut Vec<u8>) {
        match self {
            Nbt::Byte(v) => out.extend_from_slice(&[*v as u8]),
            Nbt::Short(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Int(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Long(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Float(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::Double(v) => out.extend_from_slice(&v.to_be_bytes()),
            Nbt::ByteArray(b) => {
                out.extend_from_slice(&(b.len() as i32).to_be_bytes());
                out.extend_from_slice(b);
            }
            Nbt::Str(s) => write_string(out, s),
            Nbt::List(items) => {
                let inner = items.first().map(|i| i.tag_id()).unwrap_or(TAG_END);
                out.push(inner);
                out.extend_from_slice(&(items.len() as i32).to_be_bytes());
                for i in items {
                    i.write_payload(out);
                }
            }
            Nbt::Compound(m) => {
                for (k, v) in m {
                    v.write_named(out, k);
                }
                out.push(TAG_END);
            }
            Nbt::IntArray(a) => {
                out.extend_from_slice(&(a.len() as i32).to_be_bytes());
                for v in a {
                    out.extend_from_slice(&v.to_be_bytes());
                }
            }
            Nbt::LongArray(a) => {
                out.extend_from_slice(&(a.len() as i32).to_be_bytes());
                for v in a {
                    out.extend_from_slice(&v.to_be_bytes());
                }
            }
        }
    }

    /// NBT réseau : racine compound SANS nom (0x0A + payload).
    pub fn write_network(&self, out: &mut Vec<u8>) {
        out.push(TAG_COMPOUND);
        self.write_payload(out);
    }
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    let b = s.as_bytes();
    out.extend_from_slice(&(b.len() as u16).to_be_bytes());
    out.extend_from_slice(b);
}

// ---------- lecteur ----------

pub struct NbtRdr<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> NbtRdr<'a> {
    pub fn new(b: &'a [u8]) -> NbtRdr<'a> {
        NbtRdr { b, p: 0 }
    }
    fn u8(&mut self) -> Option<u8> {
        let v = *self.b.get(self.p)?;
        self.p += 1;
        Some(v)
    }
    fn u16(&mut self) -> Option<u16> {
        let v = self.b.get(self.p..self.p + 2)?;
        self.p += 2;
        Some(u16::from_be_bytes(v.try_into().ok()?))
    }
    fn i32v(&mut self) -> Option<i32> {
        let v = self.b.get(self.p..self.p + 4)?;
        self.p += 4;
        Some(i32::from_be_bytes(v.try_into().ok()?))
    }
    fn i64v(&mut self) -> Option<i64> {
        let v = self.b.get(self.p..self.p + 8)?;
        self.p += 8;
        Some(i64::from_be_bytes(v.try_into().ok()?))
    }
    fn f32v(&mut self) -> Option<f32> {
        let v = self.b.get(self.p..self.p + 4)?;
        self.p += 4;
        Some(f32::from_be_bytes(v.try_into().ok()?))
    }
    fn f64v(&mut self) -> Option<f64> {
        let v = self.b.get(self.p..self.p + 8)?;
        self.p += 8;
        Some(f64::from_be_bytes(v.try_into().ok()?))
    }
    fn string(&mut self) -> Option<String> {
        let n = self.u16()? as usize;
        if self.p + n > self.b.len() {
            return None;
        }
        let s = String::from_utf8_lossy(&self.b[self.p..self.p + n]).to_string();
        self.p += n;
        Some(s)
    }
    fn payload(&mut self, tag: u8) -> Option<Nbt> {
        Some(match tag {
            TAG_BYTE => Nbt::Byte(self.u8()? as i8),
            TAG_SHORT => Nbt::Short(self.u16()? as i16),
            TAG_INT => Nbt::Int(self.i32v()?),
            TAG_LONG => Nbt::Long(self.i64v()?),
            TAG_FLOAT => Nbt::Float(self.f32v()?),
            TAG_DOUBLE => Nbt::Double(self.f64v()?),
            TAG_BYTE_ARRAY => {
                let n = self.i32v()?;
                if n < 0 || self.p + n as usize > self.b.len() {
                    return None;
                }
                let v = self.b[self.p..self.p + n as usize].to_vec();
                self.p += n as usize;
                Nbt::ByteArray(v)
            }
            TAG_STRING => Nbt::Str(self.string()?),
            TAG_LIST => {
                let inner = self.u8()?;
                let n = self.i32v()?;
                if n < 0 {
                    return None;
                }
                let mut items = Vec::new();
                for _ in 0..n {
                    if inner == TAG_END {
                        break;
                    }
                    items.push(self.payload(inner)?);
                }
                Nbt::List(items)
            }
            TAG_COMPOUND => self.compound_body()?,
            TAG_INT_ARRAY => {
                let n = self.i32v()?;
                if n < 0 || self.p + 4 * n as usize > self.b.len() {
                    return None;
                }
                let mut v = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    v.push(self.i32v()?);
                }
                Nbt::IntArray(v)
            }
            TAG_LONG_ARRAY => {
                let n = self.i32v()?;
                if n < 0 || self.p + 8 * n as usize > self.b.len() {
                    return None;
                }
                let mut v = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    v.push(self.i64v()?);
                }
                Nbt::LongArray(v)
            }
            _ => return None,
        })
    }
    fn compound_body(&mut self) -> Option<Nbt> {
        let mut m = Vec::new();
        loop {
            let tag = self.u8()?;
            if tag == TAG_END {
                break;
            }
            let name = self.string()?;
            let v = self.payload(tag)?;
            m.push((name, v));
        }
        Some(Nbt::Compound(m))
    }

    /// Racine compound SANS nom (NBT réseau).
    pub fn read_network(&mut self) -> Option<Nbt> {
        let tag = self.u8()?;
        if tag != TAG_COMPOUND {
            return None;
        }
        self.compound_body()
    }
    pub fn pos(&self) -> usize {
        self.p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_network() {
        let mut root = Nbt::compound();
        root.set("int", Nbt::Int(-42));
        root.set("s", Nbt::Str("hé".to_string()));
        root.set(
            "list",
            Nbt::List(vec![Nbt::Long(1), Nbt::Long(2), Nbt::Long(i64::MAX)]),
        );
        let mut inner = Nbt::compound();
        inner.set("f", Nbt::Float(0.5));
        root.set("inner", inner);
        root.set(
            "la",
            Nbt::LongArray(vec![1, -2, 0x7FFF_FFFF_FFFF]),
        );
        root.set("ba", Nbt::ByteArray(vec![1, 2, 255]));

        let mut buf = Vec::new();
        root.write_network(&mut buf);
        let mut r = NbtRdr::new(&buf);
        let back = r.read_network().unwrap();
        assert_eq!(r.pos(), buf.len());
        assert_eq!(back.get("int").unwrap().as_i64(), Some(-42));
        assert_eq!(back.get("s").unwrap().as_str(), Some("hé"));
        assert_eq!(
            back.get("list").unwrap(),
            &Nbt::List(vec![Nbt::Long(1), Nbt::Long(2), Nbt::Long(i64::MAX)])
        );
        assert_eq!(
            back.get("inner").unwrap().get("f").unwrap(),
            &Nbt::Float(0.5)
        );
        assert_eq!(
            back.get("la").unwrap(),
            &Nbt::LongArray(vec![1, -2, 0x7FFF_FFFF_FFFF])
        );
        assert_eq!(back.get("ba").unwrap(), &Nbt::ByteArray(vec![1, 2, 255]));
    }

    #[test]
    fn empty_list_and_compound() {
        let mut root = Nbt::compound();
        root.set("empty", Nbt::List(vec![]));
        root.set("ec", Nbt::compound());
        let mut buf = Vec::new();
        root.write_network(&mut buf);
        let back = NbtRdr::new(&buf).read_network().unwrap();
        assert_eq!(back.get("empty").unwrap(), &Nbt::List(vec![]));
    }
}
