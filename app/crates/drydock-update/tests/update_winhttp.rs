// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `m13-update-winhttp`: the platform-neutral rules of the Windows
//! transport. The WinHTTP calls themselves are checked by the `winclippy` gate and on Windows.

use drydock_update::{
    BodyBudget, CONNECT_TIMEOUT_MS, GET_TOTAL_TIMEOUT, HttpsUrl, RECEIVE_TIMEOUT_MS,
    TransportError, USER_AGENT, check_status, error_from_win32, header_block, parse_https_url,
};

fn other<T: std::fmt::Debug>(r: Result<T, TransportError>) {
    assert!(matches!(r, Err(TransportError::Other(_))), "got {r:?}");
}

fn url(host: &str, port: u16, path: &str) -> HttpsUrl {
    HttpsUrl {
        host: host.into(),
        port,
        path: path.into(),
    }
}

fn h(n: &str, v: &str) -> (String, String) {
    (n.into(), v.into())
}

// ---- URLs ----

#[test]
fn url_plain() {
    assert_eq!(
        parse_https_url("https://example.invalid/a/b.txt"),
        Ok(url("example.invalid", 443, "/a/b.txt"))
    );
}

#[test]
fn url_without_path_gets_slash() {
    assert_eq!(
        parse_https_url("https://example.invalid"),
        Ok(url("example.invalid", 443, "/"))
    );
}

#[test]
fn url_query_without_path() {
    assert_eq!(
        parse_https_url("https://example.invalid?x=1"),
        Ok(url("example.invalid", 443, "/?x=1"))
    );
}

#[test]
fn url_port_and_query() {
    assert_eq!(
        parse_https_url("https://example.invalid:8443/dl?file=a%20b&v=2"),
        Ok(url("example.invalid", 8443, "/dl?file=a%20b&v=2"))
    );
}

#[test]
fn url_fragment_is_dropped() {
    assert_eq!(
        parse_https_url("https://example.invalid/a?b=1#frag"),
        Ok(url("example.invalid", 443, "/a?b=1"))
    );
    assert_eq!(
        parse_https_url("https://example.invalid#frag"),
        Ok(url("example.invalid", 443, "/"))
    );
}

#[test]
fn url_scheme_is_case_insensitive_host_kept_as_given() {
    assert_eq!(
        parse_https_url("HTTPS://Example.Invalid/x"),
        Ok(url("Example.Invalid", 443, "/x"))
    );
}

#[test]
fn url_ipv6_literal() {
    assert_eq!(
        parse_https_url("https://[::1]:9000/x"),
        Ok(url("::1", 9000, "/x"))
    );
    assert_eq!(
        parse_https_url("https://[2001:db8::2]/x"),
        Ok(url("2001:db8::2", 443, "/x"))
    );
}

#[test]
fn url_refuses_other_schemes() {
    for u in [
        "http://example.invalid/a",
        "HTTP://example.invalid/a",
        "ftp://example.invalid/a",
        "file:///c:/x",
        "//example.invalid/a",
        "example.invalid/a",
        "https:/example.invalid/a",
        "",
    ] {
        other(parse_https_url(u));
    }
}

#[test]
fn url_refuses_bad_authority() {
    for u in [
        "https:///path",
        "https://:443/path",
        "https://user@example.invalid/a",
        "https://user:pw@example.invalid/a",
        "https://example.invalid:/a",
        "https://example.invalid:0/a",
        "https://example.invalid:65536/a",
        "https://example.invalid:80a/a",
        "https://[::1/a",
        "https://[]/a",
    ] {
        other(parse_https_url(u));
    }
}

#[test]
fn url_refuses_whitespace_and_control_characters() {
    for u in [
        "https://example.invalid/a b",
        "https://example.invalid/a\r\nHost: x",
        "https://example.invalid/a\tb",
        "https://exa mple.invalid/",
        " https://example.invalid/",
        "https://example.invalid/\u{0}",
    ] {
        other(parse_https_url(u));
    }
}

// ---- Headers ----

#[test]
fn header_block_formats_in_order() {
    let block = header_block(&[h("Authorization", "Bearer abc"), h("X-Channel", "dev")]).unwrap();
    assert_eq!(block, "Authorization: Bearer abc\r\nX-Channel: dev\r\n");
}

