use argh::FromArgs;
use evdev_rs::enums::{EventCode, EV_KEY, EV_SYN, EV_ABS, InputProp};
use evdev_rs::{DeviceWrapper as _, InputEvent, UInputDevice};
use evdev_utils::{DeviceWrapperExt as _};
use futures::{TryStreamExt as _};
use log::{debug, info, trace};
use evdev_utils::AsyncDevice;

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

const MAX_STICK: i32 = 1000;
const MAX_TRIGGER: i32 = 140;

const STICK_ABSINFO: libc::input_absinfo = libc::input_absinfo {
    value: 0,
    minimum: -1590,
    maximum: 1590,
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
}

#[derive(Clone, Copy)]
struct State {
    x: i32,
    y: i32,
    m: Mod,
}

impl State {
    fn coord(&self) -> (i32, i32) {
        let &State { x, y, m } = self;
        match m {
            Mod::Null => {
                if x != 0 && y != 0 {
                    (700*x, 700*y)
                } else {
                    (x*MAX_STICK, y*MAX_STICK)
                }
            }
            Mod::X => {
                (x*737, y*if x == 0 { 650 } else { 288 })
            }
            Mod::Y => {
                (x*288, if x == 0 {
                    y*662
                } else {
                    y*650
                })
            }
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
    let () = uninit_device.enable(&InputProp::INPUT_PROP_BUTTONPAD).expect("enable buttonpad pty");
    let () = uninit_device.enable_gamepad().expect("failed to enable gamepad functionality");
    let () = uninit_device.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_X), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    let () = uninit_device.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_Y), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    let () = uninit_device.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_RX), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    let () = uninit_device.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_RY), Some(&STICK_ABSINFO))
        .expect("failed to enable ABS axis");
    let () = uninit_device.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_Z), Some(&TRIGGER_ABSINFO))
        .expect("failed to enable ABS trigger");
    let () = uninit_device.enable_event_code(&EventCode::EV_ABS(EV_ABS::ABS_RZ), Some(&TRIGGER_ABSINFO))
        .expect("failed to enable ABS trigger");
    let l = UInputDevice::create_from_device(&uninit_device).expect("create uinput device");

    let keeb_device = AsyncDevice::new(keeb_path).expect("failed to create keyboard device");
    //let () = keeb_device.grab(evdev_rs::GrabMode::Grab).expect("failed to grab keyboard");
    let mut state = State {
        m: Mod::Null,
        x: 0,
        y: 0,
    };

    let () = futures::executor::block_on(keeb_device.try_for_each(|event| {
        log_event(&event);
        let InputEvent {
            time: _,
            event_code,
            value,
        } = event;
        if value == 2 {
            return futures::future::ready(Ok(()));
        }
        futures::future::ready(match event_code {
            // left stick
            EventCode::EV_KEY(EV_KEY::KEY_A) => {
                state.y = value;
                send_stick(&l, state.coord())
            }
            EventCode::EV_KEY(EV_KEY::KEY_O) => {
                state.x = -value;
                send_stick(&l, state.coord())
            }
            EventCode::EV_KEY(EV_KEY::KEY_E) => {
                state.y = -value;
                send_stick(&l, state.coord())
            }
            EventCode::EV_KEY(EV_KEY::KEY_U) => {
                state.x = value;
                send_stick(&l, state.coord())
            }
            // modifiers
            EventCode::EV_KEY(EV_KEY::KEY_ENTER) => {
                state.m = if value == 1 { Mod::X } else { Mod::Null };
                send_stick(&l, state.coord())
            }
            EventCode::EV_KEY(EV_KEY::KEY_TAB) => {
                state.m = if value == 1 { Mod::Y } else { Mod::Null };
                send_stick(&l, state.coord())
            }
            // B
            EventCode::EV_KEY(EV_KEY::KEY_S) => {
                send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_EAST), value)
            }
            // X
            EventCode::EV_KEY(EV_KEY::KEY_N) => {
                send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_WEST), value)
            }
            // R
            EventCode::EV_KEY(EV_KEY::KEY_T) => {
                send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_Z), value * MAX_TRIGGER)
            }
            // lightest shield possible
            EventCode::EV_KEY(EV_KEY::KEY_W) => {
                send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_Z), value * 43)
            }
            // medium shield
            EventCode::EV_KEY(EV_KEY::KEY_C) => {
                send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_Z), value * 92)
            }
            // Z
            EventCode::EV_KEY(EV_KEY::KEY_H) => {
                send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_Z), value)
            }
            // A
            EventCode::EV_KEY(EV_KEY::KEY_BACKSPACE) => {
                send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_SOUTH), value)
            }
            // C-stick
            EventCode::EV_KEY(EV_KEY::KEY_ESC) => {
                send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RY), value * MAX_STICK)
            }
            EventCode::EV_KEY(EV_KEY::KEY_RIGHT) => {
                send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RY), -value * MAX_STICK)
            }
            EventCode::EV_KEY(EV_KEY::KEY_LEFTSHIFT) => {
                send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RX), -value * MAX_STICK)
            }
            EventCode::EV_KEY(EV_KEY::KEY_SPACE) => {
                send_event(&l, EventCode::EV_ABS(EV_ABS::ABS_RX), value * MAX_STICK)
            }
            // start select
            EventCode::EV_KEY(EV_KEY::KEY_6) => {
                send_event(&l, EventCode::EV_KEY(EV_KEY::BTN_START), value)
            }
            _ => Ok(()),
        })
    })).expect("keyboard event stream ended");
}
