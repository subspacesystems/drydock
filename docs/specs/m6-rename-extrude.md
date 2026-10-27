# m6-rename-extrude

Tyrel's feedback: the Push/Pull tool is called **Extrude** in the UI.

## Change
- User-visible name of `Tool::PushPull` becomes "Extrude": `TOOL_BUTTONS` label (`chrome.rs`), the tool name shown in the status bar/tooltips (`app.rs`, the match near `Tool::PushPull => "Push/Pull"`), the Tools menu entry, the Customize window rows ("Remove Extrude"), and the undo step name passed to `push_pull` in `editor.rs` (both calls).
- The message "Can't push/pull that" becomes "Can't extrude that".
- Do not rename the enum variant, the module, the layout-file id (`"pushpull"` stays so saved layouts still load), or any test file/spec names.
- No "Push/Pull" string may remain in any user-visible text.

## Tests
`tests/chrome.rs`, `tests/toolbar_customize.rs` and `tests/menus.rs` are updated: the label list contains "Extrude", the Customize row is "Remove Extrude", the Tools menu has "Extrude" and no "Push/Pull". Existing tests otherwise unchanged and must stay green.
