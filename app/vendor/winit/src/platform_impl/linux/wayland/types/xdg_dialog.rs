//! Local patch (Drydock, PATCHES.md): xdg-dialog-v1, which marks a dialog window as modal.

use sctk::reexports::client::globals::{BindError, GlobalList};
use sctk::reexports::client::{delegate_dispatch, Connection, Dispatch, Proxy, QueueHandle};
use sctk::reexports::protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use sctk::reexports::protocols::xdg::dialog::v1::client::xdg_dialog_v1::XdgDialogV1;
use sctk::reexports::protocols::xdg::dialog::v1::client::xdg_wm_dialog_v1::XdgWmDialogV1;

use sctk::globals::GlobalData;

use crate::platform_impl::wayland::state::WinitState;

/// The compositor's `xdg_wm_dialog_v1` global.
#[derive(Debug, Clone)]
pub struct XdgDialogState {
    manager: XdgWmDialogV1,
}

impl XdgDialogState {
    pub fn bind(
        globals: &GlobalList,
        queue_handle: &QueueHandle<WinitState>,
    ) -> Result<Self, BindError> {
        let manager = globals.bind(queue_handle, 1..=1, GlobalData)?;
        Ok(Self { manager })
    }

    /// A dialog object for `toplevel`, hinted modal. Only one per toplevel; destroy it before
    /// dropping the window, or leave it to become inert with the toplevel.
    pub fn modal_dialog(
        &self,
        toplevel: &XdgToplevel,
        queue_handle: &QueueHandle<WinitState>,
    ) -> XdgDialogV1 {
        let dialog = self.manager.get_xdg_dialog(toplevel, queue_handle, GlobalData);
        dialog.set_modal();
        dialog
    }
}

impl Dispatch<XdgWmDialogV1, GlobalData, WinitState> for XdgDialogState {
    fn event(
        _: &mut WinitState,
        _: &XdgWmDialogV1,
        _: <XdgWmDialogV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        unreachable!("no events defined for xdg_wm_dialog_v1");
    }
}

impl Dispatch<XdgDialogV1, GlobalData, WinitState> for XdgDialogState {
    fn event(
        _: &mut WinitState,
        _: &XdgDialogV1,
        _: <XdgDialogV1 as Proxy>::Event,
        _: &GlobalData,
        _: &Connection,
        _: &QueueHandle<WinitState>,
    ) {
        unreachable!("no events defined for xdg_dialog_v1");
    }
}

delegate_dispatch!(WinitState: [XdgWmDialogV1: GlobalData] => XdgDialogState);
delegate_dispatch!(WinitState: [XdgDialogV1: GlobalData] => XdgDialogState);
