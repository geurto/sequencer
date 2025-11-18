use iced::{color, Color, Font};
use iced_futures::core::font;

pub struct CustomTheme {
    pub primary_color: Color,
    pub primary_color_muted: Color,
    pub secondary_color: Color,
    pub secondary_color_muted: Color,
    pub primary_text_color: Color,
    pub secondary_text_color: Color,
    pub text_color: Color,
    pub surface_color: Color,
    pub overlay_color: Color,
    pub accent_color: Color,
    pub accent_color_muted: Color,
    pub header_font: Font,
    pub bold_font: Font,
    pub header_text_size: u16,
    pub text_size: u16,
}

impl Default for CustomTheme {
    fn default() -> Self {
        // All colors taken from Catppuccin Mocha
        Self {
            primary_color: color!(0xcba6f7),       // Mauve
            primary_color_muted: color!(0x65537b), // Muted mauve

            secondary_color: color!(0xf5c2e7), // Pink
            secondary_color_muted: color!(0x7a6173), // Muted pink

            primary_text_color: color!(0x89b4fa), // Blue
            secondary_text_color: color!(0xb4befe), // Lavender
            text_color: color!(0xcdd6f4),         // Text,

            surface_color: color!(0x1e1e2e), // Base
            overlay_color: color!(0x313244), // Surface0

            accent_color: color!(0xb4befe), // Lavender
            accent_color_muted: color!(0x5a5f7f), // Muted lavender

            header_font: Font {
                weight: font::Weight::Bold,
                stretch: font::Stretch::Expanded,
                ..Font::default()
            },
            bold_font: Font {
                weight: font::Weight::Bold,
                ..Font::default()
            },
            header_text_size: 14,
            text_size: 12,
        }
    }
}
