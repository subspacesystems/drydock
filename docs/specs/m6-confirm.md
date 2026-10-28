# m6-confirm: ask before destroying settings, and ask for names in a dialog

Milestone M6 (UI), feedback round 4. Crate `modeler`: `src/dialog.rs`, `src/toolbars.rs`, `src/chrome.rs`, `src/app.rs`.

## Goal
The user's rule: anything destructive outside model editing gets a confirmation. Model edits are excluded because they can be undone. Also, the Customize Toolbars window should ask for a toolbar's name in a dialog of its own, not in text boxes on the window. So:
- Customize Toolbars: "New Toolbar..." and "Rename Toolbar..." open a name dialog. "Delete Toolbar..." and "Reset Toolbars..." open a confirmation. The inline name fields go.
- View > Toolbars > "Reset Toolbars..." and View > "Reset Panels..." ask before resetting.
- Labels that open a dialog end with "..." (the earlier rule from round 3).

## Already on main (skeleton)
- `dialog::confirm_dialog(ctx, id, title, message, action) -> Option<bool>` and `dialog::name_dialog(ctx, id, title, &mut String) -> Option<bool>`, `unimplemented!()`.
- `toolbars::CustomizePrompt` (`New { name }`, `Rename { index, name }`, `Delete { index }`, `Reset`) and `CustomizeState::prompt: Option<CustomizePrompt>`.
- `ToolbarsMenuEvent::Reset`. `chrome::MenuOutcome::reset_toolbars` is set when the menu reports it; the app ignores it so far.

## Rules
### Dialogs (`dialog.rs`)
- Both are an `egui::Modal` with the given `id`, about 360 pt wide. At the top is the title in strong text a little larger than the body text (the ids and the modal mean the window behind can't be clicked meanwhile). Buttons go through the existing `button_row`, so they sit at the bottom right with the gap above, have a border, and the default one is highlighted.
- `confirm_dialog`: the message, then the buttons `[action, "Cancel"]`. **Cancel is the default** (the safe answer, per the Windows guidelines the user asked us to follow), so Enter declines. `Some(true)` on the frame `action` is clicked. `Some(false)` on Cancel, Enter, Escape or a click outside (`response.should_close()`). `None` while waiting.
- `name_dialog`: one row with the label "Name:" (use `field_label`) and a single-line text box editing `name`, its accessibility label set with `labelled_by`. Then `["OK", "Cancel"]`, with OK the default and **enabled only while `name.trim()` isn't empty**. `Some(true)` on OK, or Enter with a non-blank name. `Some(false)` on Cancel, Escape or a click outside. `None` otherwise, including Enter or an OK click while the name is blank. Give the text box keyboard focus when the dialog first shows, so the user can type at once. The tests don't check focus; check it in the real window.

### Customize window (`toolbars::customize_panel`)
- Remove the "Toolbar name:" and "New toolbar name:" rows, and the "Rename toolbar", "Create toolbar", "Delete toolbar" and "Reset Toolbars" buttons. Remove `CustomizeState::new_name` and `CustomizeState::rename` (nothing else may use them).
- Below the two lists, one row of four ordinary buttons: "New Toolbar...", "Rename Toolbar...", "Delete Toolbar...", "Reset Toolbars...". Rename and Delete are disabled when there are no toolbars.
- A click only sets `state.prompt`. New → `New { name: "" }`. Rename → `Rename { index: selected, name: <its current name> }`. Delete → `Delete { index: selected }`. Reset → `Reset`. Nothing changes yet, and the function returns false for that frame.
- While `state.prompt` is `Some`, draw its dialog each frame with the id `"customize-prompt"`, editing the prompt's own `name` field:
  - New: `name_dialog` titled "New Toolbar". OK → `layout.add_toolbar(name)`, select the new toolbar, return true.
  - Rename: `name_dialog` titled "Rename Toolbar". OK → `layout.rename_toolbar(index, name)`, which already cleans the name and numbers a taken one. Return true when it changed.
  - Delete: `confirm_dialog` titled "Delete Toolbar", message `Delete the toolbar "<name>"?` (the name in straight double quotes), action "Delete". Confirm → remove it, clamp the selection, return true.
  - Reset: `confirm_dialog` titled "Reset Toolbars", a one-sentence message saying the toolbars go back to how they were installed and the user's own toolbars are deleted, action "Reset". Confirm → `layout.reset()`, clamp the selection, return true.
  - Any answer (yes or no) sets `state.prompt = None`. Cancel changes nothing.
- The toolbar picker and the two lists stay as they are.

### Menus and app
- `toolbars_menu`: the item becomes "Reset Toolbars..." and only returns `ToolbarsMenuEvent::Reset`; the layout is untouched.
- View menu: "Reset Panels" becomes "Reset Panels...". It still yields `Action::ResetPanels`.
- App: keep one pending app-level confirmation (e.g. `Option<AppConfirm>` with `ResetToolbars` and `ResetPanels`). `outcome.reset_toolbars` opens the first. `Effect::ResetPanels` opens the second instead of resetting. Draw it with `confirm_dialog`. For Reset Toolbars, use the title, message and action of the Customize window's reset. For Reset Panels, use title "Reset Panels", a one-sentence message that every panel goes back to its default place and size, and action "Reset". On confirm do what the code does today: reset and save the toolbars, or reset the dock, clear `shadows_preview` and save the panels. Treat this modal like the other dialogs for `dialog_open` (the viewport ignores input meanwhile).

## Notes
- A throwaway implementation that passes the tests took about 140 lines (`dialog.rs` 50, `toolbars.rs` 90 replacing about 60, chrome one label). It is not the app wiring, which you write.
- kittest's `get_by_label` and `query_by_label` match the whole label; the tests use their own exact-match `button` helper because "Delete" and "Delete Toolbar..." are both on screen.
- The unsaved-changes and file dialogs are out of scope.

## Acceptance
- `cargo test -p modeler --test confirm`: both dialogs (buttons, default, Enter, Escape, blank names), the menu Reset event, and the Customize window's four buttons with each dialog's confirm and cancel paths.
- Changed in the spec commit: `toolbar_customize.rs` loses the tests of the inline create, delete and reset and of the menu's immediate reset, `toolbar_customize_rename.rs` keeps only the layout-level rename tests, `toolbar_customize_lists.rs` drops `new_name`, and `menus_fit.rs` and `menus_items.rs` use the new "..." labels.
- All other tests unchanged and green, plus `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and all workspace tests.
- I'll check the two View menu confirmations and the name dialog's focus in the real window before merging.
