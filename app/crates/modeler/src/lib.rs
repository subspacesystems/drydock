// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! modeler (codename).

/// One-line product banner.
pub fn banner() -> String {
    format!("modeler {}", env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn banner_names_the_codename() {
        assert!(super::banner().starts_with("modeler "));
    }
}
