//! Shared panic and fault handlers for the examples: blink every LED (the image runs from
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

// Any other exception or interrupt without a handler (MemManage, BusFault, UsageFault, ...).
#[cortex_m_rt::exception]
unsafe fn DefaultHandler(_irqn: i16) -> ! {
    blackbox_rs::leds::panic_blink()
}
