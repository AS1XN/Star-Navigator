//! Binary catalog format.
//!
//! Little-endian. Header: magic `SNCAT`, version u8, star count u32. Each record:
//!
//! ```text
//! hyg u32, hip u32, hd u32, hr u32         (0 = none)
//! ra f32 (hours), dec f32 (deg)
//! dist f32 (pc, NaN = unknown)
//! mag f32, absmag f32, ci f32 (NaN = none)
//! con u8 (255 = none), flamsteed u8 (0 = none)
//! proper, bayer, gliese, spectral: u8 length + UTF-8
//! ```

use crate::Star;

const MAGIC: &[u8; 5] = b"SNCAT";
const VERSION: u8 = 1;

#[derive(Debug)]
pub enum DecodeError {
    BadMagic,
    Version(u8),
    Truncated,
    Utf8,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BadMagic => write!(f, "not a star catalog file"),
            Self::Version(v) => write!(f, "unsupported catalog version {v}"),
            Self::Truncated => write!(f, "catalog file is truncated"),
            Self::Utf8 => write!(f, "invalid UTF-8 in catalog strings"),
        }
    }
}

impl std::error::Error for DecodeError {}

pub fn encode(stars: &[Star]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + stars.len() * 64);
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&(stars.len() as u32).to_le_bytes());

    let opt_u32 = |v: Option<u32>| v.unwrap_or(0).to_le_bytes();
    let opt_f32 = |v: Option<f32>| v.unwrap_or(f32::NAN).to_le_bytes();
    for s in stars {
        out.extend_from_slice(&s.hyg.to_le_bytes());
        out.extend_from_slice(&opt_u32(s.hip));
        out.extend_from_slice(&opt_u32(s.hd));
        out.extend_from_slice(&opt_u32(s.hr));
        out.extend_from_slice(&s.ra.to_le_bytes());
        out.extend_from_slice(&s.dec.to_le_bytes());
        out.extend_from_slice(&opt_f32(s.dist_pc));
        out.extend_from_slice(&s.mag.to_le_bytes());
        out.extend_from_slice(&s.absmag.to_le_bytes());
        out.extend_from_slice(&opt_f32(s.ci));
        out.push(s.con.unwrap_or(255));
        out.push(s.flamsteed.unwrap_or(0));
        for text in [&s.proper, &s.bayer, &s.gliese] {
            put_str(&mut out, text.as_deref().unwrap_or(""));
        }
        put_str(&mut out, &s.spectral);
    }
    out
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    let bytes = &s.as_bytes()[..s.len().min(255)];
    out.push(bytes.len() as u8);
    out.extend_from_slice(bytes);
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.buf.len());
        let end = end.ok_or(DecodeError::Truncated)?;
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn f32(&mut self) -> Result<f32, DecodeError> {
        Ok(f32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn opt_u32(&mut self) -> Result<Option<u32>, DecodeError> {
        self.u32().map(|v| (v != 0).then_some(v))
    }

    fn opt_f32(&mut self) -> Result<Option<f32>, DecodeError> {
        self.f32().map(|v| (!v.is_nan()).then_some(v))
    }

    fn string(&mut self) -> Result<String, DecodeError> {
        let len = self.u8()? as usize;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes).map(str::to_owned).map_err(|_| DecodeError::Utf8)
    }

    fn opt_string(&mut self) -> Result<Option<String>, DecodeError> {
        self.string().map(|s| (!s.is_empty()).then_some(s))
    }
}

pub fn decode(buf: &[u8]) -> Result<Vec<Star>, DecodeError> {
    let mut r = Reader { buf, pos: 0 };
    if r.take(5).map_err(|_| DecodeError::BadMagic)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let version = r.u8()?;
    if version != VERSION {
        return Err(DecodeError::Version(version));
    }
    let count = r.u32()? as usize;
    let mut stars = Vec::with_capacity(count.min(buf.len() / 50));
    for _ in 0..count {
        stars.push(Star {
            hyg: r.u32()?,
            hip: r.opt_u32()?,
            hd: r.opt_u32()?,
            hr: r.opt_u32()?,
            ra: r.f32()?,
            dec: r.f32()?,
            dist_pc: r.opt_f32()?,
            mag: r.f32()?,
            absmag: r.f32()?,
            ci: r.opt_f32()?,
            con: Some(r.u8()?).filter(|&c| c != 255),
            flamsteed: Some(r.u8()?).filter(|&f| f != 0),
            proper: r.opt_string()?,
            bayer: r.opt_string()?,
            gliese: r.opt_string()?,
            spectral: r.string()?,
        });
    }
    Ok(stars)
}
