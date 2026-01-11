#![no_main]
#![no_std]

mod hc595_cols;
mod keymap;
mod shiftreg_matrix;
mod vial;

use crate::{hc595_cols::Hc595Cols, shiftreg_matrix::ShiftRegMatrix};
use embassy_executor::Spawner;
use embassy_stm32::{
    Config,
    bind_interrupts,
    exti,
    exti::ExtiInput,
    flash::Flash,
    gpio::{Level, Output, Pull, Speed},
    interrupt::typelevel,
    peripherals::USB,
    rcc::{self, mux},
    usb::{self, Driver},
};
use panic_probe as _;
use rmk::{
    channel::EVENT_CHANNEL,
    config::{BehaviorConfig, DeviceConfig, PositionalConfig, RmkConfig, StorageConfig, VialConfig},
    futures::future::join3,
    initialize_encoder_keymap_and_storage,
    input_device::{Runnable, rotary_encoder::RotaryEncoder},
    keyboard::Keyboard,
    run_devices,
    run_rmk,
    storage::async_flash_wrapper,
};
use vial::{VIAL_KEYBOARD_DEF, VIAL_KEYBOARD_ID};

bind_interrupts!(struct Irqs {
    USB => usb::InterruptHandler<USB>;
    EXTI0 => exti::InterruptHandler<typelevel::EXTI0>;
    EXTI3 => exti::InterruptHandler<typelevel::EXTI3>;
    EXTI4 => exti::InterruptHandler<typelevel::EXTI4>;
    EXTI9_5 => exti::InterruptHandler<typelevel::EXTI9_5>;
    EXTI15_10 => exti::InterruptHandler<typelevel::EXTI15_10>;
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
        vial_config: VialConfig::new(VIAL_KEYBOARD_ID, VIAL_KEYBOARD_DEF, &[(5, 0), (3, 1)]),
        device_config: DeviceConfig {
            manufacturer: "Keychron",
            product_name: "Q1 Pro",
            vid: 0x3434,
            pid: 0x0611,
            serial_number: "vial:f64c2b3c:000001",
        },
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
        ExtiInput::new(p.PB5, p.EXTI5, Pull::Up, Irqs),
        ExtiInput::new(p.PB4, p.EXTI4, Pull::Up, Irqs),
        ExtiInput::new(p.PB3, p.EXTI3, Pull::Up, Irqs),
        ExtiInput::new(p.PA15, p.EXTI15, Pull::Up, Irqs),
        ExtiInput::new(p.PA14, p.EXTI14, Pull::Up, Irqs),
        ExtiInput::new(p.PA13, p.EXTI13, Pull::Up, Irqs),
    ];

    // Rotary enoder
    let pin_a = ExtiInput::new(p.PA10, p.EXTI10, Pull::Up, Irqs);
    let pin_b = ExtiInput::new(p.PA0, p.EXTI0, Pull::Up, Irqs);
    let mut encoder = RotaryEncoder::with_resolution(pin_a, pin_b, 4, true, 0);

    // Initialize the storage and keymap
    let mut default_keymap = keymap::get_default_keymap();
    let mut default_encoder = keymap::get_default_encoder_map();
    let mut behavior_config = BehaviorConfig::default();
    let storage_config = StorageConfig::default();
    let mut per_key_config = PositionalConfig::default();
    let (keymap, mut storage) = initialize_encoder_keymap_and_storage(
        &mut default_keymap,
        &mut default_encoder,
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
            (matrix, encoder) => EVENT_CHANNEL,
        ),
        keyboard.run(),
        run_rmk(&keymap, driver, &mut storage, rmk_config),
    )
    .await;
}
