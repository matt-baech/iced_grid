//! The [`Grid`] widget.
//!
//! It draws text and quads straight to the renderer, and only for the rows
//! and columns in view, so a hundred thousand rows scroll as smoothly as
//! ten. There are no child widgets.

use crate::state::{Align, Column, State, Target};
use crate::style::{Catalog, Status, Style, StyleFn};

use iced_core::alignment;
use iced_core::clipboard::{self, Clipboard};
use iced_core::keyboard::{self, key};
use iced_core::layout::{self, Layout};
use iced_core::mouse;
use iced_core::renderer;
use iced_core::text::{self, LineHeight, Shaping, Text, Wrapping};
use iced_core::widget::{self, Widget};
use iced_core::{Border, Element, Event, Font, Length, Pixels, Point, Rectangle, Shell, Size, Vector};

/// Thickness of the scrollbars.
const SCROLLBAR: f32 = 8.0;
/// Space between a cell's border and its text.
const PADDING: f32 = 8.0;
const MIN_WIDTH: f32 = 32.0;
/// Widest a column gets when sized to its content automatically.
const MAX_AUTO_WIDTH: f32 = 400.0;
/// How close to a header border the pointer must be to resize the column.
const GRAB: f32 = 4.0;
/// Rows looked at when sizing columns to their content.
const SAMPLE_ROWS: usize = 1000;
/// Longest value drawn in a cell; the rest is clipped anyway.
const MAX_CELL_CHARS: usize = 300;

/// How a value is drawn; see [`Grid::format`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Format {
    /// Draw the value in the style's `dim_text` colour.
    pub dim: bool,
    /// Draw the value in italics.
    pub italic: bool,
}

type FormatFn<'a> = Box<dyn Fn(&str) -> Format + 'a>;

/// A scrollable grid of text cells with a header row and row numbers.
///
/// Click, drag or Shift+click to select cells; click a header or a row
/// number to select columns or rows. Ctrl+C copies the selection as
/// tab-separated text (Ctrl+Shift+C with headers), arrows move the
/// selection, and dragging a header border resizes a column (double-click
/// it to fit the content).
///
/// # Example
/// ```no_run
/// use iced_grid::{Column, State, grid};
///
/// struct App {
///     columns: Vec<Column>,
///     rows: Vec<Vec<String>>,
///     grid: State,
/// }
///
/// fn view<Message: 'static>(app: &App) -> iced::Element<'_, Message> {
///     grid(&app.grid, &app.columns, &app.rows).into()
/// }
/// ```
pub struct Grid<'a, Theme = iced_core::Theme>
where
    Theme: Catalog,
{
    state: &'a State,
    columns: &'a [Column],
    rows: &'a [Vec<String>],
    width: Length,
    height: Length,
    text_size: Option<Pixels>,
    font: Option<Font>,
    header_font: Option<Font>,
    row_height: Option<f32>,
    format: Option<FormatFn<'a>>,
    class: Theme::Class<'a>,
}

/// Creates a [`Grid`] showing `rows` under `columns`.
pub fn grid<'a, Theme: Catalog>(state: &'a State, columns: &'a [Column], rows: &'a [Vec<String>]) -> Grid<'a, Theme> {
    Grid {
        state,
        columns,
        rows,
        width: Length::Fill,
        height: Length::Fill,
        text_size: None,
        font: None,
        header_font: None,
        row_height: None,
        format: None,
        class: <Theme as Catalog>::default(),
    }
}

impl<'a, Theme: Catalog> Grid<'a, Theme> {
    /// Sets the width; [`Length::Fill`] by default.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height; [`Length::Fill`] by default.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// The text size of cells and headers.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// The font of the cells. Column widths are estimated for a monospaced
    /// font.
    pub fn font(mut self, font: impl Into<Font>) -> Self {
        self.font = Some(font.into());
        self
    }

    /// The font of the column titles; the cell font by default.
    pub fn header_font(mut self, font: impl Into<Font>) -> Self {
        self.header_font = Some(font.into());
        self
    }

    /// The height of a row; by default 1.9 × the text size.
    pub fn row_height(mut self, height: f32) -> Self {
        self.row_height = Some(height);
        self
    }

