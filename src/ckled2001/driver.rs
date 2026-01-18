use crate::ckled2001::registers::*;
use embassy_stm32::{i2c, i2c::I2c, mode::Async};

pub const DEFAULT_CURRENT_TUNE: [u8; LED_CURRENT_TUNE_LENGTH] = [0xFF; LED_CURRENT_TUNE_LENGTH];

#[derive(Copy, Clone)]
pub struct CkLed {
    pub driver: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Copy, Clone)]
pub enum CkledError {
    I2c,
    BlockTooLarge,
}

pub struct Ckled2001<'d, const DRIVER_COUNT: usize> {
    i2c: I2c<'d, Async, i2c::Master>,
    addrs: [u8; DRIVER_COUNT],
    leds: &'static [CkLed],

    pwm: [[u8; LED_PWM_LENGTH]; DRIVER_COUNT],
    pwm_dirty: [bool; DRIVER_COUNT],

    led_ctrl: [[u8; LED_CONTROL_ON_OFF_LENGTH]; DRIVER_COUNT],
    led_ctrl_dirty: [bool; DRIVER_COUNT],

    global_brightness: u8,
}

impl<'d, const DRIVER_COUNT: usize> Ckled2001<'d, DRIVER_COUNT> {
    pub fn new(i2c: I2c<'d, Async, i2c::Master>, addrs: [u8; DRIVER_COUNT], leds: &'static [CkLed]) -> Self {
        Self {
            i2c,
            addrs,
            leds,
            pwm: [[0; LED_PWM_LENGTH]; DRIVER_COUNT],
            pwm_dirty: [false; DRIVER_COUNT],
            led_ctrl: [[0; LED_CONTROL_ON_OFF_LENGTH]; DRIVER_COUNT],
            led_ctrl_dirty: [false; DRIVER_COUNT],
            global_brightness: 255,
        }
    }

    #[inline]
    fn scale(&self, v: u8) -> u8 { ((v as u16 * self.global_brightness as u16 + 127) / 255) as u8 }

    #[inline]
    fn gamma2(&self, v: u8) -> u8 {
        let x = v as u16;
        ((x * x + 127) / 255) as u8
    }

    async fn set_global_brightness(&mut self, b: u8) { self.global_brightness = b; }

    pub async fn set_global_brightness_percent(&mut self, percent: u8) {
        let p = percent.min(100);
        let b = (p as u16 * 255 / 100) as u8;
        self.set_global_brightness(b).await;
    }

    #[inline]
    async fn write_bytes(&mut self, addr7: u8, bytes: &[u8]) -> Result<(), CkledError> {
        self.i2c.write(addr7, bytes).await.map_err(|_| CkledError::I2c)
    }

    #[inline]
    async fn write_reg(&mut self, addr7: u8, reg: u8, data: u8) -> Result<(), CkledError> {
        self.write_bytes(addr7, &[reg, data]).await
    }

    #[inline]
    async fn select_page(&mut self, addr7: u8, page: u8) -> Result<(), CkledError> {
        self.write_reg(addr7, CONFIGURE_CMD_PAGE, page).await
    }

    async fn write_block(&mut self, addr7: u8, start_reg: u8, data: &[u8]) -> Result<(), CkledError> {
        if data.len() > 64 {
            return Err(CkledError::BlockTooLarge);
        }

        let mut buf = [0u8; 65];
        buf[0] = start_reg;
        buf[1..1 + data.len()].copy_from_slice(data);

        self.write_bytes(addr7, &buf[..1 + data.len()]).await
    }

    async fn write_repeat(&mut self, addr7: u8, start_reg: u8, value: u8, len: usize) -> Result<(), CkledError> {
        let mut tmp = [0u8; 64];
        tmp.fill(value);

        let mut offset = 0usize;
        while offset < len {
            let n = (len - offset).min(64);
            self.write_block(addr7, start_reg.wrapping_add(offset as u8), &tmp[..n]).await?;
            offset += n;
        }
        Ok(())
    }

    async fn write_pwm_page(&mut self, addr7: u8, pwm: &[u8; LED_PWM_LENGTH]) -> Result<(), CkledError> {
        self.select_page(addr7, LED_PWM_PAGE).await?;
        for (chunk_idx, chunk) in pwm.chunks(64).enumerate() {
            let start = (chunk_idx * 64) as u8;
            self.write_block(addr7, start, chunk).await?;
        }
        Ok(())
    }

