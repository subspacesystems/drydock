// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m4-ui-gpu-info: the adapter line. See docs/specs/m4-ui-gpu-info.md.

use drydock::chrome::adapter_line;

#[test]
fn full_line() {
    assert_eq!(
        adapter_line("Quadro P2000", "Vulkan", "discrete GPU"),
        "Graphics: Quadro P2000 (Vulkan, discrete GPU)"
    );
}

#[test]
fn arguments_are_trimmed() {
    assert_eq!(
        adapter_line("  llvmpipe ", " Vulkan", "CPU  "),
        "Graphics: llvmpipe (Vulkan, CPU)"
    );
}

#[test]
fn empty_name_is_unknown() {
    assert_eq!(
        adapter_line("", "DX12", "other"),
        "Graphics: unknown adapter (DX12, other)"
    );
}

#[test]
fn empty_backend_or_type_is_left_out() {
    assert_eq!(adapter_line("X", "", "CPU"), "Graphics: X (CPU)");
    assert_eq!(adapter_line("X", "Vulkan", ""), "Graphics: X (Vulkan)");
    assert_eq!(adapter_line("X", "", ""), "Graphics: X");
}
