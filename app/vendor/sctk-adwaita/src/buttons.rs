use log::{debug, warn};
use smithay_client_toolkit::reexports::csd_frame::{WindowManagerCapabilities, WindowState};
use tiny_skia::{FillRule, PathBuilder, PixmapMut, Rect, Stroke, Transform};

use crate::{theme::ColorMap, Location, SkiaResult};

/// The size of the button on the header bar in logical points.
const BUTTON_SIZE: f32 = 24.;
const BUTTON_MARGIN: f32 = 5.;
const BUTTON_SPACING: f32 = 13.;

#[derive(Debug)]
pub struct Buttons {
    // Sorted by order vec of buttons for the left and right sides
    buttons_left: Vec<Button>,
    buttons_right: Vec<Button>,
    layout_config: Option<(String, String)>,
    // Local patch (Drydock, PATCHES.md): compositor and window limits are kept separately.
    compositor_minimize: bool,
    compositor_maximize: bool,
    enabled_minimize: bool,
    enabled_maximize: bool,
    resizable: bool,
    last_arrange: Option<(u32, f32)>,
}

type ButtonLayout = (Vec<Button>, Vec<Button>);

impl Default for Buttons {
    fn default() -> Self {
        let (buttons_left, buttons_right) = Buttons::get_default_buttons_layout();

        Self {
            buttons_left,
            buttons_right,
            layout_config: None,
            compositor_minimize: true,
            compositor_maximize: true,
            enabled_minimize: true,
            enabled_maximize: true,
            resizable: true,
            last_arrange: None,
        }
    }
}

impl Buttons {
    pub fn new(layout_config: Option<(String, String)>) -> Self {
        match Buttons::parse_button_layout(layout_config.clone()) {
            Some((buttons_left, buttons_right)) => Self {
                buttons_left,
                buttons_right,
                layout_config,
                ..Self::default()
            },
            _ => Self::default(),
        }
    }

    /// Rearrange the buttons with the new width.
    pub fn arrange(&mut self, width: u32, margin_h: f32) {
        // Local patch (Drydock, PATCHES.md): remember the arguments so a filter change re-places
        // the remaining buttons instead of leaving stale offsets.
        self.last_arrange = Some((width, margin_h));

        let mut left_x = BUTTON_MARGIN + margin_h;
        let mut right_x = width as f32 - BUTTON_MARGIN;

        for button in &mut self.buttons_left {
            button.offset = left_x;

            // Add the button size plus spacing
            left_x += BUTTON_SIZE + BUTTON_SPACING;
        }

        for button in &mut self.buttons_right {
            // Subtract the button size.
            right_x -= BUTTON_SIZE;

            // Update it
            button.offset = right_x;

            // Subtract spacing for the next button.
            right_x -= BUTTON_SPACING;
        }
    }

    /// Find the coordinate of the button.
    pub fn find_button(&self, x: f64, y: f64) -> Location {
        let x = x as f32;
        let y = y as f32;
        let buttons = self.buttons_left.iter().chain(self.buttons_right.iter());

        for button in buttons {
            if button.contains(x, y) {
                return Location::Button(button.kind);
            }
        }

        Location::Head
    }

    pub fn update_wm_capabilities(&mut self, wm_capabilites: WindowManagerCapabilities) {
        let supports_maximize = wm_capabilites.contains(WindowManagerCapabilities::MAXIMIZE);
        let supports_minimize = wm_capabilites.contains(WindowManagerCapabilities::MINIMIZE);

        self.update_buttons(supports_maximize, supports_minimize);
    }

    pub fn update_buttons(&mut self, supports_maximize: bool, supports_minimize: bool) {
        // Local patch (Drydock, PATCHES.md): these are the compositor's limits; keep them apart
        // from what the window allows.
        self.compositor_maximize = supports_maximize;
        self.compositor_minimize = supports_minimize;
        self.refresh();
    }

    // Local patch (Drydock, PATCHES.md): re-filter the layout for all conditions and re-place the
    // remaining buttons with the last arrange arguments.
    fn refresh(&mut self) {
        let supports_minimize = self.compositor_minimize && self.enabled_minimize;
        let supports_maximize =
            self.compositor_maximize && self.enabled_maximize && self.resizable;

        let is_supported = |button: &Button| match button.kind {
            ButtonKind::Close => true,
            ButtonKind::Maximize => supports_maximize,
            ButtonKind::Minimize => supports_minimize,
        };

        let (buttons_left, buttons_right) =
            Buttons::parse_button_layout(self.layout_config.clone())
                .unwrap_or_else(Buttons::get_default_buttons_layout);

        self.buttons_left = buttons_left.into_iter().filter(|b| is_supported(b)).collect();
        self.buttons_right = buttons_right.into_iter().filter(|b| is_supported(b)).collect();

        if let Some((width, margin_h)) = self.last_arrange {
            self.arrange(width, margin_h);
        }
    }

