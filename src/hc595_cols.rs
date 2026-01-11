use embassy_stm32::gpio::Output;

pub struct Hc595Cols<'d> {
    data: Output<'d>,
    clk: Output<'d>,
    latch: Output<'d>,
}

impl<'d> Hc595Cols<'d> {
    pub fn new(data: Output<'d>, clk: Output<'d>, latch: Output<'d>) -> Self { Self { data, clk, latch } }

    #[inline(always)]
    fn pulse(clk: &mut Output<'d>) {
        clk.set_high();
        clk.set_low();
    }

    /// Shift out 16 bits LSB-first, then latch.
    pub fn write_u16_lsb_first(&mut self, mut v: u16) {
        self.latch.set_low();
        for _ in 0..16 {
            match v & 1 {
                0 => self.data.set_low(),
                _ => self.data.set_high(),
            }
            Self::pulse(&mut self.clk);
            v >>= 1;
        }
        self.latch.set_high();
        self.latch.set_low();
    }

    /// Unselect all cols => all ones
    pub fn unselect_all(&mut self) { self.write_u16_lsb_first(0xFFFF); }

    /// Select one col => all ones except selected bit is 0 (active-low)
    pub fn select_col_active_low(&mut self, col: usize) {
        let mask = 1u16 << (15 - col);
        self.write_u16_lsb_first(!mask);
    }
}
