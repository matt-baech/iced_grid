# iced_grid

A fast, virtualized data grid widget for [Iced](https://iced.rs) 0.14.

It draws only the cells in view, straight to the renderer with no child widgets, so 100,000 rows scroll as smoothly as 100. It was built for the results pane of a SQL client, so it's tuned for reading, selecting and copying tabular text.

## Features

- **Virtualized drawing**: cost depends on what's on screen, not on the row count.
- **Sticky header row and row numbers**, with vertical and horizontal scrollbars.
- **Selection**: click a cell, drag or Shift+click for a block; click a row number or a header for whole rows or columns, the corner for everything. Arrow keys, PageUp/PageDown, Home/End and Ctrl+Home/End move it, with Shift to extend.
- **Copying**: Ctrl+C copies the selection as tab-separated text, so it pastes straight into spreadsheets; Ctrl+Shift+C adds the column titles. `State::selected_text` gives you the same text for your own menus.
- **Columns** start sized to their content. Drag a header border to resize, double-click it to fit.
- **Alignment and formatting**: right-align numeric columns, and dim or italicise values like `NULL` with `.format(...)`.
- **CSV export** with `to_csv` (RFC 4180 quoting).
- **Themed** through a `Catalog`, like Iced's own widgets, with `style::from_palette` as a starting point.

## Usage

```toml
[dependencies]
iced = "0.14"
iced_grid = "0.1"
```

```rust
use iced::Element;
use iced_grid::{Align, Column, State, grid};

struct App {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
    grid: State,
}

impl App {
    fn new() -> Self {
        Self {
            columns: vec![Column::new("id").align(Align::End), Column::new("name")],
            rows: vec![vec!["1".into(), "Ada".into()], vec!["2".into(), "Grace".into()]],
            grid: State::new(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        grid(&self.grid, &self.columns, &self.rows).into()
    }
}
```

Your app owns one `State` per data set. It holds the scroll position, column widths and selection, so switching between several grids keeps each one where it was. Create a new `State` when the data changes.

Try the 100,000-row demo:

```sh
cargo run --release --example grid
```

## Licence

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT licence ([LICENSE-MIT](LICENSE-MIT))

at your option.
