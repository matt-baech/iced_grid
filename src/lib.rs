//! A fast, virtualized data grid for [Iced](https://iced.rs).
//!
//! [`Grid`] draws only the cells in view, straight to the renderer, so large
//! result sets scroll smoothly. It has a sticky header and row numbers, cell,
//! row and column selection, copying as tab-separated text, keyboard
//! navigation and resizable columns. [`to_csv`] exports the data.
//!
//! The app owns a [`State`] per data set (scroll position, column widths and
//! selection) and passes it in with the columns and rows on every view.

#![warn(missing_docs)]

mod grid;
mod state;

pub mod style;

pub use grid::{Format, Grid, grid};
pub use state::{Align, Column, Selection, State, Target, to_csv};
pub use style::{Catalog, Status, Style};
