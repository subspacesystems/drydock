# m13-update-verify: manifest, signature, hash, version and channel checks

Milestone M13 (Windows auto-update). A new platform-neutral crate `drydock-update` holds the
pieces that decide whether an update is genuine. Nothing here touches the network, the
registry or a window: the only I/O is reading one file to hash it. Later tasks (release
tool, background updater, WinHTTP transport) build on it.

The skeleton is on main (`app/crates/drydock-update/src/*.rs`, bodies `unimplemented!()`,
doc comments give the exact rules). The workspace entry, the `ed25519-dalek` and `sha2`
dependencies and the `deny.toml` exception are already on main. Fill in the bodies; do not
change signatures, error variants or the test file. Acceptance tests:
`app/crates/drydock-update/tests/update_verify.rs`.

## What to build

**Version** (`version.rs`). `Version::parse` takes exactly `X.Y.Z`, digits only, three parts,
each fitting u64; anything else (empty part, sign, space, suffix, fourth part, non-ASCII
digits) is `None`. `is_newer_than` is strictly greater, numeric per part (the derived `Ord`
on `major, minor, patch` does it). `Display` prints `X.Y.Z`. The running version is
`env!("CARGO_PKG_VERSION")`, supplied by the caller.

**Manifest** (`manifest.rs`). A line-based text file, our own parser (no serde). Format and
validation are in the doc comments on `Manifest::parse` and in the crate docs in `lib.rs`:
`key = value`, split at the first `=`, both sides trimmed, `#` comments and blank lines
skipped, unknown keys ignored, BOM and CRLF accepted. The five required keys are `version`,
`notes_url`, `installer_url`, `size`, `sha256`. A missing one is `Missing(key)`, a repeated
one `Duplicate(key)`, a bad value `Invalid(key)`, a non-pair line `Syntax { line }` (1-based,
counting blank and comment lines). URLs must start with `https://`, have something after
it and contain no whitespace or control characters. `size` is decimal digits only, at least
1. `sha256` is 64 hex digits. `to_text` prints the canonical form (key order, `\n`, trailing
newline, lower-case hex) and must parse back to an equal value. `hex_encode` and
`hex_decode_32` are public because the channel parser and the release tool use them.

**Signature** (`verify.rs`). `verify(manifest_bytes, signature, public_key)` checks a
detached Ed25519 signature over the manifest file's exact bytes, then parses. The signature
file is the 64 raw bytes, not base64 (the test with 88 characters pins that). Order: empty
is `Missing`; length other than 64 is `Malformed`; a key that is not a valid point, or a
signature that does not verify, is `BadSignature` (use `VerifyingKey::verify_strict`, so a
weak key such as all zeros fails and nothing panics); only then `Manifest::parse`, whose
error comes back as `Parse`. A manifest whose signature fails is never parsed (the test
signs garbage with another key and expects `BadSignature`, not `Parse`).

**File check** (`verify_file_sha256`). Read the file in fixed-size chunks (for example
64 KiB), counting bytes and feeding SHA-256; never read it all into memory. If the count
passes `expected_size`, stop at once with `SizeMismatch` (do not hash a huge file to the
end). After the last chunk: wrong count is `SizeMismatch { expected, actual }`, then a wrong
hash is `HashMismatch`. Open or read failures are `Io(kind)`.

**Channel** (`channel.rs`). `Channel::parse` reads the same key=value style: `name`,
`manifest_url`, `public_key` (64 hex digits) once each, and `header = Name: value` any number
of times, kept in file order. Rules are in the doc comment. A header name is letters,
digits and `-` only; a value with a control character (so CR or LF) or an empty value is
rejected, so a channel file cannot inject extra header lines.

## Rules for the crate

- Dependencies are `ed25519-dalek` (default features off, `verify` side only in the library)
  and `sha2`. Add nothing else; no serde, no base64 crate, no network crate.
- No panics on any input (the tests feed random bytes to every parser).
- Never put a real host, key or secret in code, tests or docs: this repo becomes public.
  Tests use `example.invalid` URLs and a throwaway key from a fixed seed.
- Keep `lib.rs` re-exports as they are; later tasks import from the crate root.

## Tests (already written)

`tests/update_verify.rs`: version parsing and numeric ordering; manifest good text, unknown
keys, comments, BOM and CRLF, trimming, missing and duplicate keys per key, bad values
per key, syntax line number, non-UTF-8, canonical output; hex helpers; signature good,
missing, truncated and padded, base64 text, flipped bits, wrong key, unusable key, manifest
changed after signing (including added whitespace and CRLF), signature of another manifest,
validly signed garbage, unsigned garbage; file hash match, empty file, hash and size
mismatch, size wins over hash, missing file, directory; channel parsing, headers, missing,
duplicate and bad values, syntax; an end-to-end check with a channel's key; properties
(manifest round trip, version order, any single-byte change breaks the signature, hex round
trip, no parser panics).

## Not in scope

Fetching anything, the release tool that signs (`m13-release-manifest`), the updater state
machine (`m13-update-check`), any UI, key storage or configuration file locations.

## Where to look

- `Version::parse`, `Version::is_newer_than`; `Manifest::parse`, `Manifest::to_text`;
  `hex_encode`, `hex_decode_32`; `verify`, `verify_file_sha256`; `Channel::parse`; the error
  enums `ParseError`, `VerifyError`, `FileCheckError`, `ChannelError`
  (`app/crates/drydock-update/src/`)
- `ed25519_dalek::VerifyingKey::from_bytes`, `VerifyingKey::verify_strict`,
  `ed25519_dalek::Signature::from_bytes`; `sha2::Sha256` with `Digest` (`show <name> ed25519-dalek`)
- Test helpers in `tests/update_verify.rs`: `key`, `pubkey`, `sign`, `manifest_text`,
  `parsed`, `channel_text`, `tmp`, `digest`
