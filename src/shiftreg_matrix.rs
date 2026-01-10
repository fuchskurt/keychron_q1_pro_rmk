use embassy_stm32::gpio::Input;
use embassy_time::{Duration, Timer};
use rmk::debounce::default_debouncer::DefaultDebouncer;
use rmk::debounce::{DebounceState, DebouncerTrait};
use rmk::event::{Event, KeyboardEvent};
use rmk::input_device::InputDevice;
use rmk::matrix::KeyState;

use crate::hc595_cols::Hc595Cols;

pub struct ShiftRegMatrix<'d, const ROW: usize, const COL: usize> {
    rows: [Input<'d>; ROW],
    cols: Hc595Cols<'d>,
    debouncer: DefaultDebouncer<ROW, COL>,
    states: [[KeyState; ROW]; COL],
    settle: Duration,
    idle: Duration,
}

struct ColGuard<'a, 'd>(&'a mut Hc595Cols<'d>);
impl<'a, 'd> Drop for ColGuard<'a, 'd> {
    fn drop(&mut self) {
        self.0.unselect_all();
    }
}

impl<'d, const ROW: usize, const COL: usize> ShiftRegMatrix<'d, ROW, COL> {
    pub fn new(rows: [Input<'d>; ROW], mut cols: Hc595Cols<'d>) -> Self {
        // Start with all columns unselected
        cols.unselect_all();

        Self {
            rows,
            cols,
            debouncer: DefaultDebouncer::new(),
            states: [[KeyState { pressed: false }; ROW]; COL],
            settle: Duration::from_micros(30),
            idle: Duration::from_micros(200),
        }
    }

    async fn scan_once(&mut self) -> Option<KeyboardEvent> {
        for col in 0..COL {
            // Select one column (active-low)
            self.cols.select_col_active_low(col);
            let _guard = ColGuard(&mut self.cols);
            Timer::after(self.settle).await;

            for row in 0..ROW {
                // Row is pulled-up, Pressed pulls it low
                let pressed = self.rows[row].is_low();
                let ks = &mut self.states[col][row];
                if let DebounceState::Debounced = self.debouncer.detect_change_with_debounce(row, col, pressed, ks) {
                    ks.pressed = pressed;
                    return Some(KeyboardEvent::key(row as u8, col as u8, pressed));
                }
            }
            Timer::after(self.settle).await;
        }

        None
    }
}

impl<'d, const ROW: usize, const COL: usize> InputDevice for ShiftRegMatrix<'d, ROW, COL> {
    async fn read_event(&mut self) -> Event {
        loop {
            if let Some(ev) = self.scan_once().await {
                return Event::Key(ev);
            }
            Timer::after(self.idle).await;
        }
    }
}
