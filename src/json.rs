//! JSON minimal (parse + écriture), zéro dépendance.
//! Suffisant pour pack.mcmeta, blockstates, models, réponse status et composants de chat.

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            Json::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Json::Bool(b) => Some(*b),
            Json::Num(n) => Some(*n != 0.0),
            _ => None,
        }
    }
    pub fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
    pub fn as_obj(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(m) => Some(m),
            _ => None,
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Json::Null)
    }

    pub fn str(s: &str) -> Json {
        Json::Str(s.to_string())
    }
    pub fn num(n: f64) -> Json {
        Json::Num(n)
    }

    /// Sérialisation compacte.
    pub fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 9.0e15 {
                    out.push_str(&format!("{}", *n as i64));
                } else {
                    out.push_str(&format!("{}", n));
                }
            }
            Json::Str(s) => write_json_string(s, out),
            Json::Arr(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Json::Obj(m) => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_json_string(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
    pub fn to_string(&self) -> String {
        let mut s = String::new();
        self.write(&mut s);
        s
    }
}

pub fn write_json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

struct P<'a> {
    b: &'a [u8],
    p: usize,
}

/// Parse un document JSON (utf-8). Renvoie None si invalide.
pub fn parse(data: &str) -> Option<Json> {
    let mut p = P { b: data.as_bytes(), p: 0 };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.p != p.b.len() {
        return None;
    }
    Some(v)
}

impl<'a> P<'a> {
    fn ws(&mut self) {
        while self.p < self.b.len() && matches!(self.b[self.p], b' ' | b'\t' | b'\n' | b'\r') {
            self.p += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.p).copied()
    }
    fn eat(&mut self, c: u8) -> Option<()> {
        if self.peek() == Some(c) {
            self.p += 1;
            Some(())
        } else {
            None
        }
    }
    fn lit(&mut self, s: &str) -> Option<()> {
        if self.b[self.p..].starts_with(s.as_bytes()) {
            self.p += s.len();
            Some(())
        } else {
            None
        }
    }
    fn value(&mut self) -> Option<Json> {
        match self.peek()? {
            b'{' => self.object(),
            b'[' => self.array(),
            b'"' => self.string().map(Json::Str),
            b't' => self.lit("true").map(|_| Json::Bool(true)),
            b'f' => self.lit("false").map(|_| Json::Bool(false)),
            b'n' => self.lit("null").map(|_| Json::Null),
            _ => self.number(),
        }
    }
    fn object(&mut self) -> Option<Json> {
        self.eat(b'{')?;
        let mut m = Vec::new();
        self.ws();
        if self.eat(b'}').is_some() {
            return Some(Json::Obj(m));
        }
        loop {
            self.ws();
            let k = self.string()?;
            self.ws();
            self.eat(b':')?;
            self.ws();
            let v = self.value()?;
            m.push((k, v));
            self.ws();
            match self.peek()? {
                b',' => {
                    self.p += 1;
                }
                b'}' => {
                    self.p += 1;
                    break;
                }
                _ => return None,
            }
        }
        Some(Json::Obj(m))
    }
    fn array(&mut self) -> Option<Json> {
        self.eat(b'[')?;
        let mut a = Vec::new();
        self.ws();
        if self.eat(b']').is_some() {
            return Some(Json::Arr(a));
        }
        loop {
            self.ws();
            a.push(self.value()?);
            self.ws();
            match self.peek()? {
                b',' => {
                    self.p += 1;
                }
                b']' => {
                    self.p += 1;
                    break;
                }
                _ => return None,
            }
        }
        Some(Json::Arr(a))
    }
    fn string(&mut self) -> Option<String> {
        self.eat(b'"')?;
        let mut s = String::new();
        loop {
            let c = self.peek()?;
            self.p += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let e = self.peek()?;
                    self.p += 1;
                    match e {
                        b'"' => s.push('"'),
                        b'\\' => s.push('\\'),
                        b'/' => s.push('/'),
                        b'n' => s.push('\n'),
                        b'r' => s.push('\r'),
                        b't' => s.push('\t'),
                        b'b' => s.push('\u{8}'),
                        b'f' => s.push('\u{c}'),
                        b'u' => {
                            let h = self.hex4()?;
                            if (0xD800..0xDC00).contains(&h) {
                                // paire surrogate
                                self.lit("\\u")?;
                                let h2 = self.hex4()?;
                                let cp = 0x10000 + ((h - 0xD800) << 10) + (h2 - 0xDC00);
                                s.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                            } else {
                                s.push(char::from_u32(h).unwrap_or('\u{FFFD}'));
                            }
                        }
                        _ => return None,
                    }
                }
                c if c < 0x80 => s.push(c as char),
                _ => {
                    // utf-8 multi-octets : décode depuis la position de départ
                    let start = self.p - 1;
                    let len = if c >= 0xF0 {
                        4
                    } else if c >= 0xE0 {
                        3
                    } else {
                        2
                    };
                    if start + len > self.b.len() {
                        return None;
                    }
                    let sub = std::str::from_utf8(&self.b[start..start + len]).ok()?;
                    s.push_str(sub);
                    self.p = start + len;
                }
            }
        }
        Some(s)
    }
    fn hex4(&mut self) -> Option<u32> {
        if self.p + 4 > self.b.len() {
            return None;
        }
        let s = std::str::from_utf8(&self.b[self.p..self.p + 4]).ok()?;
        let v = u32::from_str_radix(s, 16).ok()?;
        self.p += 4;
        Some(v)
    }
    fn number(&mut self) -> Option<Json> {
        let start = self.p;
        if self.peek() == Some(b'-') {
            self.p += 1;
        }
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || matches!(c, b'.' | b'e' | b'E' | b'+' | b'-') {
                self.p += 1;
            } else {
                break;
            }
        }
        if start == self.p {
            return None;
        }
        let s = std::str::from_utf8(&self.b[start..self.p]).ok()?;
        s.parse::<f64>().ok().map(Json::Num)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_write_roundtrip() {
        let src = r#"{"a":[1,2.5,true,false,null],"b":{"c":"hé\"x\n"},"d":-7}"#;
        let v = parse(src).unwrap();
        assert_eq!(v.get("d").unwrap().as_num(), Some(-7.0));
        assert_eq!(
            v.get("b").unwrap().get("c").unwrap().as_str(),
            Some("hé\"x\n")
        );
        let out = v.to_string();
        let v2 = parse(&out).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse("{").is_none());
        assert!(parse("[1,]").is_none());
        assert!(parse("nul").is_none());
        assert!(parse("{}x").is_none());
    }

    #[test]
    fn pack_mcmeta_shape() {
        let src = r#"{"pack":{"pack_format":34,"description":"Default"}}"#;
        let v = parse(src).unwrap();
        let f = v.get("pack").unwrap().get("pack_format").unwrap();
        assert_eq!(f.as_num(), Some(34.0));
    }
}
