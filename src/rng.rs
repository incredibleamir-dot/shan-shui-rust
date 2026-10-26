use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

const P: i128 = 999_979;
const Q: i128 = 999_983;
const M: i128 = P * Q;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// A seed is either an integer or a string, mirroring the Python API.
#[derive(Clone, Debug, PartialEq)]
pub enum Seed {
    Int(i64),
    Text(String),
}

impl Seed {
    pub fn from_cli(value: &str) -> Self {
        let trimmed = value.trim();
        if !trimmed.is_empty()
            && trimmed
                .strip_prefix('-')
                .unwrap_or(trimmed)
                .bytes()
                .all(|b| b.is_ascii_digit())
        {
            if let Ok(v) = trimmed.parse::<i64>() {
                return Seed::Int(v);
            }
        }
        Seed::Text(value.to_string())
    }

    pub fn raw(&self) -> String {
        match self {
            Seed::Int(v) => v.to_string(),
            Seed::Text(v) => v.clone(),
        }
    }

    fn json_literal(&self) -> String {
        match self {
            Seed::Int(v) => v.to_string(),
            Seed::Text(v) => json_quote(v),
        }
    }
}

impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw())
    }
}

fn json_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) < 0x7f => out.push(c),
            c => {
                let cp = c as u32;
                if cp <= 0xffff {
                    out.push_str(&format!("\\u{:04x}", cp));
                } else {
                    let v = cp - 0x1_0000;
                    out.push_str(&format!(
                        "\\u{:04x}\\u{:04x}",
                        0xd800 + (v >> 10),
                        0xdc00 + (v & 0x3ff)
                    ));
                }
            }
        }
    }
    out.push('"');
    out
}

fn b64_encode(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64[((n >> 18) & 63) as usize]);
        out.push(B64[((n >> 12) & 63) as usize]);
        out.push(if chunk.len() > 1 {
            B64[((n >> 6) & 63) as usize]
        } else {
            b'='
        });
        out.push(if chunk.len() > 2 {
            B64[(n & 63) as usize]
        } else {
            b'='
        });
    }
    out
}

fn seed_hash(seed: &Seed) -> i128 {
    let json = seed.json_literal();
    let encoded = b64_encode(json.as_bytes());
    // Python: z += code * 128**i, i.e. little-endian base-128 digits.
    let mut z: i128 = 0;
    let mut place: i128 = 1;
    for byte in encoded {
        z = (z + byte as i128 * place) % M;
        place = (place * 128) % M;
    }
    z
}

/// Deterministic Blum-Blum-Shub generator used by the whole painting.
#[derive(Clone, Debug)]
pub struct Rng {
    pub seed_value: Seed,
    s: i128,
}

impl Rng {
    pub fn new(seed: Option<Seed>) -> Self {
        let seed = seed.unwrap_or_else(|| {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            Seed::Text(nanos.to_string())
        });
        let mut rng = Self {
            seed_value: seed,
            s: 0,
        };
        rng.seed_internal();
        rng
    }

    pub fn from_i64(value: i64) -> Self {
        Self::new(Some(Seed::Int(value)))
    }

    pub fn from_str(value: &str) -> Self {
        Self::new(Some(Seed::Text(value.to_string())))
    }

    fn seed_internal(&mut self) {
        let hash = seed_hash(&self.seed_value);
        let mut z: i128 = 0;
        let y = loop {
            let y = (hash + z) % M;
            z += 1;
            if y % P != 0 && y % Q != 0 && y != 0 && y != 1 {
                break y;
            }
        };
        self.s = y;
        for _ in 0..10 {
            self.random();
        }
    }

    pub fn random(&mut self) -> f64 {
        self.s = (self.s * self.s) % M;
        self.s as f64 / M as f64
    }

    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }

    pub fn randint(&mut self, a: i32, b: i32) -> i32 {
        a + ((b - a + 1) as f64 * self.random()) as i32
    }

    pub fn choice<'a, T>(&mut self, seq: &'a [T]) -> &'a T {
        assert!(!seq.is_empty(), "cannot choose from an empty sequence");
        &seq[(seq.len() as f64 * self.random()) as usize]
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.random() < p
    }

    pub fn gauss(&mut self) -> f64 {
        let u1 = self.random().max(1e-12);
        let u2 = self.random();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// Child generators use the Python-compatible label string, e.g.
    /// `"mountain|('chunk', 0)"`.
    pub fn child(&self, label: &str) -> Rng {
        Rng::new(Some(Seed::Text(format!(
            "{}|{}",
            self.seed_value.raw(),
            label
        ))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_python() {
        assert_eq!(b64_encode(b""), b"");
        assert_eq!(b64_encode(b"f"), b"Zg==");
        assert_eq!(b64_encode(b"fo"), b"Zm8=");
        assert_eq!(b64_encode(b"foo"), b"Zm9v");
        assert_eq!(b64_encode(b"foob"), b"Zm9vYg==");
    }

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::from_str("mountain");
        let mut b = Rng::from_str("mountain");
        for _ in 0..12 {
            assert_eq!(a.random(), b.random());
        }
    }

    #[test]
    fn child_matches_child() {
        let mut a = Rng::from_str("mountain").child("('chunk', 3)");
        let mut b = Rng::from_str("mountain").child("('chunk', 3)");
        for _ in 0..8 {
            assert_eq!(a.random(), b.random());
        }
    }

    #[test]
    fn values_are_normalized() {
        let mut rng = Rng::from_i64(42);
        for _ in 0..100 {
            let value = rng.random();
            assert!((0.0..1.0).contains(&value));
        }
    }

    #[test]
    fn int_seed_round_trips() {
        assert_eq!(Rng::from_i64(123).seed_value, Seed::Int(123));
    }
}
