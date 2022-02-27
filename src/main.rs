#![deny(unused_results)]

use std::io::Write as _;

use argh::FromArgs;
use evdev_utils::AsyncDevice;
use futures::{StreamExt as _, TryStreamExt as _};
use log::{debug, info, trace};

#[derive(FromArgs)]
/// Hako input remapping arguments.
struct Args {
    /// log level
    #[argh(option, short = 'l', default = "log::LevelFilter::Info")]
    log_level: log::LevelFilter,
}

fn log_event(event: &evdev_rs::InputEvent) {
    use evdev_rs::enums::EventCode;
    match event.event_code {
        EventCode::EV_MSC(_) | EventCode::EV_SYN(_) | EventCode::EV_REL(_) => {
            trace!("event: {:?}", event)
        }
        _ => debug!("event: {:?}", event),
    }
}

enum B0xxButton {
    A,
    B,
    L,
    R,
    X,
    Y,
    Z,
    LS,
    MS,
    Start,
    Up,
    Down,
    Left,
    Right,
    CUp,
    CDown,
    CLeft,
    CRight,
    ModX,
    ModY,
}

type Sign = bounded_integer::BoundedI8<-1, 1>;
const NEGATIVE: Sign = unsafe { Sign::new_unchecked(-1) };
const ZERO: Sign = unsafe { Sign::new_unchecked(0) };
const POSITIVE: Sign = unsafe { Sign::new_unchecked(1) };

enum GCButton {
    A,
    B,
    L,
    R,
    X,
    Y,
    Z,
    Start,
}

impl GCButton {
    fn pipe_input_name(&self) -> &'static str {
        match *self {
            Self::A => "A",
            Self::B => "B",
            Self::L => "L",
            Self::R => "R",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::Start => "START",
        }
    }
}

enum GCStick {
    A,
    C,
}

enum GCTrigger {
    L,
    R,
}

