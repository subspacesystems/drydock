# m10-brand-theme: one conflict in the spec

The spec says "open: both fills surface_raised, stroke border_control". In the Light theme
`surface-raised` is #FFFFFF, which is also `surface` (panel_fill) and window_fill. The
pre-existing test `tests/dock_titles.rs::the_title_fill_is_a_dialog_title_bar_and_differs_from_the_panel`
requires `widgets.open.weak_bg_fill` (used by `dock::header_fill` for panel title bars) to
differ from both panel_fill and window_fill. Both requirements cannot hold in Light.

Resolution taken: open-state fills use `p.bg` (light #EEF7FF, dark #091126) — a palette
colour, present in `allowed` in `the_visuals_use_no_stock_egui_colours`, and distinct from
panel_fill, window_fill and the hovered/active accent_soft in both themes. All 1672 drydock
tests pass. If the intent was surface_raised despite the title-bar test, the dock titles
spec is the one that needs revisiting.
