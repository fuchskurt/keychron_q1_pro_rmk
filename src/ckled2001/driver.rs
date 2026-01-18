use embassy_stm32::{i2c, i2c::I2c, mode::Async};

pub const CONFIGURE_CMD_PAGE: u8 = 0xFD;

pub const LED_CONTROL_PAGE: u8 = 0x00;
pub const LED_PWM_PAGE: u8 = 0x01;
pub const FUNCTION_PAGE: u8 = 0x03;
pub const CURRENT_TUNE_PAGE: u8 = 0x04;

pub const CONFIGURATION_REG: u8 = 0x00;
pub const MSKSW_SHUT_DOWN_MODE: u8 = 0x00;
pub const MSKSW_NORMAL_MODE: u8 = 0x01;

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

pub const LED_CONTROL_ON_OFF_LENGTH: usize = 0x18;
pub const LED_PWM_LENGTH: usize = 0xC0;
pub const LED_CURRENT_TUNE_LENGTH: usize = 0x0C;

pub const DEFAULT_CURRENT_TUNE: [u8; LED_CURRENT_TUNE_LENGTH] = [0xFF; LED_CURRENT_TUNE_LENGTH];

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

    async fn set_global_brightness(&mut self, b: u8) { self.global_brightness = b; }

    pub async fn set_global_brightness_percent(&mut self, percent: u8) {
        let p = percent.min(100);
        let b = (p as u16 * 255 / 100) as u8;
        self.set_global_brightness(b).await;
    }

    #[inline]
    fn scale(&self, v: u8) -> u8 { ((v as u16 * self.global_brightness as u16 + 127) / 255) as u8 }

    #[inline]
    fn gamma2(&self, v: u8) -> u8 {
        // gamma ~2.0
        let x = v as u16;
        ((x * x + 127) / 255) as u8
    }

    async fn write_reg(&mut self, addr7: u8, reg: u8, data: u8) -> Result<(), ()> {
        let buf = [reg, data];
        self.i2c.write(addr7, &buf).await.map_err(|_| ())
    }

    async fn write_page(&mut self, addr7: u8, page: u8) -> Result<(), ()> {
        self.write_reg(addr7, CONFIGURE_CMD_PAGE, page).await
    }

    async fn write_block(&mut self, addr7: u8, start_reg: u8, data: &[u8]) -> Result<(), ()> {
        // First byte is start register; device auto-increments.
        // We keep 64-byte payload chunks like QMK => buffer size 65.
        if data.len() > 64 {
            return Err(());
        }
        let mut buf = [0u8; 65];
        buf[0] = start_reg;
        buf[1..1 + data.len()].copy_from_slice(data);
        self.i2c.write(addr7, &buf[..1 + data.len()]).await.map_err(|_| ())
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

    pub async fn init(&mut self) -> Result<(), ()> {
        for di in 0..DRIVER_COUNT {
            let addr = self.addrs[di];

            // Function page setup
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

            // clear all channels OFF
            for i in 0..LED_PWM_LENGTH {
                self.pwm[di][i] = 0x00;
            }
            let pwm_copy = self.pwm[di];
            self.write_pwm_page_from_buf(addr, &pwm_copy).await?;
            self.pwm_dirty[di] = false;

            // Current tune page: use QMK default (0x38 x 12)
            self.write_page(addr, CURRENT_TUNE_PAGE).await?;
            self.write_block(addr, 0, &DEFAULT_CURRENT_TUNE).await?;

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

    pub async fn set_color(&mut self, led_index: usize, r: u8, g: u8, b: u8, brightness: u8) -> Result<(), ()> {
        if led_index >= self.leds.len() {
            return Ok(());
        }

        self.set_global_brightness_percent(brightness).await;

        let led = self.leds[led_index];
        let d = led.driver as usize;
        if d >= DRIVER_COUNT {
            return Ok(());
        }

        let r = self.gamma2(self.scale(r));
        let g = self.gamma2(self.scale(g));
        let b = self.gamma2(self.scale(b));

        self.pwm[d][led.r as usize] = r;
        self.pwm[d][led.g as usize] = g;
        self.pwm[d][led.b as usize] = b;
        self.pwm_dirty[d] = true;

        Ok(())
    }

    pub async fn set_color_all(&mut self, r: u8, g: u8, b: u8, brightness: u8) -> Result<(), ()> {
        self.set_global_brightness_percent(brightness).await;

        let r = self.gamma2(self.scale(r));
        let g = self.gamma2(self.scale(g));
        let b = self.gamma2(self.scale(b));

        for i in 0..self.leds.len() {
            let led = self.leds[i];
            let d = led.driver as usize;
            if d >= DRIVER_COUNT {
                continue;
            }
            self.pwm[d][led.r as usize] = r;
            self.pwm[d][led.g as usize] = g;
            self.pwm[d][led.b as usize] = b;
            self.pwm_dirty[d] = true;
        }

        self.flush().await
    }

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
