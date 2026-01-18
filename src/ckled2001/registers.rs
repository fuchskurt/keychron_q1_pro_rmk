#![allow(dead_code)]

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
