// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! Windows transport over WinHTTP. Behaviour: `docs/specs/m13-update-winhttp.md`.

#![allow(unsafe_code)] // WinHTTP calls; docs/adr/009-winhttp-unsafe.md

use std::ffi::c_void;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use windows::Win32::Networking::WinHttp::{
    ERROR_WINHTTP_HEADER_NOT_FOUND, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, WINHTTP_ADDREQ_FLAG_ADD, WINHTTP_ADDREQ_FLAG_REPLACE,
    WINHTTP_FLAG_SECURE, WINHTTP_OPTION_REDIRECT_POLICY,
    WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP, WINHTTP_QUERY_CONTENT_LENGTH,
    WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpAddRequestHeaders,
    WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetOption, WinHttpSetTimeouts,
};
use windows::core::{Error, PCWSTR};

use crate::net::{
    BodyBudget, CHUNK_BYTES, CONNECT_TIMEOUT_MS, GET_TOTAL_TIMEOUT, HttpsUrl, RECEIVE_TIMEOUT_MS,
    RESOLVE_TIMEOUT_MS, SEND_TIMEOUT_MS, USER_AGENT, check_status, error_from_win32, header_block,
    parse_https_url,
};
use crate::{Transport, TransportError};

/// Stateless: every call opens its own WinHTTP session and closes it before returning.
#[derive(Clone, Copy, Debug, Default)]
pub struct WinHttpTransport;

impl WinHttpTransport {
    pub fn new() -> WinHttpTransport {
        WinHttpTransport
    }
}

/// Owns the three handles of one request and closes them, child first, on every path.
struct Handles {
    request: *mut c_void,
    connect: *mut c_void,
    session: *mut c_void,
}

impl Handles {
    fn empty() -> Handles {
        Handles {
            request: std::ptr::null_mut(),
            connect: std::ptr::null_mut(),
            session: std::ptr::null_mut(),
        }
    }
}

impl Drop for Handles {
    fn drop(&mut self) {
        unsafe {
            if !self.request.is_null() {
                let _ = WinHttpCloseHandle(self.request);
            }
            if !self.connect.is_null() {
                let _ = WinHttpCloseHandle(self.connect);
            }
            if !self.session.is_null() {
                let _ = WinHttpCloseHandle(self.session);
            }
        }
    }
}

struct Request {
    handles: Handles,
}