const ANALOG_MAX: i8 = 80;
const ANALOG_MIN: i8 = -80;
type I8analog = bounded_integer::BoundedI8<ANALOG_MIN, ANALOG_MAX>;
const I00000: I8analog = unsafe { I8analog::new_unchecked(0) };
const I00125: I8analog = unsafe { I8analog::new_unchecked(1) };
const I00250: I8analog = unsafe { I8analog::new_unchecked(2) };
const I00375: I8analog = unsafe { I8analog::new_unchecked(3) };
const I00500: I8analog = unsafe { I8analog::new_unchecked(4) };
const I00625: I8analog = unsafe { I8analog::new_unchecked(5) };
const I00750: I8analog = unsafe { I8analog::new_unchecked(6) };
const I00875: I8analog = unsafe { I8analog::new_unchecked(7) };
const I01000: I8analog = unsafe { I8analog::new_unchecked(8) };
const I01125: I8analog = unsafe { I8analog::new_unchecked(9) };
const I01250: I8analog = unsafe { I8analog::new_unchecked(10) };
const I01375: I8analog = unsafe { I8analog::new_unchecked(11) };
const I01500: I8analog = unsafe { I8analog::new_unchecked(12) };
const I01625: I8analog = unsafe { I8analog::new_unchecked(13) };
const I01750: I8analog = unsafe { I8analog::new_unchecked(14) };
const I01875: I8analog = unsafe { I8analog::new_unchecked(15) };
const I02000: I8analog = unsafe { I8analog::new_unchecked(16) };
const I02125: I8analog = unsafe { I8analog::new_unchecked(17) };
const I02250: I8analog = unsafe { I8analog::new_unchecked(18) };
const I02375: I8analog = unsafe { I8analog::new_unchecked(19) };
const I02500: I8analog = unsafe { I8analog::new_unchecked(20) };
const I02625: I8analog = unsafe { I8analog::new_unchecked(21) };
const I02750: I8analog = unsafe { I8analog::new_unchecked(22) };
const I02875: I8analog = unsafe { I8analog::new_unchecked(23) };
const I03000: I8analog = unsafe { I8analog::new_unchecked(24) };
const I03125: I8analog = unsafe { I8analog::new_unchecked(25) };
const I03250: I8analog = unsafe { I8analog::new_unchecked(26) };
const I03375: I8analog = unsafe { I8analog::new_unchecked(27) };
const I03500: I8analog = unsafe { I8analog::new_unchecked(28) };
const I03625: I8analog = unsafe { I8analog::new_unchecked(29) };
const I03750: I8analog = unsafe { I8analog::new_unchecked(30) };
const I03875: I8analog = unsafe { I8analog::new_unchecked(31) };
const I04000: I8analog = unsafe { I8analog::new_unchecked(32) };
const I04125: I8analog = unsafe { I8analog::new_unchecked(33) };
const I04250: I8analog = unsafe { I8analog::new_unchecked(34) };
const I04375: I8analog = unsafe { I8analog::new_unchecked(35) };
const I04500: I8analog = unsafe { I8analog::new_unchecked(36) };
const I04625: I8analog = unsafe { I8analog::new_unchecked(37) };
const I04750: I8analog = unsafe { I8analog::new_unchecked(38) };
const I04875: I8analog = unsafe { I8analog::new_unchecked(39) };
const I05000: I8analog = unsafe { I8analog::new_unchecked(40) };
const I05125: I8analog = unsafe { I8analog::new_unchecked(41) };
const I05250: I8analog = unsafe { I8analog::new_unchecked(42) };
const I05375: I8analog = unsafe { I8analog::new_unchecked(43) };
const I05500: I8analog = unsafe { I8analog::new_unchecked(44) };
const I05625: I8analog = unsafe { I8analog::new_unchecked(45) };
const I05750: I8analog = unsafe { I8analog::new_unchecked(46) };
const I05875: I8analog = unsafe { I8analog::new_unchecked(47) };
const I06000: I8analog = unsafe { I8analog::new_unchecked(48) };
const I06125: I8analog = unsafe { I8analog::new_unchecked(49) };
const I06250: I8analog = unsafe { I8analog::new_unchecked(50) };
const I06375: I8analog = unsafe { I8analog::new_unchecked(51) };
const I06500: I8analog = unsafe { I8analog::new_unchecked(52) };
const I06625: I8analog = unsafe { I8analog::new_unchecked(53) };
const I06750: I8analog = unsafe { I8analog::new_unchecked(54) };
const I06875: I8analog = unsafe { I8analog::new_unchecked(55) };
const I07000: I8analog = unsafe { I8analog::new_unchecked(56) };
const I07125: I8analog = unsafe { I8analog::new_unchecked(57) };
const I07250: I8analog = unsafe { I8analog::new_unchecked(58) };
const I07375: I8analog = unsafe { I8analog::new_unchecked(59) };
const I07500: I8analog = unsafe { I8analog::new_unchecked(60) };
const I07625: I8analog = unsafe { I8analog::new_unchecked(61) };
const I07750: I8analog = unsafe { I8analog::new_unchecked(62) };
const I07875: I8analog = unsafe { I8analog::new_unchecked(63) };
const I08000: I8analog = unsafe { I8analog::new_unchecked(64) };
const I08125: I8analog = unsafe { I8analog::new_unchecked(65) };
const I08250: I8analog = unsafe { I8analog::new_unchecked(66) };
const I08375: I8analog = unsafe { I8analog::new_unchecked(67) };
const I08500: I8analog = unsafe { I8analog::new_unchecked(68) };
const I08625: I8analog = unsafe { I8analog::new_unchecked(69) };
const I08750: I8analog = unsafe { I8analog::new_unchecked(70) };
const I08875: I8analog = unsafe { I8analog::new_unchecked(71) };
const I09000: I8analog = unsafe { I8analog::new_unchecked(72) };
const I09125: I8analog = unsafe { I8analog::new_unchecked(73) };
const I09250: I8analog = unsafe { I8analog::new_unchecked(74) };
const I09375: I8analog = unsafe { I8analog::new_unchecked(75) };
const I09500: I8analog = unsafe { I8analog::new_unchecked(76) };
const I09625: I8analog = unsafe { I8analog::new_unchecked(77) };
const I09750: I8analog = unsafe { I8analog::new_unchecked(78) };
const I09875: I8analog = unsafe { I8analog::new_unchecked(79) };
const I10000: I8analog = unsafe { I8analog::new_unchecked(80) };

