//! 100,000 rows in a grid. Scroll, select, copy with Ctrl+C, and drag a
//! header border to resize a column.
//!
//! cargo run --release --example grid

use iced::widget::{column, container, text};
use iced::{Element, Font, Length, Theme};
use iced_grid::{Align, Column, State, grid};

struct Demo {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
    grid: State,
}

impl Demo {
    fn new() -> Self {
        let columns = vec![
            Column::new("id").align(Align::End),
            Column::new("name"),
            Column::new("email"),
            Column::new("balance").align(Align::End),
            Column::new("note"),
        ];
        let rows = (1..=100_000)
            .map(|i| {
                vec![
                    i.to_string(),
                    format!("Customer {i}"),
                    format!("customer{i}@example.com"),
                    format!("{:.2}", (i * 7919 % 100_000) as f64 / 3.0),
                    if i % 7 == 0 { "NULL".into() } else { format!("Note for row {i}") },
                ]
            })
            .collect();
        Self { columns, rows, grid: State::new() }
    }

    fn update(&mut self, _message: ()) {}

    fn view(&self) -> Element<'_, ()> {
        let status = format!(
            "{} rows · click, drag or Shift+click to select · Ctrl+C copies · drag a header border to resize",
            self.rows.len()
        );
        let grid = grid(&self.grid, &self.columns, &self.rows)
            .font(Font::MONOSPACE)
            .format(|v| iced_grid::Format { dim: v == "NULL", italic: v == "NULL" });
        column![grid, container(text(status).size(13)).padding([4, 8]).width(Length::Fill)].into()
    }
}

fn main() -> iced::Result {
    iced::application(Demo::new, Demo::update, Demo::view).title("iced_grid").theme(|_: &Demo| Theme::Dark).run()
}
