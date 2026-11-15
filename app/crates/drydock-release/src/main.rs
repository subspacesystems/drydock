// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdout = std::io::stdout();
    let env = |name: &str| std::env::var(name).ok();
    if let Err(e) = drydock_release::run(&args, &env, &mut stdout) {
        eprintln!("drydock-release: {e}");
        std::process::exit(1);
    }
}
