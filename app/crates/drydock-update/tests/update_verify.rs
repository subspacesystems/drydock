// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Acceptance tests for `m13-update-verify`. The key pair below is a throwaway made from a
//! fixed seed; it is not, and must never be replaced by, a real release key.

use std::io::Write;
use std::path::PathBuf;

use drydock_update::{
    Channel, ChannelError, FileCheckError, Manifest, ParseError, VerifyError, Version,
    hex_decode_32, hex_encode, verify, verify_file_sha256,
};
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};

fn key(seed: u8) -> SigningKey {
    SigningKey::from_bytes(&[seed; 32])
}

fn pubkey(seed: u8) -> [u8; 32] {
    key(seed).verifying_key().to_bytes()
}

fn sign(seed: u8, bytes: &[u8]) -> Vec<u8> {
    key(seed).sign(bytes).to_bytes().to_vec()
}

const HASH: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

fn manifest_text() -> String {
    format!(
        "version = 1.4.2\nnotes_url = https://example.invalid/notes/1.4.2\ninstaller_url = https://example.invalid/dl/drydock-1.4.2-setup.exe\nsize = 41943040\nsha256 = {HASH}\n"
    )
}

fn parsed() -> Manifest {
    Manifest::parse(manifest_text().as_bytes()).unwrap()
}