    /// Picks how each value is drawn, e.g. NULL dimmed and in italics.
    pub fn format(mut self, format: impl Fn(&str) -> Format + 'a) -> Self {
        self.format = Some(Box::new(format));
        self
    }

    /// Sets the style; see [`style::from_palette`](crate::style::from_palette)
    /// for a starting point.
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    fn size_of_text<R: text::Renderer>(&self, renderer: &R) -> f32 {
        self.text_size.unwrap_or_else(|| renderer.default_size()).0
    }

    /// Sizes the columns to their content the first time they are shown.
    fn ensure_widths(&self, text_size: f32) {
        let mut widths = self.state.widths.borrow_mut();
        if widths.len() != self.columns.len() {
            *widths = (0..self.columns.len()).map(|c| self.fit(c, text_size, SAMPLE_ROWS)).collect();
        }
    }

    /// The width that shows a column's title and values, estimated from the
    /// number of characters in the first `sample` rows.
    fn fit(&self, col: usize, text_size: f32, sample: usize) -> f32 {
        let chars = |s: &str| s.chars().take(MAX_CELL_CHARS).count() as f32;
        let values = self.rows.iter().take(sample).filter_map(|r| r.get(col)).map(|v| chars(v)).fold(0.0, f32::max);
        let title = chars(&self.columns[col].title) * text_size * 0.62;
        (title.max(values * text_size * 0.6) + 2.0 * PADDING).clamp(MIN_WIDTH, MAX_AUTO_WIDTH).ceil()
    }

    fn geometry(&self, bounds: Rectangle, text_size: f32) -> Geometry {
        let row_height = self.row_height.unwrap_or((text_size * 1.9).round());
        let digits = self.rows.len().max(1).ilog10() as f32 + 1.0;
        let gutter = (digits * text_size * 0.6 + 2.0 * PADDING).ceil();
        let widths = self.state.widths.borrow().clone();
        let offsets: Vec<f32> = widths.iter().scan(0.0, |x, w| Some(std::mem::replace(x, *x + w))).collect();
        let content = Size::new(widths.iter().sum(), self.rows.len() as f32 * row_height);
        let body = Rectangle {
            x: bounds.x + gutter,
            y: bounds.y + row_height,
            width: (bounds.width - gutter - SCROLLBAR).max(0.0),
            height: (bounds.height - row_height - SCROLLBAR).max(0.0),
        };
        let max_scroll = Vector::new((content.width - body.width).max(0.0), (content.height - body.height).max(0.0));
        Geometry { bounds, body, row_height, gutter, widths, offsets, content, max_scroll }
    }
}

/// Where everything is, worked out from the bounds on each event and draw.
struct Geometry {
    bounds: Rectangle,
    /// The cells' viewport, between the header, row numbers and scrollbars.
    body: Rectangle,
    row_height: f32,
    gutter: f32,
    widths: Vec<f32>,
    /// Each column's left edge in content coordinates.
    offsets: Vec<f32>,
    content: Size,
    max_scroll: Vector,
}

impl Geometry {
    fn clamp(&self, scroll: Vector) -> Vector {
        Vector::new(scroll.x.clamp(0.0, self.max_scroll.x), scroll.y.clamp(0.0, self.max_scroll.y))
    }

    /// The column at a content x coordinate.
    fn column_at(&self, x: f32) -> Option<usize> {
        let col = self.offsets.partition_point(|&o| o <= x).checked_sub(1)?;
        (x < self.offsets[col] + self.widths[col]).then_some(col)
    }

    fn row_at(&self, y: f32, rows: usize) -> Option<usize> {
        let row = (y / self.row_height).floor();
        (row >= 0.0 && (row as usize) < rows).then_some(row as usize)
    }

