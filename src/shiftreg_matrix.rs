use crate::hc595_cols::Hc595Cols;
use embassy_stm32::exti::ExtiInput;
use embassy_time::{Duration, Timer};
use rmk::{
    debounce::{DebounceState, DebouncerTrait, default_debouncer::DefaultDebouncer},
    event::{Event, KeyboardEvent},
    input_device::InputDevice,
    matrix::KeyState,
};

pub struct ShiftRegMatrix<'d, const ROW: usize, const COL: usize> {
    rows: [ExtiInput<'d>; ROW],
    cols: Hc595Cols<'d>,

    debouncer: DefaultDebouncer<ROW, COL>,
    key_state: [[KeyState; COL]; ROW],

    scan_pos: (usize, usize),

    settle: Duration,
    idle: Duration,
}

impl<'d, const ROW: usize, const COL: usize> ShiftRegMatrix<'d, ROW, COL> {
    pub fn new(rows: [ExtiInput<'d>; ROW], mut cols: Hc595Cols<'d>) -> Self {
        cols.unselect_all();
        Self {
            rows,
            cols,
            debouncer: DefaultDebouncer::new(),
            key_state: [[KeyState { pressed: false }; COL]; ROW],
            scan_pos: (0, 0),
            settle: Duration::from_micros(30),
            idle: Duration::from_micros(100),
        }
    }

    async fn scan_until_event(&mut self) -> Option<KeyboardEvent> {
        let (row_start, col_start) = self.scan_pos;

        for c in col_start..COL {
            self.cols.select_col_active_low(c);
            Timer::after(self.settle).await;

            let r0 = if c == col_start { row_start } else { 0 };
            for r in r0..ROW {
                let pressed = self.rows[r].is_low();

                let ks = &mut self.key_state[r][c];
                let st = self.debouncer.detect_change_with_debounce(r, c, pressed, ks);
                if let DebounceState::Debounced = st {
                    ks.pressed = pressed;
                    self.cols.unselect_all();
                    self.scan_pos = (r, c);
                    return Some(KeyboardEvent::key(r as u8, c as u8, pressed));
                }
            }

            self.cols.unselect_all();
        }

        for c in 0..col_start {
            self.cols.select_col_active_low(c);
            Timer::after(self.settle).await;

            for r in 0..ROW {
                let pressed = self.rows[r].is_low();
                let ks = &mut self.key_state[r][c];
                let st = self.debouncer.detect_change_with_debounce(r, c, pressed, ks);
                if let DebounceState::Debounced = st {
                    ks.pressed = pressed;
                    self.cols.unselect_all();
                    self.scan_pos = (r, c);
                    return Some(KeyboardEvent::key(r as u8, c as u8, pressed));
                }
            }

            self.cols.unselect_all();
        }

        self.scan_pos = (0, 0);
        None
    }
}

impl<'d, const ROW: usize, const COL: usize> InputDevice for ShiftRegMatrix<'d, ROW, COL> {
    async fn read_event(&mut self) -> Event {
        loop {
            if let Some(ev) = self.scan_until_event().await {
                return Event::Key(ev);
            }
            Timer::after(self.idle).await;
        }
    }
}
