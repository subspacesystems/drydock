# m13-update-winhttp: Windows transport over WinHTTP

`drydock-update` has a `Transport` trait (`m13-update-check`) that the updater uses for the manifest and
the installer download. This task implements it on Windows with WinHTTP, and the platform-neutral rules
behind it. Why WinHTTP: no TLS stack to add, the Windows certificate store (a private CA the machine
trusts works), and the system proxy. ADR-009 allows `unsafe` in `winhttp.rs` only.

Skeletons are on main: `src/net.rs` (plain functions, `unimplemented!`) and `src/winhttp.rs`
(`WinHttpTransport`, `#[cfg(windows)]`). Write the bodies; do not change signatures, the `Cargo.toml`
files or `lib.rs`. Tests: `tests/update_winhttp.rs` (Linux, must pass); the Windows code is checked by
compiling (`cargo clippy --target x86_64-pc-windows-gnu -p drydock-update -- -D warnings` must be clean).

## net.rs (tests decide)

- `parse_https_url`: the scheme must be `https` (any case); anything else, or no `://`, is
  `Other`. Authority is what follows up to the first `/`, `?` or `#`. No whitespace or control character
  anywhere (check the whole text first). No `@` in the authority. Host non-empty; IPv6 literal in
  `[..]` is returned without brackets. Port after `:` (after the `]` for IPv6) is 1..=65535 digits; an
  empty or bad port is `Other`; default 443. Path = the rest up to `#`; empty becomes `/`; a rest that
  starts with `?` becomes `/?...`. The host keeps its case.
- `header_block`: `Name: value\r\n` per header, in order. Name: 1+ ASCII letters, digits, `-`. Value:
  non-empty, no control characters (so no CR/LF). Any bad header fails the whole call with `Other`.
- `check_status`: 200..=299 ok, else `Http(status)`.
- `error_from_win32`: 12002 (timeout), 12007 (name not resolved), 12029 (cannot connect), 12030
  (connection error) are `Offline`; every other code is `Other(format!("WinHTTP error {code}"))`.
- `BodyBudget`: `add` counts with saturating arithmetic; `TooLarge` when the total is above the limit
  (equal is fine). `check_declared`: `TooLarge` when a `Content-Length` is above the limit; `None` is fine.

## winhttp.rs (not unit-tested; keep it short and boring)

Each call opens its own session, connection and request and closes every handle on every path (a small
private guard type whose `Drop` calls `WinHttpCloseHandle`; no handle leaks on early returns).

1. `parse_https_url` and `header_block` first; refuse before any network use. `http://` never reaches
   WinHTTP, and no code path may downgrade: set the redirect policy to
   `WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP` explicitly (Gitea release downloads redirect,
   so redirects stay on, WinHTTP's default limit of 10).
2. `WinHttpOpen` with `USER_AGENT` and `WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY` (system proxy settings;
   if that is rejected on an old system, fall back to `WINHTTP_ACCESS_TYPE_DEFAULT_PROXY`).
3. `WinHttpSetTimeouts` with the constants from `net.rs`. For `get`, also stop with `Offline` when
   `GET_TOTAL_TIMEOUT` has passed (check between reads).
4. `WinHttpConnect` (host, port), `WinHttpOpenRequest` ("GET", path, secure flag
   `WINHTTP_FLAG_SECURE`), `WinHttpAddRequestHeaders` with the `header_block` (skip it when empty;
   `WINHTTP_ADDREQ_FLAG_ADD | WINHTTP_ADDREQ_FLAG_REPLACE`), `WinHttpSendRequest`,
   `WinHttpReceiveResponse`. All text goes in as UTF-16. Do not add any header of your own.
5. Status from `WinHttpQueryHeaders` (`WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER`) →
   `check_status`. Content length from `WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER`
   (absent is fine) → `BodyBudget::check_declared`, before reading anything.
6. Read with `WinHttpQueryDataAvailable` / `WinHttpReadData` in chunks of at most `CHUNK_BYTES`,
   counting each chunk with `BodyBudget::add` before keeping or writing it.
7. `get`: collect into a `Vec`. `download`: create or truncate `dest` first thing after the URL checks,
   write each chunk, call `progress(total_so_far)` after each, check `cancel`
   (`Ordering::Relaxed`) before every read and return `Cancelled` when set; flush at the end. A failed
   `File` call is `Io(kind)`. The caller deletes a half-written file.
8. A failing WinHTTP call becomes `error_from_win32(error.code().0 as u32 & 0xFFFF)` when the code is
   an HRESULT-wrapped Win32 error (`HRESULT_FROM_WIN32` has the low 16 bits = the code), otherwise from
   `GetLastError`; pick one approach and use it everywhere. Use no `unwrap`/`expect`, no `panic!` in
   `winhttp.rs`.

Do not log or print URLs, headers or bodies. Leave `app/crates/drydock` alone (the UI task wires the
transport in).

## Where to look

- `show Transport drydock-update`, `TransportError`: the trait and its error cases (`transport.rs`).
- `drydock-update/src/net.rs`: every function and constant to fill in or use; tests in
  `drydock-update/tests/update_winhttp.rs` (helpers `url`, `h`, `other`).
- `drydock-update/src/winhttp.rs`: `WinHttpTransport`, its two `Transport` methods.
- `show Channel drydock-update` (`headers: Vec<(String, String)>` is what `header_block` receives).
- WinHTTP functions: `show WinHttpOpen windows` and the neighbours (`WinHttpConnect`, `WinHttpOpenRequest`,
  `WinHttpSendRequest`, `WinHttpReceiveResponse`, `WinHttpQueryHeaders`, `WinHttpReadData`,
  `WinHttpSetTimeouts`, `WinHttpSetOption`, `WinHttpCloseHandle`), crate `windows` 0.62 (feature
  `Win32_Networking_WinHttp`).