    /// The window allows these buttons (Close is always allowed). A button is shown when the
    /// layout has it, the compositor allows it and the window allows it.
    /// Local patch (Drydock, PATCHES.md).
    pub fn set_enabled(&mut self, minimize: bool, maximize: bool) {
        self.enabled_minimize = minimize;
        self.enabled_maximize = maximize;
        self.refresh();
    }

    /// A window that is not resizable has no Maximize button (separate from `set_enabled`).
    /// Local patch (Drydock, PATCHES.md).
    pub fn set_resizable(&mut self, resizable: bool) {
        self.resizable = resizable;
        self.refresh();
    }

    /// True when a Maximize button may be shown or a title double-click may maximize:
    /// the compositor, the window and resizability all allow it.
    /// Local patch (Drydock, PATCHES.md).
    pub fn allows_maximize(&self) -> bool {
        self.compositor_maximize && self.enabled_maximize && self.resizable
    }

    /// The shown buttons of each side, left to right on screen: (left side, right side).
    /// Local patch (Drydock, PATCHES.md).
    pub fn layout(&self) -> (Vec<ButtonKind>, Vec<ButtonKind>) {
        (
            self.buttons_left.iter().map(|button| button.kind).collect(),
            self.buttons_right
                .iter()
                .rev()
                .map(|button| button.kind)
                .collect(),
        )
    }

    pub fn right_buttons_start_x(&self) -> Option<f32> {
        self.buttons_right.last().map(|button| button.x())
    }

    pub fn left_buttons_end_x(&self) -> Option<f32> {
        self.buttons_left.last().map(|button| button.end_x())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        start_x: f32,
        end_x: f32,
        scale: f32,
        colors: &ColorMap,
        mouse_location: Location,
        pixmap: &mut PixmapMut,
        resizable: bool,
        state: &WindowState,
    ) {
        let left_buttons_right_limit =
            self.right_buttons_start_x().unwrap_or(end_x).min(end_x) - BUTTON_SPACING;
        let buttons_left = self.buttons_left.iter().map(|x| (x, Side::Left));
        let buttons_right = self.buttons_right.iter().map(|x| (x, Side::Right));

        for (button, side) in buttons_left.chain(buttons_right) {
            let is_visible = button.x() > start_x && button.end_x() < end_x
                // If we have buttons from both sides and they overlap, prefer the right side
                && (side == Side::Right || button.end_x() < left_buttons_right_limit);

            if is_visible {
                button.draw(scale, colors, mouse_location, pixmap, resizable, state);
            }
        }
    }

    fn parse_button_layout(sides: Option<(String, String)>) -> Option<ButtonLayout> {
        let Some((left_side, right_side)) = sides else {
            return None;
        };

        let buttons_left = Buttons::parse_button_layout_side(left_side, Side::Left);
        let buttons_right = Buttons::parse_button_layout_side(right_side, Side::Right);

        if buttons_left.is_empty() && buttons_right.is_empty() {
            warn!("No valid buttons found in configuration");
            return None;
        }

        Some((buttons_left, buttons_right))
    }

    fn parse_button_layout_side(config: String, side: Side) -> Vec<Button> {
        let mut buttons: Vec<Button> = vec![];

        for button in config.split(',').take(3) {
            let button_kind = match button {
                "close" => ButtonKind::Close,
                "maximize" => ButtonKind::Maximize,
                "minimize" => ButtonKind::Minimize,
                "appmenu" => {
                    debug!("Ignoring \"appmenu\" button");
                    continue;
                }
                _ => {
                    warn!("Ignoring unknown button type: {button}");
                    continue;
                }
            };

            buttons.push(Button::new(button_kind));
        }

        // For the right side, we need to revert the order
        if side == Side::Right {
            buttons.into_iter().rev().collect()
        } else {
            buttons
        }
    }

