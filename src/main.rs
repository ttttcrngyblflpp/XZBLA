#![deny(unused_results)]

use argh::FromArgs;
use evdev_rs::enums::{EventCode, InputProp, EV_ABS, EV_KEY};
use evdev_rs::{DeviceWrapper as _, InputEvent, UInputDevice};
use evdev_utils::{AsyncDevice, DeviceWrapperExt as _, UInputExt as _};
use futures::{StreamExt as _, TryStreamExt as _};
use log::{debug, info, trace};

#[derive(FromArgs)]
/// Hako input remapping arguments.
struct Args {
    /// log level
    #[argh(option, short = 'l', default = "log::LevelFilter::Info")]
    log_level: log::LevelFilter,

    /// number of frames to delay actuation of X when the corresponding key is pressed
    #[argh(option, short = 'j', default = "0")]
    jump_delay_ms: u64,
}

fn log_event(event: &InputEvent) {
    match event.event_code {
        EventCode::EV_MSC(_) | EventCode::EV_SYN(_) | EventCode::EV_REL(_) => {
            trace!("event: {:?}", event)
        }
        _ => debug!("event: {:?}", event),
    }
}

const MAX_TRIGGER: i32 = 140;

const P2875: i32 = 23;
const P3000: i32 = 24;
const P3125: i32 = 25;
const P5625: i32 = 45;
const P6500: i32 = 52;
const P6750: i32 = 54;
const P7000: i32 = 56;
const P7375: i32 = 59;
const P8250: i32 = 66;
const P10000: i32 = 80;

const STICK_ABSINFO: libc::input_absinfo = libc::input_absinfo {
    value: 0,
    minimum: -127,
    maximum: 127,
    fuzz: 0,
    flat: 0,
    resolution: 0,
};

const TRIGGER_ABSINFO: libc::input_absinfo = libc::input_absinfo {
    value: 0,
    minimum: 0,
    maximum: 255,
    fuzz: 0,
    flat: 0,
    resolution: 0,
};

#[derive(Clone, Copy)]
enum Mod {
    Null,
    X,
    Y,
    Shield,
}

#[derive(Clone, Copy, Debug)]
enum CMod {
    Null,
    Right,
}

#[derive(Clone, Copy)]
struct State {
    x: i32,
    y: i32,
    m: Mod,
    c: CMod,
}

impl State {
    fn coord(&self) -> (i32, i32) {
        let &State { x, y, m, c } = self;
        match m {
            Mod::Null => {
                if x != 0 && y != 0 {
                    (x * P7000, y * P7000)
                } else {
                    (x * P10000, y * P10000)
                }
            }
            Mod::X => match c {
                CMod::Right if x != 0 && y != 0 => (x * P8250, y * P5625),
                CMod::Right | CMod::Null => (x * P7375, y * if x == 0 { P6500 } else { P3125 }),
            },
            Mod::Y => match c {
                CMod::Right if x != 0 && y != 0 => (x * P5625, y * P8250),
                CMod::Right | CMod::Null => (
                    x * if y == 0 { P2875 } else { P3000 },
                    y * if x == 0 { P6500 } else { P7000 },
                ),
            },
            Mod::Shield => (x * P6750, y * P6500),
        }
    }
}

