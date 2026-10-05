//! Clean slate behind 1010music's installer.
//!
//! Images at 0x08040000 are started by the stock installer, not from reset, and its exit state
//! is unknown: maybe on its process stack, interrupts masked, SysTick running, MPU and D-cache
//! on, peripherals mid-transfer. A system reset only returns to the installer, so `__pre_init`
//! undoes all of that by hand before cortex-m-rt initialises RAM and calls `main`.
//! `tools/test_boot.py` boots the built image in an emulator with exactly that state.
//! An unprivileged jump cannot be recovered here: Reset's VTOR store would already fault.

use core::arch::{asm, global_asm};

use embassy_stm32::pac;

// Registers only, before any Rust stack frame exists. cortex-m-rt has already loaded MSP
// (`set-sp`) and written VTOR (`set-vtor`).
//
// D-cache: clearing CCR.DC and then cleaning by set/way writes back every dirty line, so a
// stack store between the two would be overwritten by a stale copy of the installer's stack.
// Cortex-M7 L1 D-cache is always 4-way with 32-byte lines; only the set count is read.
global_asm!(
    ".section .text.__pre_init",
    ".global __pre_init",
    ".thumb_func",
    "__pre_init:",
    // Our vector table is live from here; IRQs stay masked until NVIC is cleared. Not emulated.
    "cpsid i",
    // Leave the installer's process stack.
    "movs r0, #0",
    "msr CONTROL, r0",
    "isb",
    "ldr r0, =0xE000ED14", // CCR
    "ldr r1, [r0]",
    "tst r1, #0x10000", // DC
    "beq 2f",
    "bic r1, r1, #0x10000",
    "dsb",
    "str r1, [r0]",
    "dsb",
    "isb",
    "movs r1, #0",
    "str r1, [r0, #0x70]", // CSSELR = L1 data
    "dsb",
    "isb",
    "ldr r1, [r0, #0x6C]", // CCSIDR
    "ubfx r1, r1, #13, #15", // sets - 1
    "lsls r1, r1, #5",
    "0:",
    "orr r2, r1, #0xC0000000", // way 3
    "1:",
    "str r2, [r0, #0x260]", // DCCISW
    "subs r2, r2, #0x40000000", // borrows after way 0
    "bcs 1b",
    "subs r1, r1, #32", // borrows after set 0
    "bcs 0b",
    "dsb",
    "isb",
    "2:",
    "b __behind_installer",
);

/// Set every listed reset field of an RCC reset register, then release them all.
macro_rules! pulse {
    ($reg:ident: $($f:ident),+) => {{
        pac::RCC.$reg().write(|w| { $(w.$f(true);)+ });
        pac::RCC.$reg().write(|_| {});
    }};
}

/// Runs privileged on MSP with the D-cache off and IRQs masked, before RAM is initialised:
/// no statics, no `defmt`, no panics.
#[no_mangle]
unsafe extern "C" fn __behind_installer() {
    // SAFETY: nothing else runs yet. `steal`'s one static write lands in `.bss`, which
    // `Reset` zeroes right after this returns.
    let cp = unsafe { cortex_m::Peripherals::steal() };

    // SAFETY: plain register writes, privileged, no live users of SysTick/NVIC/MPU yet.
    unsafe {
        cp.SYST.csr.write(0);
        cp.SCB.icsr.write(1 << 25); // PENDSTCLR
        cp.SCB.icsr.write(1 << 27); // PENDSVCLR
        for r in cp.NVIC.icer.iter().take(8) {
            r.write(0xFFFF_FFFF);
        }
        for r in cp.NVIC.icpr.iter().take(8) {
            r.write(0xFFFF_FFFF);
        }

        cp.MPU.ctrl.write(0);
        for r in 0..16 {
            cp.MPU.rnr.write(r);
            cp.MPU.rbar.write(0);
            cp.MPU.rasr.write(0);
        }
        cortex_m::asm::dsb();
        cortex_m::asm::isb();
    }

    // H743 peripherals only: the PAC's RCC block covers every H7, and fields for parts this
    // chip lacks (OCTOSPI, OTFDEC, CRYP, DSI, ...) are reserved bits here, which stay 0.
    // AHB3 CPURST is left out too: it would reset the core itself.
    pulse!(ahb3rstr: set_mdmarst, set_dma2drst, set_jpgdecrst, set_fmcrst, set_quadspirst,
        set_sdmmc1rst);
    pulse!(ahb1rstr: set_dma1rst, set_dma2rst, set_adc12rst, set_ethrst, set_usb_otg_hsrst,
        set_usb_otg_fsrst);
    pulse!(ahb2rstr: set_dcmirst, set_rngrst, set_sdmmc2rst);
    pulse!(ahb4rstr: set_gpioarst, set_gpiobrst, set_gpiocrst, set_gpiodrst, set_gpioerst,
        set_gpiofrst, set_gpiogrst, set_gpiohrst, set_gpioirst, set_gpiojrst, set_gpiokrst,
        set_crcrst, set_bdmarst, set_adc3rst, set_hsemrst);
    pulse!(apb3rstr: set_ltdcrst);
    pulse!(apb1lrstr: set_tim2rst, set_tim3rst, set_tim4rst, set_tim5rst, set_tim6rst,
        set_tim7rst, set_tim12rst, set_tim13rst, set_tim14rst, set_lptim1rst, set_spi2rst,
        set_spi3rst, set_spdifrxrst, set_usart2rst, set_usart3rst, set_uart4rst, set_uart5rst,
        set_i2c1rst, set_i2c2rst, set_i2c3rst, set_cecrst, set_dac12rst, set_uart7rst,
        set_uart8rst);
    pulse!(apb1hrstr: set_crsrst, set_swpmirst, set_opamprst, set_mdiosrst, set_fdcanrst);
    pulse!(apb2rstr: set_tim1rst, set_tim8rst, set_usart1rst, set_usart6rst, set_spi1rst,
        set_spi4rst, set_tim15rst, set_tim16rst, set_tim17rst, set_spi5rst, set_sai1rst,
        set_sai2rst, set_sai3rst, set_dfsdm1rst, set_hrtimrst);
    pulse!(apb4rstr: set_syscfgrst, set_lpuart1rst, set_spi6rst, set_i2c4rst, set_lptim2rst,
        set_lptim3rst, set_lptim4rst, set_lptim5rst, set_comp12rst, set_vrefrst, set_sai4rst);

    // SAFETY: SysTick, NVIC and pending bits are cleared above, so unmasking delivers nothing.
    unsafe {
        cortex_m::register::basepri::write(0);
        asm!("cpsie f", "cpsie i", options(nomem, nostack, preserves_flags));
    }
}