const TRIGGER_MAX: u8 = 140;
type U8trigger = bounded_integer::BoundedU8<0, TRIGGER_MAX>;

enum Output {
    Button(GCButton, bool),
    Stick(GCStick, I8analog, I8analog),
    Trigger(GCTrigger, U8trigger),
}

struct B0xxEvent {
    time: libc::timeval,
    b0xx_btn: B0xxButton,
    pressed: bool,
}

struct Remapper;

impl Remapper {
    fn keyboard_to_b0xx(&self, c: evdev_rs::enums::EventCode) -> Option<B0xxButton> {
        use evdev_rs::enums::{EventCode, EV_KEY};
        match c {
            EventCode::EV_KEY(EV_KEY::KEY_O) => Some(B0xxButton::Left),
            EventCode::EV_KEY(EV_KEY::KEY_E) => Some(B0xxButton::Down),
            EventCode::EV_KEY(EV_KEY::KEY_U) => Some(B0xxButton::Right),
            EventCode::EV_KEY(EV_KEY::KEY_Z) => Some(B0xxButton::Up),
            EventCode::EV_KEY(EV_KEY::KEY_LEFTSHIFT) => Some(B0xxButton::ModX),
            EventCode::EV_KEY(EV_KEY::KEY_LEFTCTRL) => Some(B0xxButton::ModY),
            EventCode::EV_KEY(EV_KEY::KEY_SPACE) => Some(B0xxButton::A),
            EventCode::EV_KEY(EV_KEY::KEY_H) => Some(B0xxButton::B),
            EventCode::EV_KEY(EV_KEY::KEY_Y) | EventCode::EV_KEY(EV_KEY::KEY_F) => {
                Some(B0xxButton::Start)
            }
            _ => None,
        }
    }

