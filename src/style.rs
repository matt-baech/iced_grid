//! The appearance of a [`Grid`](crate::Grid).

use iced_core::theme::{self, palette::Extended};
use iced_core::{Color, Theme};

/// Whether the grid has keyboard focus. The selection is drawn more
/// strongly while it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Active,
    Focused,
}

/// The appearance of a [`Grid`](crate::Grid).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub background: Color,
    pub text: Color,
    /// Values shown dimmed, such as NULL.
    pub dim_text: Color,
    pub header_background: Color,
    pub header_text: Color,
    /// The row-number column on the left.
    pub row_number: Color,
    pub lines: Color,
    pub selection: Color,
    /// The border around the focused cell.
    pub focus: Color,
    pub scroller: Color,
}

/// The theme catalog of a [`Grid`](crate::Grid).
pub trait Catalog: theme::Base {
    type Class<'a>;

    fn default<'a>() -> Self::Class<'a>;

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// A styling function for a [`Grid`](crate::Grid).
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// The default style: quiet lines, a slightly darker header, and the
/// primary colour for the selection.
pub fn default(theme: &Theme, status: Status) -> Style {
    from_palette(theme.extended_palette(), status)
}

/// Builds the default style from an extended palette, for apps with their own
/// theme type.
pub fn from_palette(palette: &Extended, status: Status) -> Style {
    let background = palette.background.base.color;
    let text = palette.background.base.text;
    let primary = palette.primary.base.color;
    Style {
        background,
        text,
        dim_text: text.scale_alpha(0.5),
        header_background: mix(background, text, 0.04),
        header_text: text,
        row_number: text.scale_alpha(0.45),
        lines: mix(background, text, 0.1),
        selection: primary.scale_alpha(if status == Status::Focused { 0.28 } else { 0.16 }),
        focus: primary,
        scroller: text.scale_alpha(0.25),
    }
}

fn mix(a: Color, b: Color, t: f32) -> Color {
    Color { r: a.r + (b.r - a.r) * t, g: a.g + (b.g - a.g) * t, b: a.b + (b.b - a.b) * t, a: a.a }
}
