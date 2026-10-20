// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Typed lengths: parsing what the user types into the measurement box, and writing lengths
//! back as text.
//!
//! Behaviour: `docs/specs/m2-units.md`.

/// The unit a bare number is read in, and the unit lengths are written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Millimeter,
    Centimeter,
    Meter,
    Inch,
    /// Written as feet and inches (`5' 4"`); a bare number is read as feet.
    Foot,
}

/// Why a typed length was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Nothing but white space.
    Empty,
    /// Anything that is not a length.
    Invalid,
}

/// Meters in one `unit`.
pub fn meters_per(unit: Unit) -> f64 {
    match unit {
        Unit::Millimeter => 0.001,
        Unit::Centimeter => 0.01,
        Unit::Meter => 1.0,
        Unit::Inch => 0.0254,
        Unit::Foot => 0.3048,
    }
}

/// A cursor over the trimmed input bytes.
struct Cur<'a> {
    b: &'a [u8],
    i: usize,
}

impl Cur<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && self.b[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.b.get(self.i).copied()
    }
    fn digits(&mut self) -> &str {
        let s = self.i;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.i += 1;
        }
        std::str::from_utf8(&self.b[s..self.i]).unwrap_or("")
    }
    fn qty(&mut self) -> Option<f64> {
        let s = self.i;
        let int = self.digits().len();
        let mut frac = 0;
        if self.peek() == Some(b'.') {
            self.i += 1;
            frac = self.digits().len();
            if frac == 0 {
                return None;
            }
        }
        if int + frac == 0 {
            return None;
        }
        let a: f64 = std::str::from_utf8(&self.b[s..self.i]).ok()?.parse().ok()?;
        let is_int = self.b[s..self.i].iter().all(u8::is_ascii_digit);
        if self.peek() == Some(b'/') {
            if !is_int {
                return None;
            }
            return Some(a / self.denominator()?);
        }
        let save = self.i;
        self.ws();
        if self.peek().is_some_and(|c| c.is_ascii_digit()) {
            if !is_int {
                return None;
            }
            let n: f64 = self.digits().parse().ok()?;
            if self.peek() != Some(b'/') {
                return None;
            }
            return Some(a + n / self.denominator()?);
        }
        self.i = save;
        Some(a)
    }
    fn denominator(&mut self) -> Option<f64> {
        self.i += 1;
        let d: f64 = self.digits().parse().ok()?;
        (d != 0.0).then_some(d)
    }
    fn suffix(&mut self) -> Option<String> {
        self.ws();
        match self.peek() {
            Some(c @ (b'\'' | b'"')) => {
                self.i += 1;
                Some((c as char).to_string())
            }
            Some(c) if c.is_ascii_alphabetic() => {
                let s = self.i;
                while self.peek().is_some_and(|c| c.is_ascii_alphabetic()) {
                    self.i += 1;
                }
                Some(String::from_utf8_lossy(&self.b[s..self.i]).to_ascii_lowercase())
            }
            _ => None,
        }
    }
}

/// A typed length in meters. A bare number is read in `default`.
pub fn parse_length(input: &str, default: Unit) -> Result<f64, ParseError> {
    let t = input.trim_matches(|c: char| c.is_ascii_whitespace());
    if t.is_empty() {
        return Err(ParseError::Empty);
    }
    let mut c = Cur {
        b: t.as_bytes(),
        i: 0,
    };
    let mut sign = 1.0;
    if let Some(s @ (b'-' | b'+')) = c.peek() {
        if s == b'-' {
            sign = -1.0;
        }
        c.i += 1;
        c.ws();
    }
    let a = c.qty().ok_or(ParseError::Invalid)?;
    let suf = c.suffix();
    let end = |c: &mut Cur| {
        c.ws();
        c.i == c.b.len()
    };
    let m = match suf.as_deref() {
        None => a * meters_per(default),
        Some("mm") => a * 0.001,
        Some("cm") => a * 0.01,
        Some("m") => a,
        Some("in" | "\"") => a * 0.0254,
        Some("ft" | "'") => {
            let mut m = a * 0.3048;
            c.ws();
            if c.i < c.b.len() {
                let b = c.qty().ok_or(ParseError::Invalid)?;
                match c.suffix().as_deref() {
                    None | Some("in" | "\"") => {}
                    _ => return Err(ParseError::Invalid),
                }
                m += b * 0.0254;
            }
            m
        }
        _ => return Err(ParseError::Invalid),
    };
    if !end(&mut c) || !m.is_finite() {
        return Err(ParseError::Invalid);
    }
    Ok(sign * m)
}

/// Comma-separated lengths (`5,3` for a rectangle), each as for [`parse_length`].
pub fn parse_lengths(input: &str, default: Unit) -> Result<Vec<f64>, ParseError> {
    input.split(',').map(|p| parse_length(p, default)).collect()
}

fn trim_num(v: f64, d: usize) -> String {
    let s = format!("{v:.d$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// A length in meters as text in `unit`, rounded to `decimals` places of the last unit
/// written (inches for `Foot` and `Inch`).
pub fn format_length(meters: f64, unit: Unit, decimals: u32) -> String {
    let d = decimals.min(6) as usize;
    if !meters.is_finite() {
        return "0".into();
    }
    let scale = 10f64.powi(d as i32);
    let a = meters.abs();
    let (neg, body) = match unit {
        Unit::Foot => {
            let n = (a / 0.0254 * scale).round() as u128;
            let per_foot = 12 * 10u128.pow(d as u32);
            let (ft, rest) = (n / per_foot, n % per_foot);
            let inch = rest as f64 / scale;
            let s = match (ft > 0, rest > 0) {
                (true, true) => format!("{ft}' {}\"", trim_num(inch, d)),
                (true, false) => format!("{ft}'"),
                (false, true) => format!("{}\"", trim_num(inch, d)),
                (false, false) => "0\"".to_string(),
            };
            (n > 0 && meters < 0.0, s)
        }
        _ => {
            let (u, suf) = match unit {
                Unit::Millimeter => (0.001, "mm"),
                Unit::Centimeter => (0.01, "cm"),
                Unit::Meter => (1.0, "m"),
                _ => (0.0254, "\""),
            };
            let n = (a / u * scale).round();
            (
                n > 0.0 && meters < 0.0,
                format!("{}{suf}", trim_num(n / scale, d)),
            )
        }
    };
    if neg { format!("-{body}") } else { body }
}
