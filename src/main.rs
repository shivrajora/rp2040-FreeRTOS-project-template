//! Blinks the LED on a Pico board using FreeRTOS tasks.
//!
//! GP25 is the on-board LED pin on the Raspberry Pi Pico.
//!
//! # Modes
//!
//! **Default** — single FreeRTOS task on whichever core the scheduler picks:
//! ```sh
//! cargo run
//! ```
//!
//! **SMP demo** — two tasks with explicit core affinity, demonstrating both
//! Cortex-M0+ cores running FreeRTOS tasks simultaneously:
//! ```sh
//! cargo run --features smp-demo
//! ```
//! The LED task is pinned to core 0 and a counter task is pinned to core 1.
//! Both tasks log their actual core ID via defmt so you can verify they are
//! running on the expected core.
#![no_std]
#![no_main]

use bsp::entry;
use cortex_m::asm;
use cortex_m_rt::{ExceptionFrame, exception};
use defmt::*;
use defmt_rtt as _;
use embedded_hal::digital::OutputPin;
use freertos_rust::*;
use panic_probe as _;

#[global_allocator]
static GLOBAL: FreeRtosAllocator = FreeRtosAllocator;

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
use rp_pico as bsp;
// use sparkfun_pro_micro_rp2040 as bsp;

use bsp::hal::{clocks::init_clocks_and_plls, pac, sio::Sio, watchdog::Watchdog};

#[entry]
fn main() -> ! {
    #[cfg(not(feature = "smp-demo"))]
    Task::new()
        .name("default")
        .stack_size(1000)
        .start(move |_| app_main())
        .unwrap();

    #[cfg(feature = "smp-demo")]
    {
        // LED blink pinned to core 0 (affinity mask = 0b01)
        Task::new()
            .name("led-core0")
            .stack_size(1000)
            .core_affinity(1 << 0)
            .start(move |_| app_main())
            .unwrap();

        // Counter task pinned to core 1 (affinity mask = 0b10)
        Task::new()
            .name("counter-core1")
            .stack_size(512)
            .core_affinity(1 << 1)
            .start(move |_| smp_counter_task())
            .unwrap();
    }

    FreeRtosUtils::start_scheduler();
}

fn app_main() -> ! {
    #[cfg(feature = "smp-demo")]
    info!("LED task on core{}", get_core_id());
    #[cfg(not(feature = "smp-demo"))]
    info!("Program start");
    let mut pac = pac::Peripherals::take().unwrap();
    let sio = Sio::new(pac.SIO);
    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    // External high-speed crystal on the pico board is 12Mhz
    let external_xtal_freq_hz = 12_000_000u32;
    let _clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // This is the correct pin on the Raspberry Pico board. On other boards, even if they have an
    // on-board LED, it might need to be changed.
    //
    // Notably, on the Pico W, the LED is not connected to any of the RP2040 GPIOs but to the cyw43 module instead.
    // One way to do that is by using [embassy](https://github.com/embassy-rs/embassy/blob/main/examples/rp/src/bin/wifi_blinky.rs)
    //
    // If you have a Pico W and want to toggle a LED with a simple GPIO output pin, you can connect an external
    // LED to one of the GPIO pins, and reference that pin here. Don't forget adding an appropriate resistor
    // in series with the LED.
    let mut led_pin = pins.led.into_push_pull_output();

    loop {
        info!("on!");
        led_pin.set_high().unwrap();
        CurrentTask::delay(Duration::ms(500));
        info!("off!");
        led_pin.set_low().unwrap();
        CurrentTask::delay(Duration::ms(500));
    }
}

#[allow(non_snake_case)]
#[exception]
unsafe fn DefaultHandler(_irqn: i16) {
    // custom default handler
    // irqn is negative for Cortex-M exceptions
    // irqn is positive for device specific (line IRQ)
    // set_led(true);(true);
    // panic!("Exception: {}", irqn);
    asm::bkpt();
    loop {}
}

#[allow(non_snake_case)]
#[exception]
unsafe fn HardFault(_ef: &ExceptionFrame) -> ! {
    asm::bkpt();
    loop {}
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
fn vApplicationMallocFailedHook() {
    asm::bkpt();
    loop {}
}

#[allow(non_snake_case)]
#[unsafe(no_mangle)]
fn vApplicationStackOverflowHook(_pxTask: FreeRtosTaskHandle, _pcTaskName: FreeRtosCharPtr) {
    asm::bkpt();
}

#[cfg(feature = "smp-demo")]
fn smp_counter_task() -> ! {
    let mut count: u32 = 0;
    loop {
        info!("core{} counter: {}", get_core_id(), count);
        count = count.wrapping_add(1);
        CurrentTask::delay(Duration::ms(1000));
    }
}

// End of file