    fn hit(&self, p: Point, scroll: Vector, rows: usize) -> Hit {
        let body = self.body;
        if !self.bounds.contains(p) {
            return Hit::Outside;
        }
        let (x, y) = (p.x - body.x + scroll.x, p.y - body.y + scroll.y);
        if p.x >= body.x + body.width && p.y >= body.y && p.y < body.y + body.height {
            return Hit::VerticalBar;
        }
        if p.y >= body.y + body.height {
            return if p.x >= body.x && p.x < body.x + body.width { Hit::HorizontalBar } else { Hit::Nothing };
        }
        if p.y < body.y {
            if p.x < body.x {
                return Hit::Corner;
            }
            let border = self.offsets.iter().zip(&self.widths).position(|(o, w)| (x - (o + w)).abs() <= GRAB);
            return match (border, self.column_at(x)) {
                (Some(col), _) => Hit::Border(col),
                (None, Some(col)) => Hit::Header(col),
                (None, None) => Hit::Nothing,
            };
        }
        match (p.x < body.x, self.row_at(y, rows), self.column_at(x)) {
            (true, Some(row), _) => Hit::RowNumber(row),
            (false, Some(row), Some(col)) => Hit::Cell(row, col),
            _ => Hit::Nothing,
        }
    }

    fn vertical_track(&self) -> Rectangle {
        Rectangle { x: self.body.x + self.body.width, width: SCROLLBAR, ..self.body }
    }

    fn horizontal_track(&self) -> Rectangle {
        Rectangle { y: self.body.y + self.body.height, height: SCROLLBAR, ..self.body }
    }

    /// The thumb's start and length along a track.
    fn thumb(track: f32, view: f32, content: f32, offset: f32, max: f32) -> (f32, f32) {
        let length = (track * view / content.max(1.0)).clamp(20.0_f32.min(track), track);
        let start = if max > 0.0 { (track - length) * offset / max } else { 0.0 };
        (start, length)
    }

    fn vertical_thumb(&self, scroll: Vector) -> Rectangle {
        let track = self.vertical_track();
        let (start, length) =
            Self::thumb(track.height, self.body.height, self.content.height, scroll.y, self.max_scroll.y);
        Rectangle { y: track.y + start, height: length, ..track }
    }

    fn horizontal_thumb(&self, scroll: Vector) -> Rectangle {
        let track = self.horizontal_track();
        let (start, length) =
            Self::thumb(track.width, self.body.width, self.content.width, scroll.x, self.max_scroll.x);
        Rectangle { x: track.x + start, width: length, ..track }
    }

    /// The scroll that keeps a cell in view.
    fn reveal(&self, scroll: Vector, row: usize, col: usize) -> Vector {
        let top = row as f32 * self.row_height;
        let y = scroll.y.clamp((top + self.row_height - self.body.height).max(0.0), top);
        let (left, width) =
            (self.offsets.get(col).copied().unwrap_or(0.0), self.widths.get(col).copied().unwrap_or(0.0));
        let x = scroll.x.clamp((left + width - self.body.width).max(0.0), left.max(0.0));
        self.clamp(Vector::new(x, y))
    }

