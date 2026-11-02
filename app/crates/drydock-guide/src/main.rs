// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! `drydock-guide SRC_DIR OUT_DIR`: builds the user guide (`docs/specs/m8-guide-build.md`).
//! Prints `drydock-guide: <error>` to stderr and exits 1 when the build fails.

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: drydock-guide SRC_DIR OUT_DIR");
        return ExitCode::from(2);
    }
    let src = PathBuf::from(&args[0]);
    let out = PathBuf::from(&args[1]);

    match drydock_guide::build(&src, &out) {
        Ok(report) => {
            println!("drydock-guide: built {} pages", report.pages.len());
            for orphan in &report.orphans {
                println!("drydock-guide: not in the contents: {orphan}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("drydock-guide: {error}");
            ExitCode::from(1)
        }
    }
}