fn main() {
    let Args {
        log_level,
        jump_delay_ms,
    } = argh::from_env();

    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .with_module_level(std::module_path!(), log_level)
        .init()
        .expect("failed to initialize logger");

    let keeb_path = futures::executor::block_on(evdev_utils::identify_keyboard())
        .expect("failed to identify keyboard and mouse");
    info!("found keyboard {:?}", keeb_path);

    let uninit_device = evdev_rs::UninitDevice::new().expect("failed to create uninit device");
    uninit_device.set_name("hako");
    uninit_device.set_bustype(3);
    uninit_device
        .enable(&InputProp::INPUT_PROP_BUTTONPAD)
        .expect("enable buttonpad pty");
    uninit_device
        .enable_gamepad()
        .expect("failed to enable gamepad functionality");
    uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_X), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_Y), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_RX), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_RY), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_Z), Some(&TRIGGER_ABSINFO))
        .expect("failed to enable ABS trigger");
    uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_RZ), Some(&TRIGGER_ABSINFO))
        .expect("failed to enable ABS trigger");
    let l = UInputDevice::create_from_device(&uninit_device).expect("create uinput device");

    let mut keeb_device = AsyncDevice::new(keeb_path)
        .expect("failed to create keyboard device")
        .fuse();
    let mut state = State {
        m: Mod::Null,
        c: CMod::Null,
        x: 0,
        y: 0,
    };

    let delayed_jump_fut = futures::future::Fuse::terminated();
    futures::pin_mut!(delayed_jump_fut);
    let fut = async {
        loop {
            futures::select! {
                i = delayed_jump_fut => {
                    let _: std::time::Instant = i;
                    l.inject_key_syn(EV_KEY::BTN_WEST, 1).unwrap();
                }
                r = keeb_device.try_next() => {
                    let event = r.expect("keyboard event stream error")
                        .expect("keyboard event stream ended unexpectedly");
                    log_event(&event);
                    let InputEvent {
                        time: _,
                        event_code,
                        value,
                    } = event;
                    if value == 2 {
                        continue;
                    }
                    match event_code {
                        // modifiers
                        EventCode::EV_KEY(EV_KEY::KEY_SEMICOLON) => {
                            state.m = if value == 1 { Mod::X } else { Mod::Null };
                            l.inject_xy((EV_ABS::ABS_X, EV_ABS::ABS_Y), state.coord()).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_A) => {
                            state.m = if value == 1 { Mod::Y } else { Mod::Null };
                            l.inject_xy((EV_ABS::ABS_X, EV_ABS::ABS_Y), state.coord()).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_TAB) => {
                            state.m = if value == 1 { Mod::Shield } else { Mod::Null };
                            l.inject_xy((EV_ABS::ABS_X, EV_ABS::ABS_Y), state.coord()).unwrap();
                        }
                        // left stick
                        EventCode::EV_KEY(EV_KEY::KEY_O) => {
                            if !(value == 0 && state.x == 1) {
                                state.x = -value;
                            }
                            l.inject_xy((EV_ABS::ABS_X, EV_ABS::ABS_Y), state.coord()).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_E) => {
                            if !(value == 0 && state.y == 1) {
                                state.y = -value;
                            }
                            l.inject_xy((EV_ABS::ABS_X, EV_ABS::ABS_Y), state.coord()).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_U) => {
                            if !(value == 0 && state.x == -1) {
                                state.x = value;
                            }
                            l.inject_xy((EV_ABS::ABS_X, EV_ABS::ABS_Y), state.coord()).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_LEFTSHIFT) => {
                            if !(value == 0 && state.y == -1) {
                                state.y = value;
                            }
                            l.inject_xy((EV_ABS::ABS_X, EV_ABS::ABS_Y), state.coord()).unwrap();
                        }
                        // dpad
                        EventCode::EV_KEY(EV_KEY::KEY_Q) => {
                            l.inject_key_syn(EV_KEY::BTN_DPAD_LEFT, value).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_J) => {
                            l.inject_key_syn(EV_KEY::BTN_DPAD_UP, value).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_K) => {
                            l.inject_key_syn(EV_KEY::BTN_DPAD_RIGHT, value).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_LEFTCTRL) => {
                            l.inject_key_syn(EV_KEY::BTN_DPAD_DOWN, value).unwrap();
                        }
                        // Start
                        EventCode::EV_KEY(EV_KEY::KEY_Y) => {
                            l.inject_key_syn(EV_KEY::BTN_START, value).unwrap();
                        }
                        // X
                        EventCode::EV_KEY(EV_KEY::KEY_Z) if value == 1 => {
                            if jump_delay_ms == 0 {
                                l.inject_key_syn(EV_KEY::BTN_WEST, 1).unwrap();
                            } else {
                                delayed_jump_fut.set(futures::FutureExt::fuse(async_io::Timer::after(std::time::Duration::from_millis(jump_delay_ms))));
                            }
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_Z) if value == 0 => {
                            l.inject_key_syn(EV_KEY::BTN_WEST, 0).unwrap();
                        }
                        // Y
                        EventCode::EV_KEY(EV_KEY::KEY_S) => {
                            l.inject_key_syn(EV_KEY::BTN_NORTH, value).unwrap();
                        }
                        // Z
                        EventCode::EV_KEY(EV_KEY::KEY_N) => {
                            l.inject_key_syn(EV_KEY::BTN_Z, value).unwrap();
                        }
                        // B
                        EventCode::EV_KEY(EV_KEY::KEY_T) => {
                            l.inject_key_syn(EV_KEY::BTN_EAST, value).unwrap();
                        }
                        // R
                        EventCode::EV_KEY(EV_KEY::KEY_C) => {
                            l.inject_abs_syn(EV_ABS::ABS_RZ, value * MAX_TRIGGER).unwrap();
                        }
                        // L
                        EventCode::EV_KEY(EV_KEY::KEY_H) => {
                            l.inject_abs_syn(EV_ABS::ABS_Z, value * MAX_TRIGGER).unwrap();
                        }
                        // lightest shield possible
                        EventCode::EV_KEY(EV_KEY::KEY_M) => {
                            l.inject_abs_syn(EV_ABS::ABS_RZ, value * 49).unwrap();
                        }
                        // medium shield
                        EventCode::EV_KEY(EV_KEY::KEY_G) => {
                            l.inject_abs_syn(EV_ABS::ABS_RZ, value * 92).unwrap();
                        }
                        // A
                        EventCode::EV_KEY(EV_KEY::KEY_SPACE) => {
                            state.c = if value == 1 { CMod::Right } else { CMod::Null };
                            l.inject_xy((EV_ABS::ABS_X, EV_ABS::ABS_Y), state.coord()).unwrap();
                            l.inject_key_syn(EV_KEY::BTN_SOUTH, value).unwrap();
                        }
                        // C-stick
                        EventCode::EV_KEY(EV_KEY::KEY_ESC) => {
                            l.inject_abs_syn(EV_ABS::ABS_RY, value * P10000).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_RIGHT) => {
                            l.inject_abs_syn(EV_ABS::ABS_RY, -value * P10000).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_BACKSPACE) => {
                            l.inject_abs_syn(EV_ABS::ABS_RX, -value * P10000).unwrap();
                        }
                        EventCode::EV_KEY(EV_KEY::KEY_RIGHTSHIFT) => {
                            l.inject_abs_syn(EV_ABS::ABS_RX, value * P10000).unwrap();
                        }
                        _ => {}
                    }
                }
            }
        }
    };
    futures::executor::block_on(fut);
}
