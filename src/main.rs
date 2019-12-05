#![no_main]
#![no_std]

// The aim here is for me to be able to write an effect,
// and not have to think about how it's scheduled and how lights are actually updated.
//
// As an effect, I write:
// - the `State` I care about; and
// - provide a state transition function.
//
// The result of the transition is:
// - some new state; plus
// -a bunch of actions to carry out (light on, light off).
//
// The RTFM init sets all this up, and interprets the actions into changes to the LED array.
//

extern crate panic_semihosting;

use stm32f4xx_hal as hal;
use ws2812_spi as ws2812;

use hal::spi::*;
use hal::{prelude::*, stm32};

use ws2812::Ws2812;

use smart_leds::SmartLedsWrite;
use smart_leds_trait::RGB8;

use cortex_m::iprintln;
use cortex_m_semihosting::hprintln;

use rtfm::cyccnt::U32Ext;

// How often to schedule an update (e.g., 8th of a second)
const PERIOD: u32 = 48_000_000 / 8;

// Index 0 to 49
const NUM_LEDS: usize = 50;
const LAST_LED: usize = NUM_LEDS - 1;

// Types for WS
use hal::gpio::gpiob::{PB3, PB5};
use hal::gpio::{Alternate, AF5};
use hal::spi::{NoMiso, Spi};
use hal::stm32::SPI1;

type Pins = (PB3<Alternate<AF5>>, NoMiso, PB5<Alternate<AF5>>);

//
// Application-specific types and functions
//

#[derive(Clone, Debug)]
enum Direction {
    Up,
    Down,
}

#[derive(Debug, Clone)]
pub struct State {
    idx: usize,
    dir: Direction,
}

enum Position {
    AtStart,
    AtEnd,
    SomewhereBetween,
}

impl Position {
    fn of(idx: usize) -> Position {
        match idx {
            0 => Position::AtStart,
            LAST_LED => Position::AtEnd,
            _ => Position::SomewhereBetween,
        }
    }
}

fn next_state(state: &State) -> (State, [Action; 2]) {
    let dir = match (Position::of(state.idx), &state.dir) {
        (Position::AtEnd, Direction::Up) => Direction::Down,
        (Position::AtStart, Direction::Down) => Direction::Up,
        (_, dir) => dir.clone(),
    };

    let idx = match dir {
        Direction::Up => state.idx + 1,
        Direction::Down => state.idx - 1,
    };

    let new_state = State { idx, dir };

    let colour = BLUE;

    let actions = [Action::Off { idx: state.idx }, Action::On { idx, colour }];

    (new_state, actions)
}

//
// Glue language for communicating changs to the LED scheduled controller
//

#[derive(Debug)]
enum Action {
    On { idx: usize, colour: RGB8 },
    Off { idx: usize },
}

const BLACK: RGB8 = RGB8 { r: 0, g: 0, b: 0 };
const BLUE: RGB8 = RGB8 {
    r: 0,
    g: 0,
    b: 0xA0,
};

//
// Ideally evertything below here would be not tied to the specific application
// (effect) I'm trying to run.
// But it's not:  there's some set up in init to do (which is fair enoough)
// and a few TODOs to sort out
//

#[rtfm::app(device = stm32f4xx_hal::stm32, peripherals = true, monotonic = rtfm::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
        ws: Ws2812<Spi<SPI1, Pins>>,
        itm: cortex_m::peripheral::ITM,
        data: [RGB8; NUM_LEDS],
        state: State,
    }

    #[init(schedule = [step])]
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
            .step(cx.start + PERIOD.cycles())
            .expect("failed schedule initial step");

        let state = State {
            idx: 0,
            dir: Direction::Up,
        };

        let mut data = [BLACK; NUM_LEDS];
        data[0] = BLUE;

        init::LateResources {
            ws,
            itm,
            data,
            state,
        }
    }

    #[task(schedule = [step], resources = [ws, itm, data, state])]
    fn step(cx: step::Context) {
        let step::Resources {
            ws,
            itm,
            mut state,
            data,
        } = cx.resources;
        // let port = &mut itm.stim[0];

        // hprintln!("step").unwrap();

        // Render the `data` array, and in a moment we'll compute the next `data` state:
        ws.write(data.iter().cloned())
            .expect("Failed to write lights");

        let (next_state, actions) = next_state(&state);
        // hprintln!(" - state in: {:?}", state);
        // hprintln!(" - state out: {:?}", next_state);

        // Action interpreter, updating the data state:
        for action in actions.iter() {
            // hprintln!("- {:?}", action).unwrap();
            match action {
                Action::Off { idx } => data[*idx] = BLACK,
                Action::On { idx, colour } => data[*idx] = *colour,
            }
        }

        // TODO: too specific - this should be some kind of clone() or assignment
        // that doesn't care about what's inside of state
        state.idx = next_state.idx;
        state.dir = next_state.dir;

        cx.schedule
            .step(cx.scheduled + PERIOD.cycles())
            .expect("Failed to schedule step");
    }

    extern "C" {
        fn USART1();
    }
};
