//! Shared panic and HardFault handlers for the examples: blink every LED (the image runs from
//! an SD card with no probe attached). Include with `#[path = "common/fault.rs"] mod fault;`.

use cortex_m_rt::ExceptionFrame;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    blackbox_rs::leds::panic_blink()
}

#[cortex_m_rt::exception]
unsafe fn HardFault(_: &ExceptionFrame) -> ! {
    blackbox_rs::leds::panic_blink()
}
