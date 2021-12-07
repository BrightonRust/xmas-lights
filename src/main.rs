#![no_main]
#![no_std]

use defmt_rtt as _; // global logger using RTT (e.g. probe-run).

// logs panic message using "probe-run"
// https://github.com/knurling-rs/probe-run
use panic_probe as _;

// Give an *unused* interrupt to RTIC so that it can use this for scheduling
// see https://rtic.rs/dev/book/en/by-example/software_tasks.html
#[rtic::app(device = hal::pac, dispatchers = [USART1])]
mod app {
    use fugit::ExtU32;
    use stm32f4xx_hal as hal;
    use ws2812_spi as ws2812;

    use hal::{
        gpio::{gpioa, Floating, Input, NoPin},
        pac,
        prelude::*,
        spi::{self, Spi},
        timer::{monotonic::MonoTimer, Timer},
    };

    use ws2812::Ws2812;

    use smart_leds::SmartLedsWrite;
    use smart_leds_trait::RGB8;

    use cortex_m::asm;

    // CPU cycles per second
    const CORE_CLOCK_MHZ: u32 = 56;
    const NUM_LEDS: usize = 4;

    // Types for WS

    type F4Spi1 = Spi<
        pac::SPI1,
        (
            gpioa::PA5<Input<Floating>>, // SPI clock pin (unused, but wanted by hal spi constructor)
            NoPin,
            gpioa::PA7<Input<Floating>>, // SPI MOSI (data out to led string).
        ),
        spi::TransferModeNormal,
    >;

    #[shared]
    struct Shared {
        ws: ws2812_spi::Ws2812<F4Spi1>,
    }

    #[local]
    struct Local {}

    // Use the rtic monotomic timer provided by the stm32f4xx_hal crate
    // https://rtic.rs/dev/book/en/by-example/monotonic.html
    #[monotonic(binds = TIM2, default = true)]
    type MicrosecMono = MonoTimer<pac::TIM2, 1_000_000>;

    #[init]
    fn init(cx: init::Context) -> (Shared, Local, init::Monotonics) {
        defmt::println!("In init...");
        // Device specific peripherals
        let dp = cx.device;

        // Set up the system clock, using the internal oscillator.
        let rcc = dp.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(CORE_CLOCK_MHZ.mhz()).freeze();

        // If using ITM for debugging output
        //cx.core.DCB.enable_trace();

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
        let sck = gpioa.pa5;
        let mosi = gpioa.pa7;
        let pins = (sck, NoPin, mosi);

        // Clock setup in the stm32f4xx hal is currently a bit naff - it configures a clock which
        // is close to the requested clock, based on what it can attain as a result of the way that
        // other clocks in the microcontroller are setup (using a frequency divider).  There is no
        // way to optimise other clocks outside of the SPI peripheral to attain a specific SPI
        // frequency, and there is no way to find out what actual frequency has been acheived.
        // This is usually good enough with SPI devices (because there is a separate clock signal
        // which the peripheral uses to synchronise to), but with the LED drivers we are using,
        // this doesn't work because they don't use a clock signal, but instead work in a narrow
        // frequency band, and use the content of the data signals themselves to synchronise
        // instead.  With the core clock set to 56 MHz, the SPI peripheral will set itself to:
        //
        // 56 MHz / 16 = 3.5 MHz
        //
        // ... which works for me (tm).
        //
        // Further reading:
        //
        // https://docs.rs/stm32f4xx-hal/latest/stm32f4xx_hal/rcc/index.html
        //
        // see also:
        //
        // https://github.com/stm32-rs/stm32f4xx-hal/issues/394
        let spi = Spi::new(
            dp.SPI1,
            pins,
            ws2812::MODE,
            // Setup SPI clock to run at 3.5 MHz to keep WS2811s happy.
            3500.khz(),
            &clocks,
        );

        let ws = Ws2812::new(spi);

        lights_on::spawn().expect("failed schedule initial lights on");
        let mono = Timer::new(dp.TIM2, &clocks).monotonic();

        (Shared { ws }, Local {}, init::Monotonics(mono))
    }

    #[task(shared = [ws])]
    fn lights_on(mut cx: lights_on::Context) {
        defmt::println!("ON");

        let blue = RGB8 {
            b: 0xa0,
            g: 0,
            r: 0,
        };
        let data = [blue; NUM_LEDS];

        cx.shared.ws.lock(|ws| {
            ws.write(data.iter().cloned())
                .expect("Failed to write lights_on");
        });

        lights_off::spawn_after(1500.millis()).expect("Failed to schedule lights_off");
    }

    #[task(shared = [ws])]
    fn lights_off(mut cx: lights_off::Context) {
        defmt::println!("OFF");

        let empty = [RGB8::default(); NUM_LEDS];
        cx.shared.ws.lock(|ws| {
            ws.write(empty.iter().cloned())
                .expect("Failed to write lights_off");
        });

        lights_on::spawn_after(1500.millis()).expect("Failed to schedule lights_on");
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        loop {
            asm::nop();
        }
    }
}
