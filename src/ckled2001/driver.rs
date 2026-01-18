// ckled2001/driver.rs

use embassy_stm32::{i2c, i2c::I2c, mode::Async};

pub const CONFIGURE_CMD_PAGE: u8 = 0xFD;

pub const LED_CONTROL_PAGE: u8 = 0x00;
pub const LED_PWM_PAGE: u8 = 0x01;
pub const FUNCTION_PAGE: u8 = 0x03;
pub const CURRENT_TUNE_PAGE: u8 = 0x04;

// Function page registers
pub const CONFIGURATION_REG: u8 = 0x00;
pub const MSKSW_SHUT_DOWN_MODE: u8 = 0x00;
pub const MSKSW_NORMAL_MODE: u8 = 0x01;

pub const DRIVER_ID_REG: u8 = 0x11;
pub const CKLED2001_ID: u8 = 0x8A;

pub const PDU_REG: u8 = 0x13;
pub const MSKSET_CA_CB_CHANNEL: u8 = 0xAA;

pub const SCAN_PHASE_REG: u8 = 0x14;
pub const MSKPHASE_12CHANNEL: u8 = 0x00;

pub const SLEW_RATE_CONTROL_MODE1_REG: u8 = 0x15;
pub const MSKPWM_DELAY_PHASE_ENABLE: u8 = 0x04;

pub const SLEW_RATE_CONTROL_MODE2_REG: u8 = 0x16;
pub const MSKDRIVING_SINKING_CHHANNEL_SLEWRATE_ENABLE: u8 = 0xC0;

pub const SOFTWARE_SLEEP_REG: u8 = 0x1A;
pub const MSKSLEEP_ENABLE: u8 = 0x02;
pub const MSKSLEEP_DISABLE: u8 = 0x00;

// lengths
pub const LED_CONTROL_ON_OFF_LENGTH: usize = 0x18; // 24 bytes (0x00..=0x17)
pub const LED_PWM_LENGTH: usize = 0xC0; // 192 bytes
pub const LED_CURRENT_TUNE_LENGTH: usize = 0x0C; // 12 bytes

#[derive(Copy, Clone)]
pub struct CkLed {
    pub driver: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
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

    fn set_global_brightness(&mut self, b: u8) { self.global_brightness = b; }

    #[inline]
    fn scale(&self, v: u8) -> u8 { ((v as u16 * self.global_brightness as u16 + 127) / 255) as u8 }

    async fn write_reg(&mut self, addr7: u8, reg: u8, data: u8) -> Result<(), ()> {
        let buf = [reg, data];
        self.i2c.write(addr7, &buf).await.map_err(|_| ())
    }

    async fn write_page(&mut self, addr7: u8, page: u8) -> Result<(), ()> {
        self.write_reg(addr7, CONFIGURE_CMD_PAGE, page).await
    }

    async fn write_block(&mut self, addr7: u8, start_reg: u8, data: &[u8]) -> Result<(), ()> {
        // First byte is start register; device auto-increments.
        // QMK uses 64-byte chunks => 65 bytes total.
        if data.len() > 64 {
            return Err(());
        }
        let mut buf = [0u8; 65];
        buf[0] = start_reg;
        buf[1..1 + data.len()].copy_from_slice(data);
        self.i2c.write(addr7, &buf[..1 + data.len()]).await.map_err(|_| ())
    }

    pub async fn init(&mut self) -> Result<(), ()> {
        for di in 0..DRIVER_COUNT {
            let addr = self.addrs[di];

            self.write_page(addr, FUNCTION_PAGE).await?;
            self.write_reg(addr, CONFIGURATION_REG, MSKSW_SHUT_DOWN_MODE).await?;
            self.write_reg(addr, PDU_REG, MSKSET_CA_CB_CHANNEL).await?;
            self.write_reg(addr, SCAN_PHASE_REG, MSKPHASE_12CHANNEL).await?;
            self.write_reg(addr, SLEW_RATE_CONTROL_MODE1_REG, MSKPWM_DELAY_PHASE_ENABLE).await?;
            self.write_reg(addr, SLEW_RATE_CONTROL_MODE2_REG, MSKDRIVING_SINKING_CHHANNEL_SLEWRATE_ENABLE).await?;
            self.write_reg(addr, SOFTWARE_SLEEP_REG, MSKSLEEP_DISABLE).await?;

            // LED control page: clear then enable
            self.write_page(addr, LED_CONTROL_PAGE).await?;
            for r in 0..LED_CONTROL_ON_OFF_LENGTH {
                self.write_reg(addr, r as u8, 0x00).await?;
                self.led_ctrl[di][r] = 0x00;
            }

            // PWM page: clear all PWM (OFF)
            self.write_page(addr, LED_PWM_PAGE).await?;
            for i in 0..LED_PWM_LENGTH {
                self.pwm[di][i] = 0x00;
            }
            let pwm_copy = self.pwm[di];
            self.write_pwm_page_from_buf(addr, &pwm_copy).await?;
            self.pwm_dirty[di] = false;

            // Current tune page: default 0xFF x 12
            self.write_page(addr, CURRENT_TUNE_PAGE).await?;
            for r in 0..LED_CURRENT_TUNE_LENGTH {
                self.write_reg(addr, r as u8, 0xFF).await?;
            }

            // Enable LEDs in control page
            self.write_page(addr, LED_CONTROL_PAGE).await?;
            for r in 0..LED_CONTROL_ON_OFF_LENGTH {
                self.write_reg(addr, r as u8, 0xFF).await?;
                self.led_ctrl[di][r] = 0xFF;
            }
            self.led_ctrl_dirty[di] = false;

            // Return normal mode
            self.write_page(addr, FUNCTION_PAGE).await?;
            self.write_reg(addr, CONFIGURATION_REG, MSKSW_NORMAL_MODE).await?;
        }

        Ok(())
    }

