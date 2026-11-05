use tiny_skia::{Color, Pixmap};

#[cfg(feature = "ab_glyph")]
mod config;
#[cfg(feature = "ab_glyph")]
mod font_preference;

#[cfg(feature = "ab_glyph")]
mod skrifa_renderer;

#[cfg(not(feature = "ab_glyph"))]
mod dumb;

#[derive(Debug)]
pub struct TitleText {
    #[cfg(feature = "ab_glyph")]
    imp: skrifa_renderer::SkrifaTitleText,
    #[cfg(not(feature = "ab_glyph"))]
    imp: dumb::DumbTitleText,
}

impl TitleText {
    pub fn new(color: Color) -> Option<Self> {
        #[cfg(feature = "ab_glyph")]
        return Some(Self {
            imp: skrifa_renderer::SkrifaTitleText::new(color),
        });

        #[cfg(not(feature = "ab_glyph"))]
        {
            let _ = color;
            return None;
        }
    }

    pub fn update_scale(&mut self, scale: u32) {
        self.imp.update_scale(scale)
    }

    pub fn update_title(&mut self, title: impl Into<String>) {
        self.imp.update_title(title)
    }

    pub fn update_color(&mut self, color: Color) {
        self.imp.update_color(color)
    }

    pub fn pixmap(&self) -> Option<&Pixmap> {
        self.imp.pixmap()
    }
}
