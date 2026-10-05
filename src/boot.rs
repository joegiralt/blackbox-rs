//! Clean slate behind 1010music's installer.
//!
//! Images at 0x08040000 are started by the stock installer, not from reset, and its exit state
//! is unknown: maybe on its process stack, unprivileged, interrupts masked, SysTick running,
//! MPU and D-cache on, peripherals mid-transfer. A system reset only returns to the installer,
//! so `__pre_init` undoes all of that by hand before cortex-m-rt initialises RAM and calls
//! `main`. `tools/test_boot.py` boots the built image in an emulator with exactly that state.

use core::arch::{asm, global_asm};

use embassy_stm32::pac;

// The installer may jump on its own process stack or unprivileged; the stock app's reset
// handler defends the same way. cortex-m-rt has already loaded MSP (`set-sp`).
global_asm!(
    ".section .text.__pre_init",
    ".global __pre_init",
    ".thumb_func",
    "__pre_init:",
    "movs r0, #0",
    "msr CONTROL, r0",
    "isb",
    "b __behind_installer",
);

/// Set every listed reset field of an RCC reset register, then release them all.
macro_rules! pulse {
    ($reg:ident: $($f:ident),+) => {{
        pac::RCC.$reg().write(|w| { $(w.$f(true);)+ });
        pac::RCC.$reg().write(|_| {});
    }};
}

/// Runs privileged on MSP, before RAM is initialised: no statics, no `defmt`, no panics.
#[no_mangle]
unsafe extern "C" fn __behind_installer() {
    // SAFETY: nothing else runs yet. `steal`'s one static write lands in `.bss`, which
    // `Reset` zeroes right after this returns.
    let mut cp = unsafe { cortex_m::Peripherals::steal() };

    // SAFETY: plain register writes, privileged, no live users of SysTick/NVIC/MPU yet.
    unsafe {
        cp.SYST.csr.write(0);
        cp.SCB.icsr.write(1 << 25); // PENDSTCLR
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

    // Clean then disable: dirty lines the installer left reach memory first.
    if cortex_m::peripheral::SCB::dcache_enabled() {
        cp.SCB.disable_dcache(&mut cp.CPUID);
    }

    // Every PAC-defined reset field except AHB3 CPURST, which would reset the core itself.
    pulse!(ahb3rstr: set_mdmarst, set_dma2drst, set_jpgdecrst, set_fmcrst, set_quadspirst,
        set_sdmmc1rst, set_octospi2rst, set_iomngrrst, set_otfd1rst, set_otfd2rst);
    pulse!(ahb1rstr: set_dma1rst, set_dma2rst, set_adc12rst, set_artrst, set_ethrst,
        set_usb_otg_hsrst, set_usb_otg_fsrst);
    pulse!(ahb2rstr: set_dcmirst, set_cryprst, set_hashrst, set_rngrst, set_sdmmc2rst,
        set_fmacrst, set_cordicrst);
    pulse!(ahb4rstr: set_gpioarst, set_gpiobrst, set_gpiocrst, set_gpiodrst, set_gpioerst,
        set_gpiofrst, set_gpiogrst, set_gpiohrst, set_gpioirst, set_gpiojrst, set_gpiokrst,
        set_crcrst, set_bdmarst, set_adc3rst, set_hsemrst);
    pulse!(apb3rstr: set_ltdcrst, set_dsirst);
    pulse!(apb1lrstr: set_tim2rst, set_tim3rst, set_tim4rst, set_tim5rst, set_tim6rst,
        set_tim7rst, set_tim12rst, set_tim13rst, set_tim14rst, set_lptim1rst, set_spi2rst,
        set_spi3rst, set_spdifrxrst, set_usart2rst, set_usart3rst, set_uart4rst, set_uart5rst,
        set_i2c1rst, set_i2c2rst, set_i2c3rst, set_i2c5rst, set_cecrst, set_dac12rst,
        set_uart7rst, set_uart8rst);
    pulse!(apb1hrstr: set_crsrst, set_swpmirst, set_opamprst, set_mdiosrst, set_fdcanrst,
        set_tim23rst, set_tim24rst);
    pulse!(apb2rstr: set_tim1rst, set_tim8rst, set_usart1rst, set_usart6rst, set_uart9rst,
        set_usart10rst, set_spi1rst, set_spi4rst, set_tim15rst, set_tim16rst, set_tim17rst,
        set_spi5rst, set_sai1rst, set_sai2rst, set_sai3rst, set_dfsdm1rst, set_hrtimrst);
    pulse!(apb4rstr: set_syscfgrst, set_lpuart1rst, set_spi6rst, set_i2c4rst, set_lptim2rst,
        set_lptim3rst, set_lptim4rst, set_lptim5rst, set_dac2rst, set_comp12rst, set_vrefrst,
        set_sai4rst, set_dtsrst);

    // SAFETY: SysTick, NVIC and pending bits are cleared above, so unmasking delivers nothing.
    unsafe {
        cortex_m::register::basepri::write(0);
        asm!("cpsie f", "cpsie i", options(nomem, nostack, preserves_flags));
    }
}
