use argh::FromArgs;
use evdev_rs::enums::{EventCode, InputProp, EV_ABS, EV_KEY, EV_SYN};
use evdev_rs::{DeviceWrapper as _, InputEvent, UInputDevice};
use evdev_utils::AsyncDevice;
use evdev_utils::DeviceWrapperExt as _;
use futures::TryStreamExt as _;
use log::{debug, info, trace};

#[derive(FromArgs)]
/// Hako input remapping arguments.
struct Args {
    /// log level
    #[argh(option, short = 'l', default = "log::LevelFilter::Info")]
    log_level: log::LevelFilter,
}

fn send_syn(l: &UInputDevice) -> std::io::Result<()> {
    l.write_event(&InputEvent {
        event_code: EventCode::EV_SYN(EV_SYN::SYN_REPORT),
        value: 0,
        time: evdev_rs::TimeVal {
            tv_sec: 0,
            tv_usec: 0,
        },
    })
}

fn send_event(l: &UInputDevice, event_code: EventCode, value: i32) -> std::io::Result<()> {
    let event = InputEvent {
        event_code,
        value,
        time: evdev_rs::TimeVal {
            tv_sec: 0,
            tv_usec: 0,
        },
    };
    info!("injecting event: {:?} {:?}", event_code, value);
    let () = l.write_event(&event)?;
    let () = send_syn(l)?;
    Ok(())
}

fn send_stick(l: &UInputDevice, xy: (i32, i32)) -> std::io::Result<()> {
    let (x, y) = xy;
    let () = send_event(l, EventCode::EV_ABS(EV_ABS::ABS_X), x.into())?;
    let () = send_event(l, EventCode::EV_ABS(EV_ABS::ABS_Y), y.into())?;
    let () = send_syn(l)?;
    Ok(())
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
    let Args { log_level } = argh::from_env();

    let () = simple_logger::SimpleLogger::new()
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
    let () = uninit_device
        .enable(&InputProp::INPUT_PROP_BUTTONPAD)
        .expect("enable buttonpad pty");
    let () = uninit_device
        .enable_gamepad()
        .expect("failed to enable gamepad functionality");
    let () = uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_X), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    let () = uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_Y), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    let () = uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_RX), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    let () = uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_RY), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    let () = uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_Z), Some(&TRIGGER_ABSINFO))
        .expect("failed to enable ABS trigger");
    let () = uninit_device
        .enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_RZ), Some(&TRIGGER_ABSINFO))
        .expect("failed to enable ABS trigger");
    let l = UInputDevice::create_from_device(&uninit_device).expect("create uinput device");

    let keeb_device = AsyncDevice::new(keeb_path).expect("failed to create keyboard device");
    //let () = keeb_device.grab(evdev_rs::GrabMode::Grab).expect("failed to grab keyboard");
    let mut state = State {
        m: Mod::Null,
        c: CMod::Null,
        x: 0,
        y: 0,
    };

    let () = futures::executor::block_on(keeb_device.try_for_each(|event| {
        futures::future::ready((|| {
            log_event(&event);
            let InputEvent {
                time: _,
                event_code,
                value,
            } = event;
            if value == 2 {
                return Ok(());
            }
            match event_code {
                // modifiers
                EventCode::EV_KEY(EV_KEY::KEY_A) => {
                    state.m = if value == 1 { Mod::X } else { Mod::Null };
                    send_stick(&l, state.coord())
                }
                EventCode::EV_KEY(EV_KEY::KEY_SEMICOLON) => {
                    state.m = if value == 1 { Mod::Y } else { Mod::Null };
                    send_stick(&l, state.coord())
                }
                EventCode::EV_KEY(EV_KEY::KEY_LEFTCTRL) => {
                    state.m = if value == 1 { Mod::Shield } else { Mod::Null };
                    send_stick(&l, state.coord())
                }
                // left stick
                EventCode::EV_KEY(EV_KEY::KEY_O) => {
                    if !(value == 0 && state.x == 1) {
                        state.x = -value;
                    }
                    send_stick(&l, state.coord())
                }
                EventCode::EV_KEY(EV_KEY::KEY_E) => {
                    if !(value == 0 && state.y == 1) {
                        state.y = -value;
                    }
                    send_stick(&l, state.coord())
                }
                EventCode::EV_KEY(EV_KEY::KEY_U) => {
                    if !(value == 0 && state.x == -1) {
                        state.x = value;
                    }
                    send_stick(&l, state.coord())
                }
                EventCode::EV_KEY(EV_KEY::KEY_ENTER) => {
                    if !(value == 0 && state.y == -1) {
                        state.y = value;
                    }
                    send_stick(&l, state.coord())
                }
                // dpad
                EventCode::EV_KEY(EV_KEY::KEY_Q) => {
                    send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_DPAD_LEFT), value)
                }
                EventCode::EV_KEY(EV_KEY::KEY_J) => {
                    send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_DPAD_UP), value)
                }
                EventCode::EV_KEY(EV_KEY::KEY_K) => {
                    send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_DPAD_RIGHT), value)
                }
                EventCode::EV_KEY(EV_KEY::KEY_TAB) => {
                    send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_DPAD_DOWN), value)
                }
                // Start
                EventCode::EV_KEY(EV_KEY::KEY_Y) => {
                    send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_START), value)
                }

                // L
                EventCode::EV_KEY(EV_KEY::KEY_S) => {
                    send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_Z), value * MAX_TRIGGER)
                }
                // lightest shield possible
                EventCode::EV_KEY(EV_KEY::KEY_Z) => {
                    send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RZ), value * 49)
                }
                // medium shield
                EventCode::EV_KEY(EV_KEY::KEY_RIGHTCTRL) => {
                    send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RZ), value * 92)
                }
                // X
                EventCode::EV_KEY(EV_KEY::KEY_N) => {
                    send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_WEST), value)
                }
                // Y
                EventCode::EV_KEY(EV_KEY::KEY_V) => {
                    send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_NORTH), value)
                }
                // Z
                EventCode::EV_KEY(EV_KEY::KEY_T) => {
                    send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_Z), value)
                }
                // B
                EventCode::EV_KEY(EV_KEY::KEY_H) => {
                    send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_EAST), value)
                }
                // R
                EventCode::EV_KEY(EV_KEY::KEY_M) => {
                    send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RZ), value * MAX_TRIGGER)
                }
                // A
                EventCode::EV_KEY(EV_KEY::KEY_BACKSPACE) => {
                    send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_SOUTH), value)
                }
                // C-stick
                EventCode::EV_KEY(EV_KEY::KEY_ESC) => {
                    send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RY), value * P10000)
                }
                EventCode::EV_KEY(EV_KEY::KEY_RIGHT) => {
                    send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RY), -value * P10000)
                }
                EventCode::EV_KEY(EV_KEY::KEY_LEFTSHIFT) => {
                    send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RX), -value * P10000)
                }
                EventCode::EV_KEY(EV_KEY::KEY_SPACE) => {
                    state.c = if value == 1 { CMod::Right } else { CMod::Null };
                    let () = send_stick(&l, state.coord())?;
                    send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RX), value * P10000)
                }
                _ => Ok(()),
            }
        })())
    }))
    .expect("keyboard event stream ended");
}
