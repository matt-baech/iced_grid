//! Mouse and keyboard handling, driven headlessly.

use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::mouse::{self, ScrollDelta};
use iced::{Event, Point, Theme};
use iced_grid::{Column, State, grid};
use iced_test::Simulator;

/// Default text size 16: rows are 30px high, and 100 rows give a gutter of
/// three digits: 3 × 9.6 + 16 = 44.8 → 45px.
const ROW: f32 = 30.0;
const GUTTER: f32 = 45.0;

fn data() -> (Vec<Column>, Vec<Vec<String>>) {
    let columns = vec![Column::new("id"), Column::new("name")];
    let rows = (0..100).map(|i| vec![i.to_string(), format!("name {i}")]).collect();
    (columns, rows)
}

fn press(key: Key, modifiers: Modifiers) -> Vec<Event> {
    let physical = keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified);
    vec![Event::Keyboard(keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: physical,
        location: keyboard::Location::Standard,
        modifiers,
        repeat: false,
        text: None,
    })]
}

fn click() -> Vec<Event> {
    iced_test::simulator::click().collect()
}

fn modifiers(m: Modifiers) -> Vec<Event> {
    vec![Event::Keyboard(keyboard::Event::ModifiersChanged(m))]
}

/// Runs `steps` against a fresh grid; each step points somewhere and sends
/// events there.
fn run(state: &State, steps: Vec<(Point, Vec<Event>)>) {
    let (columns, rows) = data();
    let mut ui = Simulator::<(), Theme, iced::Renderer>::new(grid(state, &columns, &rows));
    for (point, events) in steps {
        ui.point_at(point);
        let _ = ui.simulate(events);
    }
}

fn cell(state: &State, row: usize, col: usize) -> Point {
    let widths = state.column_widths();
    let x = GUTTER + widths[..col].iter().sum::<f32>() + widths[col] / 2.0;
    Point::new(x, ROW + row as f32 * ROW + ROW / 2.0)
}

#[test]
fn click_and_shift_click_select_a_rectangle() {
    let state = State::new();
    run(&state, vec![]);
    let (a, b) = (cell(&state, 1, 0), cell(&state, 3, 1));
    let click = click();
    let shift_click = [modifiers(Modifiers::SHIFT), click.clone()].concat();
    run(&state, vec![(a, click), (b, shift_click)]);
    assert_eq!(state.selection().unwrap().ranges(100, 2), (1..=3, 0..=1));
}

#[test]
fn row_numbers_and_headers_select_rows_and_columns() {
    let state = State::new();
    let click = click();
    run(&state, vec![(Point::new(10.0, ROW * 2.5), click.clone())]);
    assert_eq!(state.selection().unwrap().ranges(100, 2), (1..=1, 0..=1));

    let header = Point::new(cell(&state, 0, 1).x, ROW / 2.0);
    run(&state, vec![(header, click)]);
    assert_eq!(state.selection().unwrap().ranges(100, 2), (0..=99, 1..=1));
}

#[test]
fn wheel_and_keys_scroll() {
    let state = State::new();
    let wheel = vec![Event::Mouse(mouse::Event::WheelScrolled { delta: ScrollDelta::Lines { x: 0.0, y: -2.0 } })];
    run(&state, vec![(Point::new(200.0, 200.0), wheel)]);
    assert_eq!(state.scroll_offset().y, 2.0 * 3.0 * ROW);

    // Ctrl+End selects the last cell and scrolls it into view.
    let state = State::new();
    let point = cell(&state_with_widths(), 0, 0);
    run(&state, vec![(point, click()), (point, press(Key::Named(Named::End), Modifiers::CTRL))]);
    assert_eq!(state.selection().unwrap().focus(), (99, 1));
    assert!(state.scroll_offset().y > 0.0);
}

#[test]
fn dragging_a_header_border_resizes_the_column() {
    let state = State::new();
    run(&state, vec![]);
    let before = state.column_widths()[0];
    let border = Point::new(GUTTER + before, ROW / 2.0);
    let press = vec![Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))];
    let to = Point::new(border.x + 40.0, border.y);
    let moved = vec![Event::Mouse(mouse::Event::CursorMoved { position: to })];
    let release = vec![Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))];
    run(&state, vec![(border, press), (to, [moved, release].concat())]);
    assert_eq!(state.column_widths()[0], before + 40.0);
}

/// A state that has been laid out once, for its column widths.
fn state_with_widths() -> State {
    let state = State::new();
    run(&state, vec![]);
    state
}