    fn evdev_to_b0xx(
        &self,
        evdev_rs::InputEvent {
            time,
            event_code,
            value,
        }: evdev_rs::InputEvent,
    ) -> Option<B0xxEvent> {
        if value == 2 {
            return None;
        }
        Some(B0xxEvent {
            time: time.as_raw(),
            pressed: value == 1,
            b0xx_btn: self.keyboard_to_b0xx(event_code)?,
        })
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    struct Digital: u32 {
        const A = 0x00001;
        const B = 0x00002;
        const L = 0x00004;
        const R = 0x00008;
        const X = 0x00010;
        const Y = 0x00020;
        const Z = 0x00040;
        const LS = 0x00080;
        const MS = 0x00100;
        const START = 0x00200;
        const MOD_X = 0x00400;
        const MOD_Y = 0x00800;
    }
}

bitflags::bitflags! {
    #[derive(Default)]
    struct Axis: u8 {
        const P = 0x1;
        const N = 0x2;
    }
}

impl Axis {
    fn sign(&self) -> Option<bool> {
        match *self {
            Self::P => Some(true),
            Self::N => Some(false),
            _ => None,
        }
    }
}

#[derive(Default)]
struct Main {
    digital: Digital,
    ax: Axis,
    ay: Axis,
    cx: Axis,
    cy: Axis,
}

impl Main {
    fn b0xx_to_gc(
        &mut self,
        B0xxEvent {
            time: _,
            b0xx_btn,
            pressed,
        }: B0xxEvent,
    ) -> Output {
        match b0xx_btn {
            B0xxButton::A => {
                self.digital.set(Digital::A, pressed);
                return Output::Button(GCButton::A, pressed);
            }
            B0xxButton::B => {
                self.digital.set(Digital::B, pressed);
                return Output::Button(GCButton::B, pressed);
            }
            B0xxButton::Start => {
                self.digital.set(Digital::START, pressed);
                return Output::Button(GCButton::Start, pressed);
            }
            B0xxButton::Up => self.ay.set(Axis::P, pressed),
            B0xxButton::Down => self.ay.set(Axis::N, pressed),
            B0xxButton::Left => self.ax.set(Axis::N, pressed),
            B0xxButton::Right => self.ax.set(Axis::P, pressed),
            B0xxButton::ModX => self.digital.set(Digital::MOD_X, pressed),
            B0xxButton::ModY => self.digital.set(Digital::MOD_Y, pressed),
            _ => {}
        }
        match (self.ax.sign(), self.ay.sign()) {
            (None, None) => Output::Stick(GCStick::A, I00000, I00000),
            (Some(x), None) => {
                if self.digital.contains(Digital::MOD_X) {
                    Output::Stick(GCStick::A, if x { I06625 } else { -I06625 }, I00000)
                } else if self.digital.contains(Digital::MOD_Y) {
                    Output::Stick(GCStick::A, if x { I03375 } else { -I03375 }, I00000)
                } else {
                    Output::Stick(GCStick::A, if x { I10000 } else { -I10000 }, I00000)
                }
            }
            (None, Some(y)) => {
                if self.digital.contains(Digital::MOD_X) {
                    Output::Stick(GCStick::A, I00000, if y { I05375 } else { -I05375 })
                } else if self.digital.contains(Digital::MOD_Y) {
                    Output::Stick(GCStick::A, I00000, if y { I07375 } else { -I07375 })
                } else {
                    Output::Stick(GCStick::A, I00000, if y { I10000 } else { -I10000 })
                }
            }
            (Some(x), Some(y)) => {
                if self.digital.contains(Digital::MOD_X) {
                    Output::Stick(
                        GCStick::A,
                        if x { I07375 } else { -I07375 },
                        if y { I03125 } else { -I03125 },
                    )
                } else if self.digital.contains(Digital::MOD_Y) {
                    Output::Stick(
                        GCStick::A,
                        if x { I03125 } else { -I03125 },
                        if y { I07375 } else { -I07375 },
                    )
                } else {
                    Output::Stick(
                        GCStick::A,
                        if x { I07000 } else { -I07000 },
                        if y { I07000 } else { -I07000 },
                    )
                }
            }
        }
    }
}

struct OutputSink {
    file: std::fs::File,
}

impl OutputSink {
    fn send(&mut self, o: Output) -> anyhow::Result<()> {
        let cmd = match o {
            Output::Button(btn, pressed) => {
                if pressed {
                    Some(format!("PRESS {}\n", btn.pipe_input_name()))
                } else {
                    Some(format!("RELEASE {}\n", btn.pipe_input_name()))
                }
            }
            Output::Stick(stick, x, y) => match stick {
                GCStick::A => {
                    let x = x.get() as f64;
                    let y = y.get() as f64;
                    Some(format!(
                        "SET MAIN {} {}\n",
                        0.5 + 0.5 * if x < 0.0 { x / 128f64 } else { x / 127f64 },
                        0.5 + 0.5 * if y < 0.0 { y / 128f64 } else { y / 127f64 },
                    ))
                }
                GCStick::C => None,
            },
            Output::Trigger(_trigger, _v) => None,
        };
        if let Some(cmd) = cmd {
            debug!("writing: {}", cmd);
            let _ = self.file.write(cmd.as_bytes())?;
        }
        Ok(())
    }
}

fn main() {
    let Args { log_level } = argh::from_env();

    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .with_module_level(std::module_path!(), log_level)
        .init()
        .expect("failed to initialize logger");

    let keeb_path = futures::executor::block_on(evdev_utils::identify_keyboard())
        .expect("failed to identify keyboard");
    info!("found keyboard {:?}", keeb_path);

    let mut keeb_device = AsyncDevice::new(keeb_path)
        .expect("failed to create keyboard device")
        .fuse();

    let remapper = Remapper;
    let mut main = Main::default();
    let mut sink = OutputSink {
        file: std::fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open("/home/tone/.config/SlippiOnline/Pipes/pipe")
            .expect("failed to open pipe"),
    };
    let fut = async {
        loop {
            futures::select! {
                r = keeb_device.try_next() => {
                    let event = r.expect("keyboard event stream error")
                        .expect("keyboard event stream ended unexpectedly");
                    log_event(&event);
                    let e = match remapper.evdev_to_b0xx(event) {
                        Some(e) => e,
                        None => continue,
                    };
                    sink.send(main.b0xx_to_gc(e)).expect("failed to write to pipe");
                }
            }
        }
    };
    futures::executor::block_on(fut);
}