    async fn write_pwm_page_from_buf(&mut self, addr7: u8, pwm: &[u8; LED_PWM_LENGTH]) -> Result<(), ()> {
        self.write_page(addr7, LED_PWM_PAGE).await?;

        let mut tmp = [0u8; 64];

        tmp.copy_from_slice(&pwm[0..64]);
        self.write_block(addr7, 0, &tmp).await?;

        tmp.copy_from_slice(&pwm[64..128]);
        self.write_block(addr7, 64, &tmp).await?;

        tmp.copy_from_slice(&pwm[128..192]);
        self.write_block(addr7, 128, &tmp).await?;

        Ok(())
    }

    pub fn set_color(&mut self, led_index: usize, r: u8, g: u8, b: u8) {
        if led_index >= self.leds.len() {
            return;
        }
        let led = self.leds[led_index];
        let d = led.driver as usize;
        if d >= DRIVER_COUNT {
            return;
        }

        self.pwm[d][led.r as usize] = self.scale(r);
        self.pwm[d][led.g as usize] = self.scale(g);
        self.pwm[d][led.b as usize] = self.scale(b);
        self.pwm_dirty[d] = true;
    }

    pub async fn set_color_all(&mut self, r: u8, g: u8, b: u8) -> Result<(), ()>  {
        for i in 0..self.leds.len() {
            self.set_color(i, r, g, b);
        }
        self.flush().await
    }

    pub fn set_led_control_register(&mut self, led_index: usize, red: bool, green: bool, blue: bool) {
        if led_index >= self.leds.len() {
            return;
        }
        let led = self.leds[led_index];
        let d = led.driver as usize;
        if d >= DRIVER_COUNT {
            return;
        }

        fn set_bit(buf: &mut [u8], chan: u8, on: bool) {
            let reg = (chan / 8) as usize;
            let bit = chan % 8;
            let mask = 1u8 << bit;
            if on {
                buf[reg] |= mask;
            } else {
                buf[reg] &= !mask;
            }
        }

        set_bit(&mut self.led_ctrl[d], led.r, red);
        set_bit(&mut self.led_ctrl[d], led.g, green);
        set_bit(&mut self.led_ctrl[d], led.b, blue);

        self.led_ctrl_dirty[d] = true;
    }

    /// Push only dirty buffers
    pub async fn flush(&mut self) -> Result<(), ()> {
        for di in 0..DRIVER_COUNT {
            let addr = self.addrs[di];

            if self.led_ctrl_dirty[di] {
                self.write_page(addr, LED_CONTROL_PAGE).await?;
                for r in 0..LED_CONTROL_ON_OFF_LENGTH {
                    self.write_reg(addr, r as u8, self.led_ctrl[di][r]).await?;
                }
                self.led_ctrl_dirty[di] = false;
            }

            if self.pwm_dirty[di] {
                self.write_page(addr, LED_PWM_PAGE).await?;
                let pwm_copy = self.pwm[di];
                self.write_pwm_page_from_buf(addr, &pwm_copy).await?;
                self.pwm_dirty[di] = false;
            }
        }
        Ok(())
    }

    pub async fn shutdown(&mut self) -> Result<(), ()> {
        for di in 0..DRIVER_COUNT {
            let addr = self.addrs[di];
            self.write_page(addr, FUNCTION_PAGE).await?;
            self.write_reg(addr, CONFIGURATION_REG, MSKSW_SHUT_DOWN_MODE).await?;
            self.write_reg(addr, SOFTWARE_SLEEP_REG, MSKSLEEP_ENABLE).await?;
        }
        Ok(())
    }

    pub async fn return_normal(&mut self) -> Result<(), ()> {
        for di in 0..DRIVER_COUNT {
            let addr = self.addrs[di];
            self.write_page(addr, FUNCTION_PAGE).await?;
            self.write_reg(addr, CONFIGURATION_REG, MSKSW_NORMAL_MODE).await?;
            self.write_reg(addr, SOFTWARE_SLEEP_REG, MSKSLEEP_DISABLE).await?;
        }
        Ok(())
    }
}