    fn rows_per_page(&self) -> isize {
        ((self.body.height / self.row_height).floor() as isize).max(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Hit {
    Corner,
    Header(usize),
    /// The right border of a column's header.
    Border(usize),
    RowNumber(usize),
    Cell(usize, usize),
    VerticalBar,
    HorizontalBar,
    Nothing,
    Outside,
}

/// The widget's own transient state.
#[derive(Debug, Default)]
struct Interaction {
    focused: bool,
    drag: Option<Drag>,
    last_click: Option<mouse::Click>,
    modifiers: keyboard::Modifiers,
}

#[derive(Debug, Clone, Copy)]
enum Drag {
    /// Selecting cells, rows or columns, by the kind of the first click.
    Select(Kind),
    Resize {
        col: usize,
        origin: f32,
        width: f32,
    },
    /// Dragging a scrollbar thumb, `grab` pixels from its start.
    Vertical {
        grab: f32,
    },
    Horizontal {
        grab: f32,
    },
}

#[derive(Debug, Clone, Copy)]
enum Kind {
    Cells,
    Rows,
    Columns,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for Grid<'_, Theme>
where
    Theme: Catalog,
    Renderer: text::Renderer<Font = Font>,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<Interaction>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(Interaction::default())
    }

    fn size(&self) -> Size<Length> {
        Size { width: self.width, height: self.height }
    }

    fn layout(&mut self, _tree: &mut widget::Tree, renderer: &Renderer, limits: &layout::Limits) -> layout::Node {
        self.ensure_widths(self.size_of_text(renderer));
        layout::Node::new(limits.resolve(self.width, self.height, Size::ZERO))
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let interaction = tree.state.downcast_mut::<Interaction>();
        let text_size = self.size_of_text(renderer);
        let g = self.geometry(layout.bounds(), text_size);
        let state = self.state;
        let scroll = g.clamp(state.scroll.get());
        let rows = self.rows.len();
        let size = (rows, self.columns.len());
        let mut changed = true;

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(g.bounds) => {
                let (x, y) = match *delta {
                    mouse::ScrollDelta::Lines { x, y } => (x * text_size * 3.0, y * g.row_height * 3.0),
                    mouse::ScrollDelta::Pixels { x, y } => (x, y),
                };
                let (x, y) = if interaction.modifiers.shift() && x == 0.0 { (y, 0.0) } else { (x, y) };
                state.scroll.set(g.clamp(scroll - Vector::new(x, y)));
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let Some(p) = cursor.position_over(g.bounds) else {
                    changed = std::mem::take(&mut interaction.focused);
                    if changed {
                        shell.request_redraw();
                    }
                    return;
                };
                interaction.focused = true;
                let click = mouse::Click::new(p, mouse::Button::Left, interaction.last_click);
                interaction.last_click = Some(click);
                let extend = interaction.modifiers.shift();
                interaction.drag = match g.hit(p, scroll, rows) {
                    Hit::Border(col) if click.kind() == mouse::click::Kind::Double => {
                        let width = self.fit(col, text_size, usize::MAX);
                        state.widths.borrow_mut()[col] = width;
                        None
                    }
                    Hit::Border(col) => Some(Drag::Resize { col, origin: p.x, width: g.widths[col] }),
                    Hit::Header(col) => {
                        state.select(Target::Column(col), extend);
                        Some(Drag::Select(Kind::Columns))
                    }
                    Hit::RowNumber(row) => {
                        state.select(Target::Row(row), extend);
                        Some(Drag::Select(Kind::Rows))
                    }
                    Hit::Cell(row, col) => {
                        state.select(Target::Cell { row, col }, extend);
                        Some(Drag::Select(Kind::Cells))
                    }
                    Hit::Corner => {
                        state.select(Target::All, false);
                        None
                    }
                    Hit::VerticalBar if g.max_scroll.y > 0.0 => {
                        let thumb = g.vertical_thumb(scroll);
                        let grab = if thumb.contains(p) { p.y - thumb.y } else { thumb.height / 2.0 };
                        let drag = Drag::Vertical { grab };
                        drag_scrollbar(&g, state, drag, p);
                        Some(drag)
                    }
                    Hit::HorizontalBar if g.max_scroll.x > 0.0 => {
                        let thumb = g.horizontal_thumb(scroll);
                        let grab = if thumb.contains(p) { p.x - thumb.x } else { thumb.width / 2.0 };
                        let drag = Drag::Horizontal { grab };
                        drag_scrollbar(&g, state, drag, p);
                        Some(drag)
                    }
                    _ => None,
                };
                shell.capture_event();
            }
            // Right-click outside the selection selects that cell first, so a
            // context menu acts on what was clicked. Not captured, so the
            // menu still opens.
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let Some(p) = cursor.position_over(g.bounds) else { return };
                let target = match g.hit(p, scroll, rows) {
                    Hit::Cell(row, col) => (row, col, Target::Cell { row, col }),
                    Hit::RowNumber(row) => (row, 0, Target::Row(row)),
                    Hit::Header(col) if rows > 0 => (0, col, Target::Column(col)),
                    _ => return,
                };
                interaction.focused = true;
                if !state.selection().is_some_and(|s| s.contains(target.0, target.1, rows, size.1)) {
                    state.select(target.2, false);
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => match interaction.drag {
                Some(Drag::Resize { col, origin, width }) => {
                    if let Some(w) = state.widths.borrow_mut().get_mut(col) {
                        *w = (width + position.x - origin).max(MIN_WIDTH);
                    }
                }
                Some(Drag::Select(kind)) => {
                    let p = *position;
                    let body = g.body;
                    // Past an edge, scroll that way.
                    let step = Vector::new(
                        edge(p.x, body.x, body.x + body.width) * text_size * 2.0,
                        edge(p.y, body.y, body.y + body.height) * g.row_height,
                    );
                    let scroll = g.clamp(scroll + step);
                    state.scroll.set(scroll);
                    let inside = Point::new(
                        p.x.clamp(body.x, body.x + body.width - 1.0),
                        p.y.clamp(body.y, body.y + body.height - 1.0),
                    );
                    let row = g.row_at(inside.y - body.y + scroll.y, rows).unwrap_or(rows.saturating_sub(1));
                    let col = g.column_at(inside.x - body.x + scroll.x);
                    let target = match (kind, col) {
                        (Kind::Rows, _) => Some(Target::Row(row)),
                        (Kind::Columns, Some(col)) => Some(Target::Column(col)),
                        (Kind::Cells, Some(col)) if rows > 0 => Some(Target::Cell { row, col }),
                        _ => None,
                    };
                    if let Some(target) = target {
                        state.select(target, true);
                    }
                }
                Some(drag) => drag_scrollbar(&g, state, drag, *position),
                None => changed = false,
            },
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                changed = interaction.drag.take().is_some();
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                interaction.modifiers = *modifiers;
                changed = false;
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, physical_key, .. })
                if interaction.focused =>
            {
                let extend = modifiers.shift();
                let page = g.rows_per_page();
                let (far_up, far_down) = (isize::MIN, isize::MAX);
                let movement = match key.as_ref() {
                    keyboard::Key::Named(key::Named::ArrowUp) => Some((-1, 0)),
                    keyboard::Key::Named(key::Named::ArrowDown) => Some((1, 0)),
                    keyboard::Key::Named(key::Named::ArrowLeft) => Some((0, -1)),
                    keyboard::Key::Named(key::Named::ArrowRight) => Some((0, 1)),
                    keyboard::Key::Named(key::Named::PageUp) => Some((-page, 0)),
                    keyboard::Key::Named(key::Named::PageDown) => Some((page, 0)),
                    keyboard::Key::Named(key::Named::Home) if modifiers.command() => Some((far_up, far_up)),
                    keyboard::Key::Named(key::Named::End) if modifiers.command() => Some((far_down, far_down)),
                    keyboard::Key::Named(key::Named::Home) => Some((0, far_up)),
                    keyboard::Key::Named(key::Named::End) => Some((0, far_down)),
                    _ => None,
                };
                if let Some((down, right)) = movement {
                    state.move_focus(down, right, extend, size);
                    if let Some(selection) = state.selection() {
                        let (row, col) = selection.focus();
                        state.scroll.set(g.reveal(scroll, row, col));
                    }
                    shell.capture_event();
                } else if modifiers.command() {
                    match key.to_latin(*physical_key) {
                        Some('a') => state.select(Target::All, false),
                        Some('c') => {
                            if let Some(text) = state.selected_text(self.columns, self.rows, modifiers.shift()) {
                                clipboard.write(clipboard::Kind::Standard, text);
                            }
                            changed = false;
                        }
                        _ => return,
                    }
                    shell.capture_event();
                } else {
                    return;
                }
            }
            _ => changed = false,
        }
        if changed {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let interaction = tree.state.downcast_ref::<Interaction>();
        let text_size = self.size_of_text(renderer);
        let g = self.geometry(layout.bounds(), text_size);
        let style = theme.style(&self.class, if interaction.focused { Status::Focused } else { Status::Active });
        let scroll = g.clamp(self.state.scroll.get());
        let font = self.font.unwrap_or_else(|| renderer.default_font());
        let header_font = self.header_font.unwrap_or(font);
        let (body, row_height) = (g.body, g.row_height);
        let (rows, cols) = (self.rows.len(), self.columns.len());

        let first = ((scroll.y / row_height).floor() as usize).min(rows);
        let last = (((scroll.y + body.height) / row_height).ceil() as usize).min(rows);
        let visible: Vec<usize> = (0..cols)
            .filter(|&c| g.offsets[c] < scroll.x + body.width && g.offsets[c] + g.widths[c] > scroll.x)
            .collect();
        let selection = self.state.selection().filter(|_| rows > 0 && cols > 0);
        let ranges = selection.map(|s| s.ranges(rows, cols));
        let column_x = |c: usize| body.x + g.offsets[c] - scroll.x;
        let row_y = |r: usize| body.y + r as f32 * row_height - scroll.y;
        let filled_width = (g.content.width - scroll.x).min(body.width);
        let filled_height = (g.content.height - scroll.y).min(body.height);

        fill(renderer, g.bounds, style.background);

        // Cells.
        renderer.with_layer(body, |r| {
            for row in first..last {
                let y = row_y(row);
                for &col in &visible {
                    let cell = Rectangle::new(Point::new(column_x(col), y), Size::new(g.widths[col], row_height));
                    if ranges.as_ref().is_some_and(|(rs, cs)| rs.contains(&row) && cs.contains(&col)) {
                        fill(r, cell, style.selection);
                    }
                    let value = self.rows[row].get(col).map_or("", String::as_str);
                    let format = self.format.as_ref().map(|f| f(value)).unwrap_or_default();
                    let cell_font =
                        if format.italic { Font { style: iced_core::font::Style::Italic, ..font } } else { font };
                    let color = if format.dim { style.dim_text } else { style.text };
                    draw_text(r, one_line(value), cell, self.columns[col].align, cell_font, text_size, color);
                }
                fill(
                    r,
                    Rectangle::new(Point::new(body.x, y + row_height - 1.0), Size::new(filled_width, 1.0)),
                    style.lines,
                );
            }
            for &col in &visible {
                let x = column_x(col) + g.widths[col] - 1.0;
                fill(r, Rectangle::new(Point::new(x, body.y), Size::new(1.0, filled_height)), style.lines);
            }
            if let Some(selection) = selection {
                let (row, col) = selection.focus();
                let cell = Rectangle::new(Point::new(column_x(col), row_y(row)), Size::new(g.widths[col], row_height));
                let border = Border { color: style.focus, width: 1.0, radius: 0.0.into() };
                r.fill_quad(
                    renderer::Quad { bounds: cell, border, ..Default::default() },
                    iced_core::Color::TRANSPARENT,
                );
            }
        });

        // Header.
        let header = Rectangle::new(Point::new(body.x, g.bounds.y), Size::new(body.width, row_height));
        renderer.with_layer(header, |r| {
            fill(r, header, style.header_background);
            for &col in &visible {
                let cell = Rectangle::new(Point::new(column_x(col), header.y), Size::new(g.widths[col], row_height));
                if ranges.as_ref().is_some_and(|(rs, cs)| spans(rs, rows) && cs.contains(&col)) {
                    fill(r, cell, style.selection);
                }
                let title = &self.columns[col].title;
                draw_text(r, title.clone(), cell, Align::Start, header_font, text_size, style.header_text);
                let border = Rectangle::new(Point::new(cell.x + cell.width - 1.0, cell.y), Size::new(1.0, row_height));
                fill(r, border, style.lines);
            }
        });

        // Row numbers.
        let gutter = Rectangle::new(Point::new(g.bounds.x, body.y), Size::new(g.gutter, body.height));
        renderer.with_layer(gutter, |r| {
            fill(r, gutter, style.header_background);
            for row in first..last {
                let cell = Rectangle::new(Point::new(gutter.x, row_y(row)), Size::new(g.gutter, row_height));
                if ranges.as_ref().is_some_and(|(rs, cs)| spans(cs, cols) && rs.contains(&row)) {
                    fill(r, cell, style.selection);
                }
                draw_text(r, (row + 1).to_string(), cell, Align::End, font, text_size, style.row_number);
            }
        });

        // Corner, dividers and scrollbars.
        fill(renderer, Rectangle::new(g.bounds.position(), Size::new(g.gutter, row_height)), style.header_background);
        fill(
            renderer,
            Rectangle::new(Point::new(g.bounds.x, body.y - 1.0), Size::new(g.bounds.width, 1.0)),
            style.lines,
        );
        fill(
            renderer,
            Rectangle::new(Point::new(body.x - 1.0, g.bounds.y), Size::new(1.0, g.bounds.height)),
            style.lines,
        );
        let thumbs = [
            (g.max_scroll.y > 0.0).then(|| g.vertical_thumb(scroll).shrink(2.0)),
            (g.max_scroll.x > 0.0).then(|| g.horizontal_thumb(scroll).shrink(2.0)),
        ];
        for thumb in thumbs.into_iter().flatten() {
            let border = Border { radius: 3.0.into(), ..Border::default() };
            renderer.fill_quad(renderer::Quad { bounds: thumb, border, ..Default::default() }, style.scroller);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let interaction = tree.state.downcast_ref::<Interaction>();
        if let Some(Drag::Resize { .. }) = interaction.drag {
            return mouse::Interaction::ResizingHorizontally;
        }
        let g = self.geometry(layout.bounds(), self.size_of_text(renderer));
        match cursor.position().map(|p| g.hit(p, g.clamp(self.state.scroll.get()), self.rows.len())) {
            Some(Hit::Border(_)) => mouse::Interaction::ResizingHorizontally,
            _ => mouse::Interaction::default(),
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Grid<'a, Theme>> for Element<'a, Message, Theme, Renderer>
where
    Theme: Catalog + 'a,
    Renderer: text::Renderer<Font = Font> + 'a,
{
    fn from(grid: Grid<'a, Theme>) -> Self {
        Element::new(grid)
    }
}

/// Scrolls so a dragged scrollbar thumb follows the pointer.
fn drag_scrollbar(g: &Geometry, state: &State, drag: Drag, p: Point) {
    let scroll = g.clamp(state.scroll.get());
    let along = |pointer: f32, track: (f32, f32), thumb: f32, grab: f32, max: f32| {
        ((pointer - track.0 - grab) / (track.1 - thumb).max(1.0) * max).clamp(0.0, max)
    };
    let scroll = match drag {
        Drag::Vertical { grab } => {
            let (track, thumb) = (g.vertical_track(), g.vertical_thumb(scroll).height);
            Vector::new(scroll.x, along(p.y, (track.y, track.height), thumb, grab, g.max_scroll.y))
        }
        Drag::Horizontal { grab } => {
            let (track, thumb) = (g.horizontal_track(), g.horizontal_thumb(scroll).width);
            Vector::new(along(p.x, (track.x, track.width), thumb, grab, g.max_scroll.x), scroll.y)
        }
        _ => return,
    };
    state.scroll.set(scroll);
}

/// Whether a range covers all of `0..len`.
fn spans(range: &std::ops::RangeInclusive<usize>, len: usize) -> bool {
    *range.start() == 0 && *range.end() + 1 == len
}

/// -1 before `start`, 1 after `end`, 0 in between.
fn edge(v: f32, start: f32, end: f32) -> f32 {
    if v < start {
        -1.0
    } else if v > end {
        1.0
    } else {
        0.0
    }
}

fn fill(renderer: &mut impl renderer::Renderer, bounds: Rectangle, color: iced_core::Color) {
    renderer.fill_quad(renderer::Quad { bounds, ..Default::default() }, color);
}

/// Draws a value in a cell, vertically centred and clipped to the cell.
fn draw_text<R: text::Renderer<Font = Font>>(
    renderer: &mut R,
    content: String,
    cell: Rectangle,
    align: Align,
    font: Font,
    size: f32,
    color: iced_core::Color,
) {
    let inner = Rectangle { x: cell.x + PADDING, width: (cell.width - 2.0 * PADDING).max(0.0), ..cell };
    let (x, align_x) = match align {
        Align::Start => (inner.x, text::Alignment::Left),
        Align::End => (inner.x + inner.width, text::Alignment::Right),
    };
    let text = Text {
        content,
        bounds: Size::new(f32::INFINITY, cell.height),
        size: Pixels(size),
        line_height: LineHeight::default(),
        font,
        align_x,
        align_y: alignment::Vertical::Center,
        shaping: Shaping::Auto,
        wrapping: Wrapping::None,
    };
    renderer.fill_text(text, Point::new(x, cell.center_y()), color, inner);
}

/// A value on one line, cut short.
fn one_line(value: &str) -> String {
    value.chars().take(MAX_CELL_CHARS).map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c }).collect()
}