fn tmp(name: &str, data: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("drydock-update-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::File::create(&path)
        .unwrap()
        .write_all(data)
        .unwrap();
    path
}

// ---- Version ----

#[test]
fn version_parses_three_parts() {
    assert_eq!(
        Version::parse("0.12.1"),
        Some(Version {
            major: 0,
            minor: 12,
            patch: 1
        })
    );
    assert_eq!(Version::parse("10.0.300").unwrap().to_string(), "10.0.300");
    assert_eq!(Version::parse("01.2.3").unwrap().to_string(), "1.2.3");
}

#[test]
fn version_rejects_everything_else() {
    for bad in [
        "",
        "1",
        "1.2",
        "1.2.3.4",
        "1..3",
        ".1.2",
        "1.2.",
        "1.2.x",
        "v1.2.3",
        "1.2.3-rc1",
        "1.2.3 ",
        " 1.2.3",
        "+1.2.3",
        "-1.2.3",
        "1.2.3+4",
        "99999999999999999999.0.0",
        "１.2.3",
    ] {
        assert_eq!(Version::parse(bad), None, "{bad:?}");
    }
}

#[test]
fn version_order_is_numeric_per_part() {
    let v = |s: &str| Version::parse(s).unwrap();
    assert!(v("0.10.0").is_newer_than(&v("0.9.0")));
    assert!(!v("0.9.0").is_newer_than(&v("0.10.0")));
    assert!(v("1.0.0").is_newer_than(&v("0.99.99")));
    assert!(v("0.12.2").is_newer_than(&v("0.12.1")));
    assert!(v("0.12.10").is_newer_than(&v("0.12.9")));
    assert!(
        !v("0.12.1").is_newer_than(&v("0.12.1")),
        "equal is not newer"
    );
    assert!(!v("0.12.0").is_newer_than(&v("0.12.1")));
    assert!(v("2.0.0") > v("1.99.99"));
}

#[test]
fn running_version_parses() {
    assert!(Version::parse(env!("CARGO_PKG_VERSION")).is_some());
}

// ---- Manifest ----

#[test]
fn manifest_parses_good_text() {
    let m = parsed();
    assert_eq!(m.version, Version::parse("1.4.2").unwrap());
    assert_eq!(m.notes_url, "https://example.invalid/notes/1.4.2");
    assert_eq!(
        m.installer_url,
        "https://example.invalid/dl/drydock-1.4.2-setup.exe"
    );
    assert_eq!(m.size, 41_943_040);
    assert_eq!(hex_encode(&m.sha256), HASH);
}

#[test]
fn manifest_ignores_unknown_keys_comments_blank_lines_bom_and_crlf() {
    let text = format!(
        "\u{feff}# release 1.4.2\r\n\r\nfuture = a\r\nfuture = b\r\n{}",
        manifest_text().replace('\n', "\r\n")
    );
    assert_eq!(Manifest::parse(text.as_bytes()).unwrap(), parsed());
}

#[test]
fn manifest_trims_around_key_and_value_and_splits_at_first_equals() {
    let text = manifest_text().replace(
        "notes_url = https://example.invalid/notes/1.4.2",
        "  notes_url\t=  https://example.invalid/n?a=b=c  ",
    );
    assert_eq!(
        Manifest::parse(text.as_bytes()).unwrap().notes_url,
        "https://example.invalid/n?a=b=c"
    );
}

#[test]
fn manifest_missing_key_is_named() {
    for key in ["version", "notes_url", "installer_url", "size", "sha256"] {
        let text: String = manifest_text()
            .lines()
            .filter(|l| !l.starts_with(key))
            .map(|l| format!("{l}\n"))
            .collect();
        match Manifest::parse(text.as_bytes()) {
            Err(ParseError::Missing(k)) => assert_eq!(k, key),
            other => panic!("{key}: {other:?}"),
        }
    }
}

#[test]
fn manifest_duplicate_required_key_is_an_error() {
    for key in ["version", "notes_url", "installer_url", "size", "sha256"] {
        let line = manifest_text()
            .lines()
            .find(|l| l.starts_with(key))
            .unwrap()
            .to_string();
        let text = format!("{}{line}\n", manifest_text());
        match Manifest::parse(text.as_bytes()) {
            Err(ParseError::Duplicate(k)) => assert_eq!(k, key),
            other => panic!("{key}: {other:?}"),
        }
    }
}

#[test]
fn manifest_rejects_line_without_equals_with_its_number() {
    let text = format!("# c\n\nthis is not a pair\n{}", manifest_text());
    assert_eq!(
        Manifest::parse(text.as_bytes()),
        Err(ParseError::Syntax { line: 3 })
    );
}

#[test]
fn manifest_rejects_non_utf8() {
    assert_eq!(
        Manifest::parse(&[0xff, 0xfe, b'a']),
        Err(ParseError::NotUtf8)
    );
}

#[test]
fn manifest_rejects_invalid_values() {
    let cases: &[(&str, &str, &str)] = &[
        ("version", "1.4", "version"),
        ("version", "1.4.2-beta", "version"),
        ("notes_url", "http://example.invalid/n", "notes_url"),
        ("notes_url", "https://", "notes_url"),
        ("notes_url", "https://exa mple.invalid/n", "notes_url"),
        (
            "installer_url",
            "ftp://example.invalid/x.exe",
            "installer_url",
        ),
        ("installer_url", "", "installer_url"),
        ("size", "0", "size"),
        ("size", "-5", "size"),
        ("size", "12abc", "size"),
        ("size", "1.5", "size"),
        ("size", "99999999999999999999999", "size"),
        ("size", "", "size"),
        ("sha256", "abcd", "sha256"),
        ("sha256", &"g".repeat(64), "sha256"),
        ("sha256", &HASH.repeat(2), "sha256"),
    ];
    for (key, value, name) in cases {
        let text: String = manifest_text()
            .lines()
            .map(|l| {
                if l.starts_with(key) {
                    format!("{key} = {value}\n")
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        match Manifest::parse(text.as_bytes()) {
            Err(ParseError::Invalid(k)) => assert_eq!(k, *name, "{key}={value}"),
            other => panic!("{key}={value}: {other:?}"),
        }
    }
}

#[test]
fn manifest_accepts_upper_case_hash_and_prints_lower_case() {
    let text = manifest_text().replace(HASH, &HASH.to_uppercase());
    let m = Manifest::parse(text.as_bytes()).unwrap();
    assert_eq!(m, parsed());
    assert!(m.to_text().contains(HASH));
}

#[test]
fn manifest_to_text_is_canonical() {
    assert_eq!(parsed().to_text(), manifest_text());
}

#[test]
fn hex_helpers() {
    assert_eq!(hex_encode(&[0, 1, 0xab, 0xff]), "0001abff");
    assert_eq!(hex_encode(&[]), "");
    assert_eq!(
        hex_decode_32(HASH).map(|b| hex_encode(&b)),
        Some(HASH.to_string())
    );
    assert!(hex_decode_32(&HASH[..62]).is_none());
    assert!(hex_decode_32(&format!("{HASH}00")).is_none());
    assert!(hex_decode_32(&format!("zz{}", &HASH[2..])).is_none());
    assert!(hex_decode_32(&format!("+1{}", &HASH[2..])).is_none());
}

// ---- Signature ----

#[test]
fn good_signature_returns_the_manifest() {
    let bytes = manifest_text().into_bytes();
    let sig = sign(1, &bytes);
    assert_eq!(sig.len(), 64);
    assert_eq!(verify(&bytes, &sig, &pubkey(1)), Ok(parsed()));
}

#[test]
fn missing_signature() {
    assert_eq!(
        verify(manifest_text().as_bytes(), &[], &pubkey(1)),
        Err(VerifyError::Missing)
    );
}

#[test]
fn truncated_or_padded_signature_is_malformed() {
    let bytes = manifest_text().into_bytes();
    let sig = sign(1, &bytes);
    for len in [1, 32, 63] {
        assert_eq!(
            verify(&bytes, &sig[..len], &pubkey(1)),
            Err(VerifyError::Malformed),
            "{len}"
        );
    }
    let mut long = sig.clone();
    long.push(0);
    assert_eq!(
        verify(&bytes, &long, &pubkey(1)),
        Err(VerifyError::Malformed)
    );
}

#[test]
fn base64_text_is_not_accepted_as_a_signature() {
    let bytes = manifest_text().into_bytes();
    // 88 characters: what a base64 file would hold.
    assert_eq!(
        verify(&bytes, &[b'A'; 88], &pubkey(1)),
        Err(VerifyError::Malformed)
    );
}

#[test]
fn flipped_signature_bit_is_bad() {
    let bytes = manifest_text().into_bytes();
    for i in [0, 31, 32, 63] {
        let mut sig = sign(1, &bytes);
        sig[i] ^= 0x01;
        assert_eq!(
            verify(&bytes, &sig, &pubkey(1)),
            Err(VerifyError::BadSignature),
            "byte {i}"
        );
    }
}

#[test]
fn key_that_does_not_match_is_bad() {
    let bytes = manifest_text().into_bytes();
    let sig = sign(1, &bytes);
    assert_eq!(
        verify(&bytes, &sig, &pubkey(2)),
        Err(VerifyError::BadSignature)
    );
}

#[test]
fn unusable_public_key_is_bad_not_a_panic() {
    let bytes = manifest_text().into_bytes();
    let sig = sign(1, &bytes);
    assert_eq!(
        verify(&bytes, &sig, &[0xff; 32]),
        Err(VerifyError::BadSignature)
    );
    assert_eq!(
        verify(&bytes, &sig, &[0; 32]),
        Err(VerifyError::BadSignature)
    );
}

#[test]
fn manifest_changed_after_signing_is_bad() {
    let bytes = manifest_text().into_bytes();
    let sig = sign(1, &bytes);
    let swapped = manifest_text().replace("example.invalid/dl", "example.invalid/evil");
    assert_eq!(
        verify(swapped.as_bytes(), &sig, &pubkey(1)),
        Err(VerifyError::BadSignature)
    );
    // Even whitespace counts: the signature covers the exact bytes.
    let spaced = format!("{}\n", manifest_text());
    assert_eq!(
        verify(spaced.as_bytes(), &sig, &pubkey(1)),
        Err(VerifyError::BadSignature)
    );
    let crlf = manifest_text().replace('\n', "\r\n");
    assert_eq!(
        verify(crlf.as_bytes(), &sig, &pubkey(1)),
        Err(VerifyError::BadSignature)
    );
}

#[test]
fn signature_of_other_manifest_is_bad() {
    let other = manifest_text().replace("1.4.2", "1.4.3").into_bytes();
    let sig = sign(1, &other);
    assert_eq!(
        verify(manifest_text().as_bytes(), &sig, &pubkey(1)),
        Err(VerifyError::BadSignature)
    );
}

#[test]
fn validly_signed_garbage_is_a_parse_error() {
    let bytes = b"nothing useful here\n".to_vec();
    assert_eq!(
        verify(&bytes, &sign(1, &bytes), &pubkey(1)),
        Err(VerifyError::Parse(ParseError::Syntax { line: 1 }))
    );
    let missing = b"version = 1.0.0\n".to_vec();
    assert_eq!(
        verify(&missing, &sign(1, &missing), &pubkey(1)),
        Err(VerifyError::Parse(ParseError::Missing("notes_url")))
    );
}

#[test]
fn unsigned_garbage_reports_signature_not_parse() {
    // The manifest is never parsed when the signature fails.
    let bytes = b"nothing useful here\n".to_vec();
    assert_eq!(
        verify(&bytes, &sign(2, &bytes), &pubkey(1)),
        Err(VerifyError::BadSignature)
    );
}

// ---- File hash ----

fn digest(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

#[test]
fn file_matches() {
    let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    let path = tmp("good.bin", &data);
    assert_eq!(
        verify_file_sha256(&path, &digest(&data), data.len() as u64),
        Ok(())
    );
}

#[test]
fn empty_file_with_matching_hash() {
    let path = tmp("empty.bin", b"");
    assert_eq!(verify_file_sha256(&path, &digest(b""), 0), Ok(()));
}

#[test]
fn hash_mismatch() {
    let data = b"installer bytes".to_vec();
    let path = tmp("hash.bin", &data);
    let mut wrong = digest(&data);
    wrong[31] ^= 1;
    assert_eq!(
        verify_file_sha256(&path, &wrong, data.len() as u64),
        Err(FileCheckError::HashMismatch)
    );
}

#[test]
fn changed_byte_is_a_hash_mismatch() {
    let data = b"installer bytes".to_vec();
    let mut bad = data.clone();
    bad[3] ^= 0x20;
    let path = tmp("flip.bin", &bad);
    assert_eq!(
        verify_file_sha256(&path, &digest(&data), data.len() as u64),
        Err(FileCheckError::HashMismatch)
    );
}

#[test]
fn size_mismatch_both_ways() {
    let data = b"installer bytes".to_vec();
    let path = tmp("size.bin", &data);
    let n = data.len() as u64;
    assert_eq!(
        verify_file_sha256(&path, &digest(&data), n + 1),
        Err(FileCheckError::SizeMismatch {
            expected: n + 1,
            actual: n
        })
    );
    assert_eq!(
        verify_file_sha256(&path, &digest(&data), n - 1),
        Err(FileCheckError::SizeMismatch {
            expected: n - 1,
            actual: n
        })
    );
}

#[test]
fn size_wins_over_hash() {
    let path = tmp("both.bin", b"abc");
    assert_eq!(
        verify_file_sha256(&path, &[0; 32], 10),
        Err(FileCheckError::SizeMismatch {
            expected: 10,
            actual: 3
        })
    );
}

#[test]
fn missing_file_is_io_not_found() {
    let path = std::env::temp_dir().join("drydock-update-no-such-file.bin");
    assert_eq!(
        verify_file_sha256(&path, &[0; 32], 1),
        Err(FileCheckError::Io(std::io::ErrorKind::NotFound))
    );
}

#[test]
fn directory_is_an_io_error() {
    let r = verify_file_sha256(&std::env::temp_dir(), &[0; 32], 1);
    assert!(matches!(r, Err(FileCheckError::Io(_))), "{r:?}");
}

// ---- Channel ----

fn channel_text() -> String {
    format!(
        "# test channel\nname = stable\nmanifest_url = https://updates.example.invalid/latest/manifest.txt\npublic_key = {}\nheader = Authorization: Bearer abc.def\nheader = X-Note:  two words \n",
        hex_encode(&pubkey(1))
    )
}

#[test]
fn channel_parses() {
    let c = Channel::parse(channel_text().as_bytes()).unwrap();
    assert_eq!(c.name, "stable");
    assert_eq!(
        c.manifest_url,
        "https://updates.example.invalid/latest/manifest.txt"
    );
    assert_eq!(c.public_key, pubkey(1));
    assert_eq!(
        c.headers,
        vec![
            ("Authorization".to_string(), "Bearer abc.def".to_string()),
            ("X-Note".to_string(), "two words".to_string()),
        ]
    );
}

#[test]
fn channel_without_headers_and_with_unknown_keys() {
    let text = channel_text()
        .lines()
        .filter(|l| !l.starts_with("header"))
        .map(|l| format!("{l}\n"))
        .collect::<String>()
        + "color = blue\n";
    assert!(Channel::parse(text.as_bytes()).unwrap().headers.is_empty());
}

#[test]
fn channel_header_value_keeps_later_colons() {
    let text = channel_text() + "header = X-Time: 12:30:00\n";
    let c = Channel::parse(text.as_bytes()).unwrap();
    assert_eq!(
        c.headers.last().unwrap(),
        &("X-Time".to_string(), "12:30:00".to_string())
    );
}

#[test]
fn channel_missing_and_duplicate_keys() {
    for key in ["name", "manifest_url", "public_key"] {
        let text: String = channel_text()
            .lines()
            .filter(|l| !l.starts_with(key))
            .map(|l| format!("{l}\n"))
            .collect();
        match Channel::parse(text.as_bytes()) {
            Err(ChannelError::Missing(k)) => assert_eq!(k, key),
            other => panic!("{key}: {other:?}"),
        }
        let line = channel_text()
            .lines()
            .find(|l| l.starts_with(key))
            .unwrap()
            .to_string();
        match Channel::parse(format!("{}{line}\n", channel_text()).as_bytes()) {
            Err(ChannelError::Duplicate(k)) => assert_eq!(k, key),
            other => panic!("{key}: {other:?}"),
        }
    }
}

#[test]
fn channel_rejects_bad_values() {
    let base = channel_text();
    let bad = [
        (base.replace("name = stable", "name ="), "name"),
        (
            base.replace("https://updates", "http://updates"),
            "manifest_url",
        ),
        (base.replace("latest/manifest.txt", "a b"), "manifest_url"),
        (base.replace(&hex_encode(&pubkey(1)), "1234"), "public_key"),
        (
            base.replace(&hex_encode(&pubkey(1))[..2], "zz"),
            "public_key",
        ),
        (base.clone() + "header = no colon here\n", "header"),
        (base.clone() + "header = : value\n", "header"),
        (base.clone() + "header = Bad Name: v\n", "header"),
        (base.clone() + "header = X-Empty:   \n", "header"),
        (base.clone() + "header = X-Ctl: a\u{1}b\n", "header"),
    ];
    for (text, name) in bad {
        match Channel::parse(text.as_bytes()) {
            Err(ChannelError::Invalid(k)) => assert_eq!(k, name, "{text}"),
            other => panic!("{text}: {other:?}"),
        }
    }
}

#[test]
fn channel_rejects_syntax_and_encoding() {
    assert_eq!(
        Channel::parse(format!("{}oops\n", channel_text()).as_bytes()),
        Err(ChannelError::Syntax {
            line: channel_text().lines().count() + 1
        })
    );
    assert_eq!(Channel::parse(&[0xc3, 0x28]), Err(ChannelError::NotUtf8));
}

#[test]
fn channel_key_verifies_a_signed_manifest_end_to_end() {
    let c = Channel::parse(channel_text().as_bytes()).unwrap();
    let bytes = manifest_text().into_bytes();
    assert_eq!(
        verify(&bytes, &sign(1, &bytes), &c.public_key),
        Ok(parsed())
    );
    assert_eq!(
        verify(&bytes, &sign(2, &bytes), &c.public_key),
        Err(VerifyError::BadSignature)
    );
}

// ---- Properties ----

mod props {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn manifest_round_trips(
            (a, b, c) in (0u64..1000, 0u64..1000, 0u64..1000),
            size in 1u64..u64::MAX,
            hash in proptest::array::uniform32(any::<u8>()),
            notes in "[a-z0-9/._?=&-]{1,40}",
            inst in "[a-z0-9/._?=&-]{1,40}",
        ) {
            let m = Manifest {
                version: Version { major: a, minor: b, patch: c },
                notes_url: format!("https://example.invalid/{notes}"),
                installer_url: format!("https://example.invalid/{inst}"),
                size,
                sha256: hash,
            };
            let text = m.to_text();
            prop_assert_eq!(Manifest::parse(text.as_bytes()), Ok(m.clone()));
            prop_assert_eq!(m.to_text(), text);
        }

        #[test]
        fn version_order_matches_tuple_order(
            a in (0u64..50, 0u64..50, 0u64..50),
            b in (0u64..50, 0u64..50, 0u64..50),
        ) {
            let va = Version::parse(&format!("{}.{}.{}", a.0, a.1, a.2)).unwrap();
            let vb = Version::parse(&format!("{}.{}.{}", b.0, b.1, b.2)).unwrap();
            prop_assert_eq!(va.is_newer_than(&vb), a > b);
        }

        #[test]
        fn any_single_byte_change_breaks_the_signature(index in 0usize..1000, delta in 1u8..=255) {
            let mut bytes = manifest_text().into_bytes();
            let sig = sign(1, &bytes);
            let i = index % bytes.len();
            bytes[i] = bytes[i].wrapping_add(delta);
            prop_assert_eq!(verify(&bytes, &sig, &pubkey(1)), Err(VerifyError::BadSignature));
        }

        #[test]
        fn hex_round_trips(bytes in proptest::array::uniform32(any::<u8>())) {
            prop_assert_eq!(hex_decode_32(&hex_encode(&bytes)), Some(bytes));
        }

        #[test]
        fn parsers_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
            let _ = Manifest::parse(&bytes);
            let _ = Channel::parse(&bytes);
            let _ = verify(&bytes, &bytes, &[1; 32]);
        }
    }
}
