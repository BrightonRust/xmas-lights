#![no_main]
#![no_std]

extern crate panic_halt;

use stm32f4xx_hal as hal;
use ws2812_spi as ws2812;

use hal::spi::*;
use hal::{prelude::*, stm32};

use ws2812::Ws2812;

use smart_leds::SmartLedsWrite;
use smart_leds_trait::RGB8;

use cortex_m::iprintln;
//use cortex_m_semihosting::hprintln;

use rtfm::cyccnt::U32Ext;

const PERIOD: u32 = 48_000_000;
const NUM_LEDS: usize = 4;

// Types for WS
use hal::gpio::gpiob::{PB3, PB5};
use hal::gpio::{Alternate, AF5};
use hal::spi::{NoMiso, Spi};
use hal::stm32::SPI1;

type Pins = (PB3<Alternate<AF5>>, NoMiso, PB5<Alternate<AF5>>);

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

        // Set up the system clock at 48MHz
        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(48.mhz()).freeze();

        // Initialize (enable) the monotonic timer (CYCCNT)
        cx.core.DCB.enable_trace();
        cx.core.DWT.enable_cycle_counter();

        // ITM for debugging output
        let itm = cx.core.ITM;

        // Configure pins for SPI
        // We don't connect sck, but I think the SPI traits require it?
        let gpiob = dp.GPIOB.split();
        let sck = gpiob.pb3.into_alternate_af5();

        // Master Out Slave In - pb5, Nucleo 64 pin d4
        let mosi = gpiob.pb5.into_alternate_af5();

        let spi = Spi::spi1(
            dp.SPI1,
            (sck, NoMiso, mosi),
            Mode {
                polarity: Polarity::IdleLow,
                phase: Phase::CaptureOnFirstTransition,
            },
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

    extern "C" {
        fn USART1();
    }
};