    pub async fn init(&mut self) -> Result<(), CkledError> {
        for di in 0..DRIVER_COUNT {
            let addr = self.addrs[di];

            // Function page setup
            self.select_page(addr, FUNCTION_PAGE).await?;
            self.write_reg(addr, CONFIGURATION_REG, MSKSW_SHUT_DOWN_MODE).await?;
            self.write_reg(addr, PDU_REG, MSKSET_CA_CB_CHANNEL).await?;
            self.write_reg(addr, SCAN_PHASE_REG, MSKPHASE_12CHANNEL).await?;
            self.write_reg(addr, SLEW_RATE_CONTROL_MODE1_REG, MSKPWM_DELAY_PHASE_ENABLE).await?;
            self.write_reg(addr, SLEW_RATE_CONTROL_MODE2_REG, MSKDRIVING_SINKING_CHHANNEL_SLEWRATE_ENABLE).await?;
            self.write_reg(addr, SOFTWARE_SLEEP_REG, MSKSLEEP_DISABLE).await?;

            // LED control page: all off
            self.select_page(addr, LED_CONTROL_PAGE).await?;
            self.write_repeat(addr, 0x00, 0x00, LED_CONTROL_ON_OFF_LENGTH).await?;
            self.led_ctrl[di].fill(0x00);
            self.led_ctrl_dirty[di] = false;

            // PWM: all 0
            self.pwm[di].fill(0x00);
            let pwm_copy = self.pwm[di];
            self.write_pwm_page(addr, &pwm_copy).await?;
            self.pwm_dirty[di] = false;

            // Current tune page
            self.select_page(addr, CURRENT_TUNE_PAGE).await?;
            self.write_block(addr, 0x00, &DEFAULT_CURRENT_TUNE).await?;

            // Enable LEDs
            self.select_page(addr, LED_CONTROL_PAGE).await?;
            self.write_repeat(addr, 0x00, 0xFF, LED_CONTROL_ON_OFF_LENGTH).await?;
            self.led_ctrl[di].fill(0xFF);
            self.led_ctrl_dirty[di] = false;

            // Return normal mode
            self.select_page(addr, FUNCTION_PAGE).await?;
            self.write_reg(addr, CONFIGURATION_REG, MSKSW_NORMAL_MODE).await?;
        }

        Ok(())
    }

    pub async fn set_color(&mut self, led_index: usize, r: u8, g: u8, b: u8, brightness: u8) {
        if led_index >= self.leds.len() {
            return;
        }

        self.set_global_brightness_percent(brightness).await;

        let led = self.leds[led_index];
        let d = led.driver as usize;
        if d >= DRIVER_COUNT {
            return;
        }

        let r = self.gamma2(self.scale(r));
        let g = self.gamma2(self.scale(g));
        let b = self.gamma2(self.scale(b));

        self.pwm[d][led.r as usize] = r;
        self.pwm[d][led.g as usize] = g;
        self.pwm[d][led.b as usize] = b;
        self.pwm_dirty[d] = true;
    }

    pub async fn set_color_all(&mut self, r: u8, g: u8, b: u8, brightness: u8) -> Result<(), CkledError> {
        for i in 0..self.leds.len() {
            self.set_color(i, r, g, b, brightness).await;
        }
        self.flush().await
    }

    pub async fn flush(&mut self) -> Result<(), CkledError> {
        for di in 0..DRIVER_COUNT {
            let addr = self.addrs[di];

            if self.led_ctrl_dirty[di] {
                self.select_page(addr, LED_CONTROL_PAGE).await?;

                let mut offset = 0usize;
                while offset < LED_CONTROL_ON_OFF_LENGTH {
                    let n = (LED_CONTROL_ON_OFF_LENGTH - offset).min(64);
                    let mut tmp = [0u8; 64];
                    tmp[..n].copy_from_slice(&self.led_ctrl[di][offset..offset + n]);
                    self.write_block(addr, offset as u8, &tmp[..n]).await?;
                    offset += n;
                }

                self.led_ctrl_dirty[di] = false;
            }

            if self.pwm_dirty[di] {
                // Write PWM page (192 bytes) in 64-byte chunks.
                self.select_page(addr, LED_PWM_PAGE).await?;

                for chunk_idx in 0..(LED_PWM_LENGTH / 64) {
                    let base = chunk_idx * 64;
                    let mut tmp = [0u8; 64];
                    tmp.copy_from_slice(&self.pwm[di][base..base + 64]);
                    self.write_block(addr, base as u8, &tmp).await?;
                }

                self.pwm_dirty[di] = false;
            }
        }

        Ok(())
    }
}
