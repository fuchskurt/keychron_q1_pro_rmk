use crate::hc595_cols::Hc595Cols;

use embassy_stm32::exti::ExtiInput;
use embassy_time::{Duration, Timer};
use rmk::{
    debounce::{default_debouncer::DefaultDebouncer, DebounceState, DebouncerTrait},
    event::{Event, KeyboardEvent},
    input_device::InputDevice,
    matrix::KeyState,
};

#[derive(Copy, Clone)]
struct ScanPos {
    row: usize,
    col: usize,
}

impl ScanPos {
    const fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }
}

struct KeyGrid<const ROW: usize, const COL: usize> {
    cells: [[KeyState; COL]; ROW],
}

impl<const ROW: usize, const COL: usize> KeyGrid<ROW, COL> {
    fn new() -> Self {
        Self {
            cells: core::array::from_fn(|_| {
                core::array::from_fn(|_| KeyState { pressed: false })
            }),
        }
    }

    #[inline]
    fn get_mut(&mut self, row: usize, col: usize) -> Option<&mut KeyState> {
        self.cells
            .get_mut(row)
            .and_then(|row_arr| row_arr.get_mut(col))
    }
}

pub struct ShiftRegMatrix<'d, const ROW: usize, const COL: usize> {
    rows: [ExtiInput<'d>; ROW],
    cols: Hc595Cols<'d>,

    debouncer: DefaultDebouncer<ROW, COL>,
    key_state: KeyGrid<ROW, COL>,

    scan_pos: ScanPos,

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
            key_state: KeyGrid::new(),
            scan_pos: ScanPos::new(0, 0),
            settle: Duration::from_micros(30),
            idle: Duration::from_micros(100),
        }
    }

    async fn scan_until_event(&mut self) -> Option<KeyboardEvent> {
        let rows: &[ExtiInput<'d>; ROW] = &self.rows;
        let settle: Duration = self.settle;

        let cols: &mut Hc595Cols<'d> = &mut self.cols;
        let debouncer: &mut DefaultDebouncer<ROW, COL> = &mut self.debouncer;
        let key_state: &mut KeyGrid<ROW, COL> = &mut self.key_state;
        let scan_pos: &mut ScanPos = &mut self.scan_pos;

        let start = *scan_pos;

        for c in (start.col..COL).chain(0..start.col) {
            cols.select_col_active_low(c);
            Timer::after(settle).await;

            let r_start = if c == start.col { start.row } else { 0 };

            for (r, row_pin) in rows.iter().enumerate().skip(r_start) {
                let pressed = row_pin.is_low();

                let Some(ks) = key_state.get_mut(r, c) else {
                    continue;
                };

                let st = debouncer.detect_change_with_debounce(r, c, pressed, ks);
                if let DebounceState::Debounced = st {
                    ks.pressed = pressed;

                    cols.unselect_all();
                    *scan_pos = ScanPos::new(r, c);

                    return Some(KeyboardEvent::key(r as u8, c as u8, pressed));
                }
            }

            cols.unselect_all();
        }

        *scan_pos = ScanPos::new(0, 0);
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
