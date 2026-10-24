# iced_grid

A fast, virtualized data grid widget for [Iced](https://iced.rs) 0.14.

It draws only the cells in view, straight to the renderer, so large data sets (100,000+ rows) scroll smoothly.

- Sticky header row and row numbers
- Cell, row and column selection: click, drag, Shift+click, arrow keys, Ctrl+A
- Ctrl+C copies the selection as tab-separated text (Ctrl+Shift+C with headers), so it pastes into spreadsheets
- Columns sized to their content; drag a header border to resize, double-click to fit
- Right-aligned numeric columns and per-value formatting (e.g. dimmed `NULL`)
- `to_csv` for exporting

```rust
use iced_grid::{Column, State, grid};

// One State per data set, owned by your app.
let state = State::new();
let columns = vec![Column::new("id"), Column::new("name")];
let rows = vec![vec!["1".to_string(), "Ada".to_string()]];

let element: iced::Element<'_, Message> = grid(&state, &columns, &rows).into();
```

## License

MIT or Apache-2.0, at your option.
