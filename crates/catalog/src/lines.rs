//! Constellation stick figures.
//!
//! File format, little-endian: magic `SNLIN`, version u8, figure count u32, then per
//! polyline: constellation index u8, point count u16, points as (ra hours f32,
//! dec degrees f32).

use crate::DecodeError;

const MAGIC: &[u8; 5] = b"SNLIN";
const VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Polyline {
    /// Index into [`crate::CONSTELLATIONS`].
    pub con: u8,
    /// (right ascension in hours, declination in degrees)
    pub points: Vec<(f32, f32)>,
}

pub fn encode_lines(lines: &[Polyline]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&(lines.len() as u32).to_le_bytes());
    for line in lines {
        out.push(line.con);
        out.extend_from_slice(&(line.points.len() as u16).to_le_bytes());
        for (ra, dec) in &line.points {
            out.extend_from_slice(&ra.to_le_bytes());
            out.extend_from_slice(&dec.to_le_bytes());
        }
    }
    out
}

pub fn decode_lines(buf: &[u8]) -> Result<Vec<Polyline>, DecodeError> {
    let mut pos = 0;
    let mut take = |n: usize| -> Result<&[u8], DecodeError> {
        let slice = buf.get(pos..pos + n).ok_or(DecodeError::Truncated)?;
        pos += n;
        Ok(slice)
    };
    if take(5).map_err(|_| DecodeError::BadMagic)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let version = take(1)?[0];
    if version != VERSION {
        return Err(DecodeError::Version(version));
    }
    let f32_at = |b: &[u8]| f32::from_le_bytes(b.try_into().unwrap());
    let count = u32::from_le_bytes(take(4)?.try_into().unwrap()) as usize;
    let mut lines = Vec::with_capacity(count.min(10_000));
    for _ in 0..count {
        let con = take(1)?[0];
        let n = u16::from_le_bytes(take(2)?.try_into().unwrap()) as usize;
        let mut points = Vec::with_capacity(n);
        for _ in 0..n {
            let p = take(8)?;
            points.push((f32_at(&p[..4]), f32_at(&p[4..])));
        }
        lines.push(Polyline { con, points });
    }
    Ok(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let lines = vec![
            Polyline { con: 59, points: vec![(5.92, 7.41), (5.24, -8.2)] },
            Polyline { con: 0, points: vec![] },
        ];
        assert_eq!(decode_lines(&encode_lines(&lines)).unwrap(), lines);
        assert!(matches!(decode_lines(b"SNCAT\x01"), Err(DecodeError::BadMagic)));
    }
}