impl Request {
    fn open(url: &HttpsUrl, headers: &str) -> Result<Request, TransportError> {
        let mut handles = Handles::empty();
        let agent = wide(USER_AGENT);

        let session = unsafe {
            WinHttpOpen(
                PCWSTR(agent.as_ptr()),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            )
        };
        let session = if session.is_null() {
            non_null(unsafe {
                WinHttpOpen(
                    PCWSTR(agent.as_ptr()),
                    WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
                    PCWSTR::null(),
                    PCWSTR::null(),
                    0,
                )
            })?
        } else {
            session
        };
        handles.session = session;

        unsafe {
            WinHttpSetTimeouts(
                handles.session,
                RESOLVE_TIMEOUT_MS,
                CONNECT_TIMEOUT_MS,
                SEND_TIMEOUT_MS,
                RECEIVE_TIMEOUT_MS,
            )
        }
        .map_err(win_error)?;

        let host = wide(&url.host);
        handles.connect = non_null(unsafe {
            WinHttpConnect(handles.session, PCWSTR(host.as_ptr()), url.port, 0)
        })?;

        let verb = wide("GET");
        let path = wide(&url.path);
        handles.request = non_null(unsafe {
            WinHttpOpenRequest(
                handles.connect,
                PCWSTR(verb.as_ptr()),
                PCWSTR(path.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            )
        })?;

        let policy = WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP.to_ne_bytes();
        unsafe {
            WinHttpSetOption(
                Some(handles.request as *const c_void),
                WINHTTP_OPTION_REDIRECT_POLICY,
                Some(&policy),
            )
        }
        .map_err(win_error)?;

        if !headers.is_empty() {
            let block: Vec<u16> = headers.encode_utf16().collect();
            unsafe {
                WinHttpAddRequestHeaders(
                    handles.request,
                    &block,
                    WINHTTP_ADDREQ_FLAG_ADD | WINHTTP_ADDREQ_FLAG_REPLACE,
                )
            }
            .map_err(win_error)?;
        }

        unsafe { WinHttpSendRequest(handles.request, None, None, 0, 0, 0) }.map_err(win_error)?;
        unsafe { WinHttpReceiveResponse(handles.request, std::ptr::null_mut()) }
            .map_err(win_error)?;

        Ok(Request { handles })
    }

    fn receive(&self, budget: &mut BodyBudget) -> Result<(), TransportError> {
        let status = query_u32(
            self.handles.request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
        )?
        .ok_or_else(|| TransportError::Other("missing HTTP status".into()))?;
        check_status(status as u16)?;

        let length = query_u32(
            self.handles.request,
            WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER,
        )?;
        budget.check_declared(length.map(u64::from))
    }

    fn read<B, C>(
        &self,
        budget: &mut BodyBudget,
        mut before_read: B,
        mut on_chunk: C,
    ) -> Result<(), TransportError>
    where
        B: FnMut() -> Result<(), TransportError>,
        C: FnMut(&[u8], u64) -> Result<(), TransportError>,
    {
        loop {
            before_read()?;

            let mut available = 0u32;
            unsafe { WinHttpQueryDataAvailable(self.handles.request, &mut available) }
                .map_err(win_error)?;
            if available == 0 {
                break;
            }

            let want = (available as usize).min(CHUNK_BYTES);
            let mut buffer = vec![0u8; want];
            let mut read = 0u32;
            unsafe {
                WinHttpReadData(
                    self.handles.request,
                    buffer.as_mut_ptr().cast(),
                    want as u32,
                    &mut read,
                )
            }
            .map_err(win_error)?;
            if read == 0 {
                break;
            }

            buffer.truncate(read as usize);
            budget.add(read as u64)?;
            on_chunk(&buffer, budget.total())?;
        }
        Ok(())
    }
}

impl Transport for WinHttpTransport {
    fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
        max_bytes: u64,
    ) -> Result<Vec<u8>, TransportError> {
        let started = Instant::now();
        let url = parse_https_url(url)?;
        let block = header_block(headers)?;
        let request = Request::open(&url, &block)?;
        let mut budget = BodyBudget::new(max_bytes);
        request.receive(&mut budget)?;

        let mut body = Vec::new();
        request.read(
            &mut budget,
            || {
                if started.elapsed() > GET_TOTAL_TIMEOUT {
                    Err(TransportError::Offline)
                } else {
                    Ok(())
                }
            },
            |chunk, _total| {
                body.extend_from_slice(chunk);
                Ok(())
            },
        )?;
        Ok(body)
    }

    fn download(
        &self,
        url: &str,
        headers: &[(String, String)],
        max_bytes: u64,
        dest: &Path,
        cancel: &AtomicBool,
        progress: &mut dyn FnMut(u64),
    ) -> Result<(), TransportError> {
        let url = parse_https_url(url)?;
        let block = header_block(headers)?;
        let mut file = File::create(dest).map_err(|e| TransportError::Io(e.kind()))?;

        let request = Request::open(&url, &block)?;
        let mut budget = BodyBudget::new(max_bytes);
        request.receive(&mut budget)?;

        request.read(
            &mut budget,
            || {
                if cancel.load(Ordering::Relaxed) {
                    Err(TransportError::Cancelled)
                } else {
                    Ok(())
                }
            },
            |chunk, total| {
                file.write_all(chunk)
                    .map_err(|e| TransportError::Io(e.kind()))?;
                progress(total);
                Ok(())
            },
        )?;
        file.flush().map_err(|e| TransportError::Io(e.kind()))?;
        Ok(())
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn non_null(handle: *mut c_void) -> Result<*mut c_void, TransportError> {
    if handle.is_null() {
        Err(win_error(Error::from_thread()))
    } else {
        Ok(handle)
    }
}

fn win_error(error: Error) -> TransportError {
    error_from_win32(error.code().0 as u32 & 0xFFFF)
}

fn query_u32(request: *mut c_void, info: u32) -> Result<Option<u32>, TransportError> {
    let mut value = 0u32;
    let mut length = std::mem::size_of::<u32>() as u32;
    let result = unsafe {
        WinHttpQueryHeaders(
            request,
            info,
            PCWSTR::null(),
            Some((&mut value as *mut u32).cast()),
            &mut length,
            std::ptr::null_mut(),
        )
    };
    match result {
        Ok(()) => Ok(Some(value)),
        Err(error) if error.code().0 as u32 & 0xFFFF == ERROR_WINHTTP_HEADER_NOT_FOUND => Ok(None),
        Err(error) => Err(win_error(error)),
    }
}
