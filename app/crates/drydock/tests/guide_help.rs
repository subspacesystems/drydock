// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Subspace Systems

//! m8-guide-help: finding and opening the bundled guide, with a stub browser launcher.
//! See docs/specs/m8-guide-help.md.

use drydock::guide_help::{
    BrowserLauncher, GUIDE_INDEX, browser_command, find_guide, guide_candidates, open_guide,
};
use drydock::input::{Action, Modifiers, key};
use drydock::look::Os;
use drydock::viewstate::{Effect, ViewState};
use std::cell::RefCell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct Stub {
    opened: RefCell<Vec<PathBuf>>,
    fail: Option<&'static str>,
}

impl BrowserLauncher for Stub {
    fn open(&self, page: &Path) -> Result<(), String> {
        self.opened.borrow_mut().push(page.to_path_buf());
        match self.fail {
            Some(reason) => Err(reason.to_owned()),
            None => Ok(()),
        }
    }
}

#[test]
fn installed_layout_comes_first() {
    let c = guide_candidates(Some(Path::new("/opt/drydock")));
    assert_eq!(c[0], Path::new("/opt/drydock/guide").join(GUIDE_INDEX));
    assert!(c.len() >= 2, "a dev fallback follows");
}

#[test]
fn dev_fallback_is_the_guide_the_build_made() {
    // The build script builds docs/guide into $OUT_DIR/guide and exports it as DRYDOCK_DEV_GUIDE.
    let dev = PathBuf::from(env!("DRYDOCK_DEV_GUIDE"));
    let c = guide_candidates(None);
    assert_eq!(c, vec![dev.join(GUIDE_INDEX)]);
    let c = guide_candidates(Some(Path::new("/opt/drydock")));
    assert_eq!(c.len(), 2);
    assert_eq!(c[1], dev.join(GUIDE_INDEX));
    let text = c[1].to_string_lossy().replace('\\', "/");
    assert!(!text.contains("../../target"), "{text}");
}

#[test]
fn find_guide_takes_the_first_existing() {
    let c = vec![
        PathBuf::from("/a/index.html"),
        PathBuf::from("/b/index.html"),
        PathBuf::from("/c/index.html"),
    ];
    let found = find_guide(&c, |p| p != Path::new("/a/index.html"));
    assert_eq!(found, Some(PathBuf::from("/b/index.html")));
    assert_eq!(find_guide(&c, |_| false), None);
    assert_eq!(find_guide(&[], |_| true), None);
}

#[test]
fn browser_commands_per_os() {
    let page = Path::new("/g/index.html");
    let (prog, args) = browser_command(Os::Linux, page);
    assert_eq!(prog, "xdg-open");
    assert_eq!(args, vec![OsString::from("/g/index.html")]);
    let (prog, args) = browser_command(Os::Mac, page);
    assert_eq!(prog, "open");
    assert_eq!(args, vec![OsString::from("/g/index.html")]);
    let (prog, args) = browser_command(Os::Windows, Path::new("C:\\D\\guide\\index.html"));
    assert_eq!(prog, "cmd");
    assert_eq!(
        args,
        vec![
            OsString::from("/C"),
            OsString::from("start"),
            OsString::from(""),
            OsString::from("C:\\D\\guide\\index.html"),
        ]
    );
}

#[test]
fn opens_the_found_page() {
    let stub = Stub::default();
    let c = vec![
        PathBuf::from("/x/index.html"),
        PathBuf::from("/y/index.html"),
    ];
    let msg = open_guide(&c, |p| p == Path::new("/y/index.html"), &stub);
    assert_eq!(*stub.opened.borrow(), vec![PathBuf::from("/y/index.html")]);
    assert!(msg.contains("User Guide"), "{msg}");
    assert!(!msg.to_lowercase().contains("modeler"), "{msg}");
}

#[test]
fn missing_guide_gives_a_message_and_launches_nothing() {
    let stub = Stub::default();
    let c = vec![PathBuf::from("/x/index.html")];
    let msg = open_guide(&c, |_| false, &stub);
    assert!(stub.opened.borrow().is_empty());
    assert!(msg.contains("User Guide"), "{msg}");
    assert!(msg.contains("not found"), "{msg}");
}

#[test]
fn launcher_failure_is_reported() {
    let stub = Stub {
        fail: Some("no browser"),
        ..Stub::default()
    };
    let c = vec![PathBuf::from("/x/index.html")];
    let msg = open_guide(&c, |_| true, &stub);
    assert_eq!(stub.opened.borrow().len(), 1);
    assert!(msg.contains("no browser"), "{msg}");
    assert!(msg.contains("User Guide"), "{msg}");
}

#[test]
fn f1_is_the_guide_key() {
    let none = Modifiers::default();
    assert_eq!(key("F1", none), Some(Action::UserGuide));
    let shift = Modifiers {
        shift: true,
        ..none
    };
    assert_eq!(key("F1", shift), None);
}

#[test]
fn the_action_becomes_an_effect() {
    let mut view = ViewState::new();
    assert_eq!(
        view.apply(Action::UserGuide, (800.0, 600.0)),
        Effect::UserGuide
    );
}
