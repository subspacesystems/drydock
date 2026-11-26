// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! The pure parts of starting the installer through the Windows shell
//! (`docs/specs/m13-update-launch-fix.md`).

use drydock_update::{
    ERROR_CANCELLED, LAUNCH_CANCELLED, join_parameters, launch_failure, quote_arg,
};

#[test]
fn plain_arguments_stay_as_they_are() {
    assert_eq!(quote_arg("/S"), "/S");
    assert_eq!(quote_arg("/UPDATE"), "/UPDATE");
    assert_eq!(quote_arg(r"C:\a\b.exe"), r"C:\a\b.exe");
    assert_eq!(quote_arg("a&b"), "a&b");
}

#[test]
fn empty_and_spaced_arguments_get_quotes() {
    assert_eq!(quote_arg(""), "\"\"");
    assert_eq!(quote_arg("two words"), "\"two words\"");
    assert_eq!(quote_arg("tab\there"), "\"tab\there\"");
    assert_eq!(quote_arg(r"C:\Program Files\x"), r#""C:\Program Files\x""#);
}

#[test]
fn double_quotes_are_escaped() {
    assert_eq!(quote_arg("say \"hi\""), r#""say \"hi\"""#);
    assert_eq!(quote_arg("\""), r#""\"""#);
}

#[test]
fn backslashes_before_a_quote_are_doubled() {
    // A trailing backslash would swallow the closing quote, so it is doubled.
    assert_eq!(quote_arg(r"C:\My Dir\"), r#""C:\My Dir\\""#);
    // Backslashes before an escaped quote are doubled, plus one for the quote itself.
    assert_eq!(quote_arg(r#"a\"b"#), r#""a\\\"b""#);
    // Backslashes elsewhere are left alone.
    assert_eq!(quote_arg(r"a\b c"), r#""a\b c""#);
}

#[test]
fn parameters_are_joined_by_single_spaces() {
    assert_eq!(join_parameters(&[]), "");
    assert_eq!(join_parameters(&["/S", "/UPDATE"]), "/S /UPDATE");
    assert_eq!(join_parameters(&["/S", "a b", ""]), "/S \"a b\" \"\"");
}

#[test]
fn a_declined_prompt_is_the_cancelled_reason() {
    assert_eq!(ERROR_CANCELLED, 1223);
    assert_eq!(
        launch_failure(1223, "The operation was canceled by the user."),
        LAUNCH_CANCELLED
    );
    assert_eq!(launch_failure(1223, ""), LAUNCH_CANCELLED);
}

#[test]
fn other_failures_keep_the_system_text() {
    assert_eq!(
        launch_failure(2, "  The system cannot find the file specified.\r\n"),
        "The system cannot find the file specified."
    );
    assert_eq!(launch_failure(5, "Access is denied."), "Access is denied.");
}

#[test]
fn a_failure_without_text_names_the_code() {
    assert_eq!(launch_failure(740, ""), "error 740");
    assert_eq!(launch_failure(740, " \r\n"), "error 740");
    assert_ne!(launch_failure(740, ""), LAUNCH_CANCELLED);
}
