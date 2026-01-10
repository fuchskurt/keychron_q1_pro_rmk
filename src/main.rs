#![no_main]
#![no_std]

mod hc595_cols;
mod keymap;
mod shiftreg_matrix;
mod vial;

use embassy_executor::Spawner;
use embassy_stm32::flash::Flash;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::peripherals::USB;
use embassy_stm32::rcc::{self, mux};
use embassy_stm32::usb::{Driver, InterruptHandler};
use embassy_stm32::{Config, bind_interrupts};
use panic_probe as _;
use rmk::channel::EVENT_CHANNEL;
use rmk::config::{BehaviorConfig, PositionalConfig, RmkConfig, StorageConfig, VialConfig};
use rmk::futures::future::join3;
use rmk::input_device::Runnable;
use rmk::keyboard::Keyboard;
use rmk::storage::async_flash_wrapper;
use rmk::{initialize_keymap_and_storage, run_devices, run_rmk};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

use crate::hc595_cols::Hc595Cols;
use crate::shiftreg_matrix::ShiftRegMatrix;

bind_interrupts!(struct Irqs {
    USB => InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // RCC config
    let mut config = Config::default();

    config.rcc.hsi = true;
    config.rcc.pll = Some(rcc::Pll {
        source: rcc::PllSource::HSI,
        prediv: rcc::PllPreDiv::DIV2,
        mul: rcc::PllMul::MUL12,
        divp: None,                     // not used
        divq: Some(rcc::PllQDiv::DIV2), // 48 MHz for USB
        divr: Some(rcc::PllRDiv::DIV2), // 48 MHz for SYSCLK
    });

    config.rcc.sys = rcc::Sysclk::PLL1_R;
    config.rcc.ahb_pre = rcc::AHBPrescaler::DIV1;
    config.rcc.apb1_pre = rcc::APBPrescaler::DIV1;
    config.rcc.apb2_pre = rcc::APBPrescaler::DIV1;
    config.rcc.mux.clk48sel = mux::Clk48sel::PLL1_Q;

    // Initialize peripherals
    let p = embassy_stm32::init(config);

    // Usb config
    let driver = Driver::new(p.USB, Irqs, p.PA12, p.PA11);

    // Use internal flash to emulate eeprom
    let flash = async_flash_wrapper(Flash::new_blocking(p.FLASH));

    // Keyboard config
    let rmk_config = RmkConfig {
        vial_config: VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF, &[(0, 0), (1, 1)]),
        ..Default::default()
    };

    // Custom Shift register Setup

    // Shift register GPIO bit-bang pins
    let data = Output::new(p.PA7, Level::Low, Speed::VeryHigh); // SER
    let clk = Output::new(p.PA1, Level::Low, Speed::VeryHigh); // SRCLK
    let lat = Output::new(p.PB0, Level::Low, Speed::VeryHigh); // RCLK

    // Pin config for cols from Shift register
    let cols = Hc595Cols::new(data, clk, lat);

    // 6 row inputs
    let rows = [
        Input::new(p.PB5, Pull::Up),
        Input::new(p.PB4, Pull::Up),
        Input::new(p.PB3, Pull::Up),
        Input::new(p.PA15, Pull::Up),
        Input::new(p.PA14, Pull::Up),
        Input::new(p.PA13, Pull::Up),
    ];

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let mut behavior_config = BehaviorConfig::default();
    let storage_config = StorageConfig::default();
    let mut per_key_config = PositionalConfig::default();
    let (keymap, mut storage) = initialize_keymap_and_storage(
        &mut default_keymap,
        flash,
        &storage_config,
        &mut behavior_config,
        &mut per_key_config,
    )
    .await;

    // Initialize the matrix + keyboard
    let mut matrix = ShiftRegMatrix::<6, 16>::new(rows, cols);
    let mut keyboard = Keyboard::new(&keymap);

    // Start
    join3(
        run_devices!(
            (matrix) => EVENT_CHANNEL,
        ),
        keyboard.run(),
        run_rmk(&keymap, driver, &mut storage, rmk_config),
    )
    .await;
}