#[test]
fn header_block_empty() {
    assert_eq!(header_block(&[]).unwrap(), "");
}

#[test]
fn header_block_keeps_colons_and_inner_spaces_in_values() {
    assert_eq!(
        header_block(&[h("X-Note", "a: b  c")]).unwrap(),
        "X-Note: a: b  c\r\n"
    );
}

#[test]
fn header_block_refuses_injection_and_bad_names() {
    for bad in [
        h("X-A", "v\r\nX-B: w"),
        h("X-A", "v\nw"),
        h("X-A", "v\u{7}"),
        h("X-A", ""),
        h("", "v"),
        h("X A", "v"),
        h("X:A", "v"),
        h("X-A\r\n", "v"),
        h("Näme", "v"),
    ] {
        other(header_block(&[h("Good", "1"), bad]));
    }
}

// ---- Status ----

#[test]
fn status_success_range() {
    for s in [200, 201, 204, 299] {
        assert_eq!(check_status(s), Ok(()));
    }
}

#[test]
fn status_everything_else_is_http_error() {
    for s in [100, 199, 300, 304, 400, 401, 403, 404, 500, 503] {
        assert_eq!(check_status(s), Err(TransportError::Http(s)));
    }
}

// ---- Size limit ----

#[test]
fn budget_allows_exactly_the_limit() {
    let mut b = BodyBudget::new(10);
    assert_eq!(b.add(4), Ok(()));
    assert_eq!(b.add(6), Ok(()));
    assert_eq!(b.total(), 10);
    assert_eq!(b.add(1), Err(TransportError::TooLarge));
}

#[test]
fn budget_zero_limit() {
    let mut b = BodyBudget::new(0);
    assert_eq!(b.add(0), Ok(()));
    assert_eq!(b.add(1), Err(TransportError::TooLarge));
}

#[test]
fn budget_single_chunk_over_the_limit() {
    let mut b = BodyBudget::new(100);
    assert_eq!(b.add(101), Err(TransportError::TooLarge));
}

#[test]
fn budget_does_not_overflow() {
    let mut b = BodyBudget::new(u64::MAX - 1);
    assert_eq!(b.add(u64::MAX - 1), Ok(()));
    assert_eq!(b.add(u64::MAX), Err(TransportError::TooLarge));
    assert_eq!(b.add(1), Err(TransportError::TooLarge));
}

#[test]
fn budget_declared_length() {
    let b = BodyBudget::new(1000);
    assert_eq!(b.check_declared(None), Ok(()));
    assert_eq!(b.check_declared(Some(0)), Ok(()));
    assert_eq!(b.check_declared(Some(1000)), Ok(()));
    assert_eq!(b.check_declared(Some(1001)), Err(TransportError::TooLarge));
}

#[test]
fn budget_declared_length_does_not_count_bytes() {
    let b = BodyBudget::new(1000);
    b.check_declared(Some(900)).unwrap();
    assert_eq!(b.total(), 0);
}

// ---- Error mapping ----

#[test]
fn network_failures_are_offline() {
    for code in [12002, 12007, 12029, 12030] {
        assert_eq!(error_from_win32(code), TransportError::Offline, "{code}");
    }
}

#[test]
fn other_failures_carry_the_code() {
    for code in [12005, 12157, 12175, 5, 0] {
        match error_from_win32(code) {
            TransportError::Other(text) => assert!(text.contains(&code.to_string()), "{text}"),
            e => panic!("{code}: {e:?}"),
        }
    }
}

// ---- Constants ----

#[test]
fn constants_are_sane() {
    assert_eq!(CONNECT_TIMEOUT_MS, 10_000);
    assert!((10_000..=60_000).contains(&RECEIVE_TIMEOUT_MS));
    assert!(GET_TOTAL_TIMEOUT.as_secs() >= 30 && GET_TOTAL_TIMEOUT.as_secs() <= 120);
    assert!(USER_AGENT.starts_with("Drydock"));
    assert!(!USER_AGENT.contains(char::is_whitespace));
}
