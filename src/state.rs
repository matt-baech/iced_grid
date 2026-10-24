//! What a grid remembers between frames: scroll position, column widths and
//! the selection. The app owns it, one per data set, so switching between
//! grids keeps each one where it was.

use std::cell::{Cell, RefCell};
use std::ops::RangeInclusive;

use iced_core::Vector;

/// How a column's values line up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    /// For numbers.
    End,
}

/// A column header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub title: String,
    pub align: Align,
}

impl Column {
    pub fn new(title: impl Into<String>) -> Self {
        Self { title: title.into(), align: Align::Start }
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }
}

/// What a click or key selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Cell { row: usize, col: usize },
    Row(usize),
    Column(usize),
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Cells,
    Rows,
    Columns,
    All,
}

/// A rectangle of cells, from where the selection started (`anchor`) to
/// where it is now (`focus`). Whole rows or columns ignore the other axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    anchor: (usize, usize),
    focus: (usize, usize),
    mode: Mode,
}

impl Selection {
    /// The cell keyboard moves start from.
    pub fn focus(&self) -> (usize, usize) {
        self.focus
    }

    /// The selected rows and columns, given the grid's size (both non-zero).
    pub fn ranges(&self, rows: usize, cols: usize) -> (RangeInclusive<usize>, RangeInclusive<usize>) {
        let span = |a: usize, b: usize, len: usize| a.min(b).min(len - 1)..=a.max(b).min(len - 1);
        let all = |len: usize| 0..=len - 1;
        match self.mode {
            Mode::Cells => (span(self.anchor.0, self.focus.0, rows), span(self.anchor.1, self.focus.1, cols)),
            Mode::Rows => (span(self.anchor.0, self.focus.0, rows), all(cols)),
            Mode::Columns => (all(rows), span(self.anchor.1, self.focus.1, cols)),
            Mode::All => (all(rows), all(cols)),
        }
    }

    pub fn contains(&self, row: usize, col: usize, rows: usize, cols: usize) -> bool {
        let (r, c) = self.ranges(rows, cols);
        r.contains(&row) && c.contains(&col)
    }
}

/// The grid's state. Create a new one when the data changes.
#[derive(Debug, Default)]
pub struct State {
    pub(crate) scroll: Cell<Vector>,
    /// Empty until the first layout sizes the columns to their content.
    pub(crate) widths: RefCell<Vec<f32>>,
    pub(crate) selection: Cell<Option<Selection>>,
}

impl State {
    pub fn new() -> Self {
        Self::default()
    }

    /// The column widths, once the grid has been laid out.
    pub fn column_widths(&self) -> Vec<f32> {
        self.widths.borrow().clone()
    }

    /// How far the cells are scrolled, in pixels.
    pub fn scroll_offset(&self) -> Vector {
        self.scroll.get()
    }

    pub fn selection(&self) -> Option<Selection> {
        self.selection.get()
    }

    /// Selects a cell, row, column or everything; `extend` grows the
    /// current selection to it instead (Shift+click).
    pub fn select(&self, target: Target, extend: bool) {
        let current = self.selection.get();
        let (mode, at) = match target {
            Target::Cell { row, col } => (Mode::Cells, (row, col)),
            Target::Row(row) => (Mode::Rows, (row, 0)),
            Target::Column(col) => (Mode::Columns, (0, col)),
            Target::All => (Mode::All, (0, 0)),
        };
        let selection = match current {
            Some(s) if extend && s.mode == mode && mode != Mode::All => Selection { focus: at, ..s },
            _ => Selection { anchor: at, focus: at, mode },
        };
        self.selection.set(Some(selection));
    }

    /// Moves the focus cell by a number of rows and columns, keeping it in
    /// the grid; `extend` grows the selection (Shift+arrows).
    pub fn move_focus(&self, rows: isize, cols: isize, extend: bool, size: (usize, usize)) {
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        let from = self.selection.get().map_or((0, 0), |s| s.focus);
        let step = |at: usize, by: isize, len: usize| at.saturating_add_signed(by).min(len - 1);
        let to = (step(from.0, rows, size.0), step(from.1, cols, size.1));
        let selection = match self.selection.get() {
            Some(s) if extend && s.mode != Mode::All => Selection { focus: to, ..s },
            Some(_) if extend => Selection { anchor: (0, 0), focus: to, mode: Mode::Cells },
            _ => Selection { anchor: to, focus: to, mode: Mode::Cells },
        };
        self.selection.set(Some(selection));
    }

    pub fn clear_selection(&self) {
        self.selection.set(None);
    }