    fn get_default_buttons_layout() -> ButtonLayout {
        (
            vec![],
            vec![
                Button::new(ButtonKind::Close),
                Button::new(ButtonKind::Maximize),
                Button::new(ButtonKind::Minimize),
            ],
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Button {
    /// The button offset into the header bar canvas.
    offset: f32,
    /// The kind of the button.
    kind: ButtonKind,
}

impl Button {
    pub fn new(kind: ButtonKind) -> Self {
        Self { offset: 0., kind }
    }

    pub fn radius(&self) -> f32 {
        BUTTON_SIZE / 2.0
    }

    pub fn x(&self) -> f32 {
        self.offset
    }

    pub fn center_x(&self) -> f32 {
        self.offset + self.radius()
    }

    pub fn center_y(&self) -> f32 {
        BUTTON_MARGIN + self.radius()
    }

    pub fn end_x(&self) -> f32 {
        self.offset + BUTTON_SIZE
    }

    fn contains(&self, x: f32, y: f32) -> bool {
        x > self.offset
            && x < self.offset + BUTTON_SIZE
            && y > BUTTON_MARGIN
            && y < BUTTON_MARGIN + BUTTON_SIZE
    }

    pub fn draw(
        &self,
        scale: f32,
        colors: &ColorMap,
        mouse_location: Location,
        pixmap: &mut PixmapMut,
        resizable: bool,
        state: &WindowState,
    ) -> SkiaResult {
        let button_bg = if mouse_location == Location::Button(self.kind)
            && (resizable || self.kind != ButtonKind::Maximize)
        {
            colors.button_hover_paint()
        } else {
            colors.button_idle_paint()
        };

        // Convert to pixels.
        let x = self.center_x() * scale;
        let y = self.center_y() * scale;
        let radius = self.radius() * scale;

        // Draw the button background.
        let circle = PathBuilder::from_circle(x, y, radius)?;
        pixmap.fill_path(
            &circle,
            &button_bg,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        let mut button_icon_paint = colors.button_icon_paint();
        // Do AA only for diagonal lines.
        button_icon_paint.anti_alias = self.kind == ButtonKind::Close;

        // Draw the icon.
        match self.kind {
            ButtonKind::Close => {
                let x_icon = {
                    let size = 3.5 * scale;
                    let mut pb = PathBuilder::new();

                    {
                        let sx = x - size;
                        let sy = y - size;
                        let ex = x + size;
                        let ey = y + size;

                        pb.move_to(sx, sy);
                        pb.line_to(ex, ey);
                        pb.close();
                    }

                    {
                        let sx = x - size;
                        let sy = y + size;
                        let ex = x + size;
                        let ey = y - size;

                        pb.move_to(sx, sy);
                        pb.line_to(ex, ey);
                        pb.close();
                    }

                    pb.finish()?
                };

                pixmap.stroke_path(
                    &x_icon,
                    &button_icon_paint,
                    &Stroke {
                        width: 1.1 * scale,
                        ..Default::default()
                    },
                    Transform::identity(),
                    None,
                );
            }
            ButtonKind::Maximize => {
                let path2 = {
                    let size = 8.0 * scale;
                    let hsize = size / 2.0;
                    let mut pb = PathBuilder::new();

                    let x = x - hsize;
                    let y = y - hsize;
                    if state.contains(WindowState::MAXIMIZED) {
                        let offset = 2.0 * scale;
                        if let Some(rect) =
                            Rect::from_xywh(x, y + offset, size - offset, size - offset)
                        {
                            pb.push_rect(rect);
                            pb.move_to(rect.left() + offset, rect.top() - offset);
                            pb.line_to(rect.right() + offset, rect.top() - offset);
                            pb.line_to(rect.right() + offset, rect.bottom() - offset + 0.5);
                        }
                    } else if let Some(rect) = Rect::from_xywh(x, y, size, size) {
                        pb.push_rect(rect);
                    }

                    pb.finish()?
                };

                pixmap.stroke_path(
                    &path2,
                    &button_icon_paint,
                    &Stroke {
                        width: 1.0 * scale,
                        ..Default::default()
                    },
                    Transform::identity(),
                    None,
                );
            }
            ButtonKind::Minimize => {
                let len = 8.0 * scale;
                let hlen = len / 2.0;
                pixmap.fill_rect(
                    Rect::from_xywh(x - hlen, y + hlen, len, scale)?,
                    &button_icon_paint,
                    Transform::identity(),
                    None,
                );
            }
        }

        Some(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ButtonKind {
    Close,
    Maximize,
    Minimize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}
