#![no_main]
#![no_std]

//extern crate panic_halt
use panic_semihosting as _; // logs messages to the host stderr; requires a debugger
                            //extern crate panic_halt;

use stm32f4xx_hal as hal;
use ws2812_spi as ws2812;

use hal::spi::*;
use hal::{prelude::*, stm32};

use ws2812::Ws2812;

use smart_leds::SmartLedsWrite;
use smart_leds_trait::RGB8;

use cortex_m::{asm, iprintln};
//use cortex_m_semihosting::hprintln;

use rtfm::cyccnt::U32Ext;

// CPU cycles per second
const CORE_CLOCK_MHZ: u32 = 48;
const PERIOD: u32 = CORE_CLOCK_MHZ * 1_000_000;
const NUM_LEDS: usize = 4;

// Types for WS
use hal::gpio::gpioa::{PA5, PA7};
use hal::gpio::{Alternate, AF5};
use hal::spi::{NoMiso, Spi};
use hal::stm32::SPI1;

type Pins = (PA5<Alternate<AF5>>, NoMiso, PA7<Alternate<AF5>>);

#[rtfm::app(device = stm32f4xx_hal::stm32, peripherals = true, monotonic = rtfm::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
        ws: Ws2812<Spi<SPI1, Pins>>,
        itm: cortex_m::peripheral::ITM,
    }

    #[init(schedule = [lights_on])]
    fn init(mut cx: init::Context) -> init::LateResources {
        // Device specific peripherals
        let dp: stm32::Peripherals = cx.device;

        // Set up the system clock, using the Nucleo's 8MHz crytal
        // instead of the (less accurate) internal oscillator.
        let rcc = dp.RCC.constrain();
        let clocks = rcc
            .cfgr
            .use_hse(8.mhz())
            .sysclk(CORE_CLOCK_MHZ.mhz())
            .freeze();

        // Initialize (enable) the monotonic timer (CYCCNT)
        cx.core.DCB.enable_trace();
        cx.core.DWT.enable_cycle_counter();

        // ITM for debugging output
        let itm = cx.core.ITM;

        // Configure pins for SPI
        // Use the SPI1 peripheral, this uses the following pins in "Alternative Function 5" mode:
        // SCK  PA5 (Nucleo 64 "Morpho" header CN10 pin 11 and Arduino pin "D13"),
        // MISO PA6 (Nucleo 64 "Morpho" header CN10 pin 13 and arduino header pin "D12")
        // MOSI PA7 (Nucleo 64 "Morpho" header CN10 pin 15 and arduino header pin "D11")
        //
        // See "STM32F446xC/E Data Sheet" (Table 11 "Alternative Function"), and:
        // "UM1724 User manual STM32 Nucleo-64 boards (MB1136)" (Table 19, and Table 29).

        // ... although we only need to use MOSI (master out slave in)
        // n.b. PA5 is also connected to the Nucleo-64 LD2 (the on-board green LED), so this
        // should switch on with ~50% brightness whilst SPI data is being sent.
        //
        // n.b. We can Specify `NoMiso`, but the SPI traits require an SPI clock pin.
        let gpioa = dp.GPIOA.split();
        let sck = gpioa.pa5.into_alternate_af5();
        let mosi = gpioa.pa7.into_alternate_af5();

        let spi = Spi::spi1(
            dp.SPI1,
            (sck, NoMiso, mosi),
            Mode {
                polarity: Polarity::IdleLow,
                phase: Phase::CaptureOnFirstTransition,
            },
            // Setup SPI clock to run at 3 MHz to keep WS2812s happy.
            stm32f4xx_hal::time::KiloHertz(3000).into(),
            clocks,
        );

        let ws = Ws2812::new(spi);

        cx.schedule
            .lights_on(cx.start + PERIOD.cycles())
            .expect("failed schedule initial lights on");

        init::LateResources { ws, itm }
    }

    #[task(schedule = [lights_off], resources = [ws, itm])]
    fn lights_on(cx: lights_on::Context) {
        let lights_on::Resources { ws, itm } = cx.resources;
        let port = &mut itm.stim[0];

        iprintln!(port, "ON");

        let blue = RGB8 {
            b: 0xa0,
            g: 0,
            r: 0,
        };
        let data = [blue; NUM_LEDS];

        ws.write(data.iter().cloned())
            .expect("Failed to write lights_on");

        cx.schedule
            .lights_off(cx.scheduled + PERIOD.cycles())
            .expect("Failed to schedule lights_off");
    }

    #[task(schedule = [lights_on], resources = [ws, itm])]
    fn lights_off(cx: lights_off::Context) {
        let lights_off::Resources { ws, itm } = cx.resources;
        let port = &mut itm.stim[0];

        //hprintln!("OFF").unwrap();
        iprintln!(port, "OFF");

        let empty = [RGB8::default(); NUM_LEDS];
        ws.write(empty.iter().cloned())
            .expect("Failed to write lights_off");

        cx.schedule
            .lights_on(cx.scheduled + PERIOD.cycles())
            .expect("Failed to schedule lights_on");
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            asm::nop();
        }
    }

    extern "C" {
        fn USART1();
    }
};