    /// The selected cells as tab-separated text, the way spreadsheets copy
    /// and paste. `headers` adds the column titles as the first line.
    pub fn selected_text(&self, columns: &[Column], rows: &[Vec<String>], headers: bool) -> Option<String> {
        if columns.is_empty() || (rows.is_empty() && !headers) {
            return None;
        }
        let selection = self.selection.get()?;
        let (row_range, col_range) = selection.ranges(rows.len().max(1), columns.len());
        let mut lines = Vec::new();
        if headers {
            lines.push(join(col_range.clone().map(|c| columns[c].title.as_str()), '\t'));
        }
        for row in rows.get(row_range).unwrap_or_default() {
            lines.push(join(col_range.clone().map(|c| row.get(c).map_or("", String::as_str)), '\t'));
        }
        Some(lines.join("\n"))
    }
}

/// The whole data set as CSV (RFC 4180), with a header line. Cells equal to
/// `null` are written empty.
pub fn to_csv(columns: &[Column], rows: &[Vec<String>], null: Option<&str>) -> String {
    let mut out = join(columns.iter().map(|c| c.title.as_str()), ',');
    for row in rows {
        out.push_str("\r\n");
        out.push_str(&join(row.iter().map(|v| if Some(v.as_str()) == null { "" } else { v }), ','));
    }
    out.push_str("\r\n");
    out
}

/// Joins values with `separator`, quoting those that contain it, a quote or
/// a line break.
fn join<'a>(values: impl Iterator<Item = &'a str>, separator: char) -> String {
    let quote = |v: &str| {
        if v.contains([separator, '"', '\n', '\r']) { format!("\"{}\"", v.replace('"', "\"\"")) } else { v.to_string() }
    };
    values.map(quote).collect::<Vec<_>>().join(&separator.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> (Vec<Column>, Vec<Vec<String>>) {
        let columns = vec![Column::new("id"), Column::new("name"), Column::new("note")];
        let rows = (0..4)
            .map(|i| vec![i.to_string(), format!("n{i}"), if i == 2 { "a\tb".into() } else { "x".into() }])
            .collect();
        (columns, rows)
    }

    #[test]
    fn copies_a_cell_rectangle() {
        let (columns, rows) = data();
        let state = State::new();
        assert_eq!(state.selected_text(&columns, &rows, false), None);

        state.select(Target::Cell { row: 1, col: 0 }, false);
        state.select(Target::Cell { row: 2, col: 1 }, true);
        assert_eq!(state.selected_text(&columns, &rows, false).unwrap(), "1\tn1\n2\tn2");
        assert_eq!(state.selected_text(&columns, &rows, true).unwrap(), "id\tname\n1\tn1\n2\tn2");
    }

    #[test]
    fn copies_rows_columns_and_all_with_quoting() {
        let (columns, rows) = data();
        let state = State::new();

        state.select(Target::Row(2), false);
        assert_eq!(state.selected_text(&columns, &rows, false).unwrap(), "2\tn2\t\"a\tb\"");

        state.select(Target::Column(1), false);
        state.select(Target::Column(0), true);
        assert_eq!(state.selected_text(&columns, &rows, false).unwrap(), "0\tn0\n1\tn1\n2\tn2\n3\tn3");

        state.select(Target::All, false);
        assert_eq!(state.selected_text(&columns, &rows, false).unwrap().lines().count(), 4);
    }

    #[test]
    fn shift_click_only_extends_the_same_kind() {
        let state = State::new();
        state.select(Target::Row(1), false);
        state.select(Target::Cell { row: 3, col: 2 }, true);
        assert_eq!(state.selection().unwrap().ranges(5, 5), (3..=3, 2..=2));
    }

    #[test]
    fn keyboard_moves_stay_in_the_grid() {
        let state = State::new();
        state.move_focus(-1, -1, false, (3, 2));
        assert_eq!(state.selection().unwrap().focus(), (0, 0));
        state.move_focus(10, 10, true, (3, 2));
        assert_eq!(state.selection().unwrap().ranges(3, 2), (0..=2, 0..=1));
        state.move_focus(0, -1, false, (3, 2));
        assert_eq!(state.selection().unwrap().ranges(3, 2), (2..=2, 0..=0));
    }

    #[test]
    fn exports_csv() {
        let columns = vec![Column::new("a"), Column::new("b,c")];
        let rows = vec![vec!["1".into(), "say \"hi\"".into()], vec!["NULL".into(), "x\ny".into()]];
        assert_eq!(to_csv(&columns, &rows, Some("NULL")), "a,\"b,c\"\r\n1,\"say \"\"hi\"\"\"\r\n,\"x\ny\"\r\n");
    }
}
