// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! STL reader, binary and ASCII (`docs/specs/m7-import-stl.md`).

use std::fmt;

use crate::MeshObject;

/// Why bytes couldn't be read as STL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StlError {
    /// Neither a binary STL of the right length nor text starting with `solid`.
    NotStl,
    /// A binary STL whose length doesn't match its triangle count: `expected` is
    /// `84 + 50 * count`, `actual` the byte count.
    Truncated { expected: usize, actual: usize },
    /// An ASCII STL that breaks the grammar; `line` is 1-based.
    Syntax { line: usize, message: String },
}

impl fmt::Display for StlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StlError::NotStl => write!(f, "not an STL file"),
            StlError::Truncated { expected, actual } => {
                write!(f, "binary STL is {actual} bytes, expected {expected}")
            }
            StlError::Syntax { line, message } => write!(f, "line {line}: {message}"),
        }
    }
}

impl std::error::Error for StlError {}

/// Reads binary or ASCII STL. Solids without a name get `default_name`.
pub fn read(bytes: &[u8], default_name: &str) -> Result<Vec<MeshObject>, StlError> {
    let count = if bytes.len() >= 84 {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]);
        let expected = binary_len(n);
        if bytes.len() == expected {
            return Ok(read_binary(bytes, n, default_name));
        }
        Some((n, expected))
    } else {
        None
    };
    if starts_with_solid(bytes) {
        let text = std::str::from_utf8(bytes).map_err(|_| StlError::NotStl)?;
        return ascii(text, default_name);
    }
    match count {
        Some((_, expected)) => Err(StlError::Truncated {
            expected,
            actual: bytes.len(),
        }),
        None => Err(StlError::NotStl),
    }
}

fn binary_len(n: u32) -> usize {
    (n as usize)
        .checked_mul(50)
        .and_then(|body| body.checked_add(84))
        .unwrap_or(usize::MAX)
}

fn read_binary(bytes: &[u8], n: u32, default_name: &str) -> Vec<MeshObject> {
    let mut triangles = Vec::with_capacity(n as usize);
    for i in 0..n as usize {
        let record = &bytes[84 + 50 * i..84 + 50 * i + 50];
        let float = |at: usize| {
            f32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]])
        };
        let corner = |k: usize| {
            let at = 12 + 12 * k;
            [float(at), float(at + 4), float(at + 8)]
        };
        triangles.push([corner(0), corner(1), corner(2)]);
    }
    vec![MeshObject {
        name: default_name.to_string(),
        triangles,
    }]
}

fn starts_with_solid(bytes: &[u8]) -> bool {
    let rest = bytes.trim_ascii_start();
    rest.len() >= 5 && rest[..5].eq_ignore_ascii_case(b"solid")
}

struct Token<'a> {
    text: &'a str,
    line: usize,
    column: usize,
}

impl Token<'_> {
    fn is(&self, keyword: &str) -> bool {
        self.text.eq_ignore_ascii_case(keyword)
    }
}

fn tokenize(text: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    for (index, line) in text.split('\n').enumerate() {
        let mut start = None;
        for (column, ch) in line.char_indices() {
            if ch.is_whitespace() {
                if let Some(s) = start.take() {
                    tokens.push(Token {
                        text: &line[s..column],
                        line: index + 1,
                        column: s,
                    });
                }
            } else if start.is_none() {
                start = Some(column);
            }
        }
        if let Some(s) = start {
            tokens.push(Token {
                text: &line[s..],
                line: index + 1,
                column: s,
            });
        }
    }
    tokens
}

fn syntax(line: usize, message: impl Into<String>) -> StlError {
    StlError::Syntax {
        line,
        message: message.into(),
    }
}

fn ascii(text: &str, default_name: &str) -> Result<Vec<MeshObject>, StlError> {
    let lines: Vec<&str> = text.split('\n').collect();
    let tokens = tokenize(text);
    let last_line = text.lines().count().max(1);
    let mut objects = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let solid = &tokens[i];
        if !solid.is("solid") {
            return Err(syntax(
                solid.line,
                format!("expected `solid`, found `{}`", solid.text),
            ));
        }
        let name = lines[solid.line - 1][solid.column + solid.text.len()..].trim();
        i += 1;
        while i < tokens.len() && tokens[i].line == solid.line {
            i += 1;
        }
        let mut triangles = Vec::new();
        loop {
            if i >= tokens.len() {
                break;
            }
            let token = &tokens[i];
            if token.is("endsolid") {
                let line = token.line;
                i += 1;
                while i < tokens.len() && tokens[i].line == line {
                    i += 1;
                }
                break;
            }
            if !token.is("facet") {
                return Err(syntax(
                    token.line,
                    format!("expected `facet` or `endsolid`, found `{}`", token.text),
                ));
            }
            i += 1;
            expect(&tokens, &mut i, "normal", last_line)?;
            for _ in 0..3 {
                number(&tokens, &mut i, last_line)?;
            }
            expect(&tokens, &mut i, "outer", last_line)?;
            expect(&tokens, &mut i, "loop", last_line)?;
            let mut corners = [[0.0f32; 3]; 3];
            for corner in &mut corners {
                expect(&tokens, &mut i, "vertex", last_line)?;
                for value in corner.iter_mut() {
                    *value = number(&tokens, &mut i, last_line)?;
                }
            }
            expect(&tokens, &mut i, "endloop", last_line)?;
            expect(&tokens, &mut i, "endfacet", last_line)?;
            triangles.push(corners);
        }
        objects.push(MeshObject {
            name: if name.is_empty() {
                default_name.to_string()
            } else {
                name.to_string()
            },
            triangles,
        });
    }
    Ok(objects)
}

fn expect(
    tokens: &[Token<'_>],
    i: &mut usize,
    keyword: &str,
    last_line: usize,
) -> Result<(), StlError> {
    let Some(token) = tokens.get(*i) else {
        return Err(syntax(
            last_line,
            format!("expected `{keyword}` before end of file"),
        ));
    };
    if !token.is(keyword) {
        return Err(syntax(
            token.line,
            format!("expected `{keyword}`, found `{}`", token.text),
        ));
    }
    *i += 1;
    Ok(())
}

fn number(tokens: &[Token<'_>], i: &mut usize, last_line: usize) -> Result<f32, StlError> {
    let Some(token) = tokens.get(*i) else {
        return Err(syntax(last_line, "expected a number before end of file"));
    };
    *i += 1;
    let value: f32 = token
        .text
        .parse()
        .map_err(|_| syntax(token.line, format!("`{}` is not a number", token.text)))?;
    if !value.is_finite() {
        return Err(syntax(
            token.line,
            format!("`{}` is not a finite number", token.text),
        ));
    }
    Ok(value)
}
