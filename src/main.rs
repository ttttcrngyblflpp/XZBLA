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
    /// enable crouch/walk option-select
    #[argh(switch)]
    crouch_walk_option_select: bool,
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

struct Remapper;

impl Remapper {
    fn keyboard_to_b0xx(&self, c: evdev_rs::enums::EventCode) -> Option<B0xx> {
        use evdev_rs::enums::{EventCode, EV_KEY};
        match c {
            EventCode::EV_KEY(EV_KEY::KEY_SEMICOLON) => {
                Some(B0xx::Impure(Impure::Button(ButtonImpure::L)))
            }
            EventCode::EV_KEY(EV_KEY::KEY_O) => {
                Some(B0xx::Impure(Impure::Stick(Stick::A, Axis::X, NEGATIVE)))
            }
            EventCode::EV_KEY(EV_KEY::KEY_E) => {
                Some(B0xx::Impure(Impure::Stick(Stick::A, Axis::Y, NEGATIVE)))
            }
            EventCode::EV_KEY(EV_KEY::KEY_U) => {
                Some(B0xx::Impure(Impure::Stick(Stick::A, Axis::X, POSITIVE)))
            }
            EventCode::EV_KEY(EV_KEY::KEY_LEFTSHIFT) => Some(B0xx::Impure(Impure::ModX)),
            EventCode::EV_KEY(EV_KEY::KEY_LEFTCTRL) => Some(B0xx::Impure(Impure::ModY)),
            EventCode::EV_KEY(EV_KEY::KEY_Y) | EventCode::EV_KEY(EV_KEY::KEY_F) => {
                Some(B0xx::Pure(Pure::Button(ButtonPure::Start)))
            }
            EventCode::EV_KEY(EV_KEY::KEY_G) => Some(B0xx::Impure(Impure::Button(ButtonImpure::R))),
            EventCode::EV_KEY(EV_KEY::KEY_C) => Some(B0xx::Pure(Pure::Button(ButtonPure::Y))),
            EventCode::EV_KEY(EV_KEY::KEY_R) => Some(B0xx::Pure(Pure::Shield(Shield::Light))),
            EventCode::EV_KEY(EV_KEY::KEY_S) => Some(B0xx::Pure(Pure::Shield(Shield::Medium))),
            EventCode::EV_KEY(EV_KEY::KEY_H) => Some(B0xx::Impure(Impure::Button(ButtonImpure::B))),
            EventCode::EV_KEY(EV_KEY::KEY_T) => Some(B0xx::Pure(Pure::Button(ButtonPure::X))),
            EventCode::EV_KEY(EV_KEY::KEY_N) => Some(B0xx::Pure(Pure::Button(ButtonPure::Z))),
            EventCode::EV_KEY(EV_KEY::KEY_Z) => {
                Some(B0xx::Impure(Impure::Stick(Stick::A, Axis::Y, POSITIVE)))
            }
            EventCode::EV_KEY(EV_KEY::KEY_ESC) => {
                Some(B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, NEGATIVE)))
            }
            EventCode::EV_KEY(EV_KEY::KEY_BACKSPACE) => {
                Some(B0xx::Impure(Impure::Stick(Stick::C, Axis::X, NEGATIVE)))
            }
            EventCode::EV_KEY(EV_KEY::KEY_DOWN) => {
                Some(B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, POSITIVE)))
            }
            EventCode::EV_KEY(EV_KEY::KEY_ENTER) => {
                Some(B0xx::Impure(Impure::Stick(Stick::C, Axis::X, POSITIVE)))
            }
            EventCode::EV_KEY(EV_KEY::KEY_SPACE) => Some(B0xx::Pure(Pure::Button(ButtonPure::A))),
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
            btn: self.keyboard_to_b0xx(event_code)?,
        })
    }
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum Button {
    Pure(ButtonPure),
    Impure(ButtonImpure),
    DPad(Axis, Direction),
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum ButtonPure {
    A,
    X,
    Y,
    Z,
    Start,
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum ButtonImpure {
    B,
    L,
    R,
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum Axis {
    X,
    Y,
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum Pure {
    Button(ButtonPure),
    Shield(Shield),
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum Impure {
    Button(ButtonImpure),
    Stick(Stick, Axis, Direction),
    ModX,
    ModY,
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum B0xx {
    Pure(Pure),
    Impure(Impure),
}

struct B0xxEvent {
    time: libc::timeval,
    btn: B0xx,
    pressed: Pressed,
}

impl B0xxEvent {
    #[cfg(test)]
    fn new_without_time(btn: B0xx, pressed: Pressed) -> Self {
        Self {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            btn,
            pressed,
        }
    }
}

bounded_integer::bounded_integer! {
    enum Analog { -80..=80 }
}

#[allow(dead_code)]
mod consts {
    use super::Analog;

    pub(crate) const P0000: Analog = Analog::Z;
    pub(crate) const P0125: Analog = Analog::P1;
    pub(crate) const P0250: Analog = Analog::P2;
    pub(crate) const P0375: Analog = Analog::P3;
    pub(crate) const P0500: Analog = Analog::P4;
    pub(crate) const P0625: Analog = Analog::P5;
    pub(crate) const P0750: Analog = Analog::P6;
    pub(crate) const P0875: Analog = Analog::P7;
    pub(crate) const P1000: Analog = Analog::P8;
    pub(crate) const P1125: Analog = Analog::P9;
    pub(crate) const P1250: Analog = Analog::P10;
    pub(crate) const P1375: Analog = Analog::P11;
    pub(crate) const P1500: Analog = Analog::P12;
    pub(crate) const P1625: Analog = Analog::P13;
    pub(crate) const P1750: Analog = Analog::P14;
    pub(crate) const P1875: Analog = Analog::P15;
    pub(crate) const P2000: Analog = Analog::P16;
    pub(crate) const P2125: Analog = Analog::P17;
    pub(crate) const P2250: Analog = Analog::P18;
    pub(crate) const P2375: Analog = Analog::P19;
    pub(crate) const P2500: Analog = Analog::P20;
    pub(crate) const P2625: Analog = Analog::P21;
    pub(crate) const P2750: Analog = Analog::P22;
    pub(crate) const P2875: Analog = Analog::P23;
    pub(crate) const P3000: Analog = Analog::P24;
    pub(crate) const P3125: Analog = Analog::P25;
    pub(crate) const P3250: Analog = Analog::P26;
    pub(crate) const P3375: Analog = Analog::P27;
    pub(crate) const P3500: Analog = Analog::P28;
    pub(crate) const P3625: Analog = Analog::P29;
    pub(crate) const P3750: Analog = Analog::P30;
    pub(crate) const P3875: Analog = Analog::P31;
    pub(crate) const P4000: Analog = Analog::P32;
    pub(crate) const P4125: Analog = Analog::P33;
    pub(crate) const P4250: Analog = Analog::P34;
    pub(crate) const P4375: Analog = Analog::P35;
    pub(crate) const P4500: Analog = Analog::P36;
    pub(crate) const P4625: Analog = Analog::P37;
    pub(crate) const P4750: Analog = Analog::P38;
    pub(crate) const P4875: Analog = Analog::P39;
    pub(crate) const P5000: Analog = Analog::P40;
    pub(crate) const P5125: Analog = Analog::P41;
    pub(crate) const P5250: Analog = Analog::P42;
    pub(crate) const P5375: Analog = Analog::P43;
    pub(crate) const P5500: Analog = Analog::P44;
    pub(crate) const P5625: Analog = Analog::P45;
    pub(crate) const P5750: Analog = Analog::P46;
    pub(crate) const P5875: Analog = Analog::P47;
    pub(crate) const P6000: Analog = Analog::P48;
    pub(crate) const P6125: Analog = Analog::P49;
    pub(crate) const P6250: Analog = Analog::P50;
    pub(crate) const P6375: Analog = Analog::P51;
    pub(crate) const P6500: Analog = Analog::P52;
    pub(crate) const P6625: Analog = Analog::P53;
    pub(crate) const P6750: Analog = Analog::P54;
    pub(crate) const P6875: Analog = Analog::P55;
    pub(crate) const P7000: Analog = Analog::P56;
    pub(crate) const P7125: Analog = Analog::P57;
    pub(crate) const P7250: Analog = Analog::P58;
    pub(crate) const P7375: Analog = Analog::P59;
    pub(crate) const P7500: Analog = Analog::P60;
    pub(crate) const P7625: Analog = Analog::P61;
    pub(crate) const P7750: Analog = Analog::P62;
    pub(crate) const P7875: Analog = Analog::P63;
    pub(crate) const P8000: Analog = Analog::P64;
    pub(crate) const P8125: Analog = Analog::P65;
    pub(crate) const P8250: Analog = Analog::P66;
    pub(crate) const P8375: Analog = Analog::P67;
    pub(crate) const P8500: Analog = Analog::P68;
    pub(crate) const P8625: Analog = Analog::P69;
    pub(crate) const P8750: Analog = Analog::P70;
    pub(crate) const P8875: Analog = Analog::P71;
    pub(crate) const P9000: Analog = Analog::P72;
    pub(crate) const P9125: Analog = Analog::P73;
    pub(crate) const P9250: Analog = Analog::P74;
    pub(crate) const P9375: Analog = Analog::P75;
    pub(crate) const P9500: Analog = Analog::P76;
    pub(crate) const P9625: Analog = Analog::P77;
    pub(crate) const P9750: Analog = Analog::P78;
    pub(crate) const P9875: Analog = Analog::P79;
}
use consts::*;

bounded_integer::bounded_integer! {
    enum Trigger { 0..=140 }
}
const LS: Trigger = Trigger::P49;
const MS: Trigger = Trigger::P94;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum Stick {
    A,
    C,
}

type GCStickInput = (Analog, Analog);
type AStickInput = GCStickInput;
type CStickInput = GCStickInput;

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum GCInput {
    Button(Button, Pressed),
    Stick(Stick, GCStickInput),
    Trigger(Trigger),
    ModifiedPress(AStickInput, ButtonImpure),
    ReleaseModifier(ButtonImpure, AStickInput),
    CStickModifier { a: AStickInput, c: CStickInput },
}

bitflags::bitflags! {
    #[derive(Default)]
    struct B0xxState: u16 {
        const NONE = 0;

        const B = 0x001;
        const L = 0x002;
        const R = 0x004;
        const MOD_X = 0x008;
        const MOD_Y = 0x010;
        const D_UP = 0x020;
        const D_DOWN = 0x040;
        const D_LEFT = 0x080;
        const D_RIGHT = 0x100;

        const MODS = Self::MOD_X.bits | Self::MOD_Y.bits;
        const LR = Self::L.bits | Self::R.bits;
    }
}

impl B0xxState {
    fn dpad_convert(axis: Axis, dir: Direction) -> Self {
        match (axis, dir) {
            (Axis::X, POSITIVE) => Self::D_RIGHT,
            (Axis::X, NEGATIVE) => Self::D_LEFT,
            (Axis::Y, POSITIVE) => Self::D_UP,
            (Axis::Y, NEGATIVE) => Self::D_DOWN,
        }
    }

    fn dpad_insert(&mut self, axis: Axis, dir: Direction) {
        self.insert(Self::dpad_convert(axis, dir))
    }

    fn dpad_clear_if(&mut self, axis: Axis, dir: Direction) -> bool {
        let bit = Self::dpad_convert(axis, dir);
        let rtn = self.contains(bit);
        if rtn {
            self.remove(bit);
        }
        rtn
    }
}

type Direction = bool;
const POSITIVE: Direction = true;
const NEGATIVE: Direction = false;

type Pressed = bool;
const PRESSED: Pressed = true;
const RELEASED: Pressed = false;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum AxisState {
    Null(Option<Direction>),
    // Direction is active and whether the opposing direction is pressed.
    Active(Direction, Pressed),
}

impl std::default::Default for AxisState {
    fn default() -> Self {
        Self::Null(None)
    }
}

impl AxisState {
    fn transition(&mut self, dir: Direction, pressed: Pressed) {
        *self = match *self {
            Self::Null(None) if pressed => Self::Active(dir, RELEASED),
            Self::Null(Some(inactive)) if !pressed && inactive == dir => Self::Null(None),
            Self::Null(Some(inactive)) if pressed && inactive != dir => Self::Active(dir, PRESSED),
            Self::Active(active, RELEASED) if !pressed && dir == active => Self::Null(None),
            Self::Active(active, RELEASED) if pressed && dir != active => {
                Self::Active(dir, PRESSED)
            }
            Self::Active(active, PRESSED) if !pressed => {
                if dir == active {
                    Self::Null(Some(!active))
                } else {
                    Self::Active(active, RELEASED)
                }
            }
            _ => *self,
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum ShieldState {
    Null,
    M(bool),
    L,
    ML,
    LM,
}

impl std::default::Default for ShieldState {
    fn default() -> Self {
        Self::Null
    }
}

impl ShieldState {
    fn transition(&mut self, shield: Shield, pressed: Pressed) -> Option<Trigger> {
        let (new, rtn) = match (*self, shield, pressed) {
            (Self::Null, Shield::Light, PRESSED) => (Self::L, Some(LS)),
            (Self::Null, Shield::Medium, PRESSED) => (Self::M(PRESSED), Some(MS)),
            (Self::M(_), Shield::Light, PRESSED) => (Self::ML, Some(LS)),
            (Self::L, Shield::Medium, PRESSED) => (Self::LM, Some(MS)),
            (Self::ML, Shield::Light, RELEASED) => (Self::M(RELEASED), Some(Trigger::Z)),
            (Self::LM, Shield::Light, RELEASED) => (Self::M(PRESSED), None),
            (Self::ML, Shield::Medium, RELEASED) => (Self::L, None),
            (Self::LM, Shield::Medium, RELEASED) => (Self::L, Some(LS)),
            (Self::M(PRESSED), Shield::Medium, RELEASED) | (Self::L, Shield::Light, RELEASED) => {
                (Self::Null, Some(Trigger::Z))
            }
            (Self::M(RELEASED), Shield::Medium, RELEASED) => (Self::Null, None),
            _ => (*self, None),
        };
        *self = new;
        rtn
    }
}

trait NegExt: std::ops::Neg {
    fn neg_not(self, b: bool) -> Self;
}

impl<N: std::ops::Neg<Output = N>> NegExt for N {
    fn neg_not(self, b: bool) -> N {
        if b {
            self
        } else {
            -self
        }
    }
}

#[derive(Default)]
struct StickState {
    x: AxisState,
    y: AxisState,
    gc_input: GCStickInput,
}

impl StickState {
    fn update(&mut self, input: GCStickInput) -> Option<GCStickInput> {
        (self.gc_input != input).then(|| {
            self.gc_input = input;
            input
        })
    }
}

#[derive(Default)]
struct Main {
    state: B0xxState,
    a_stick: StickState,
    c_stick: StickState,
    shield_state: ShieldState,
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum Shield {
    Light,
    Medium,
}

impl std::convert::From<Shield> for Trigger {
    fn from(s: Shield) -> Self {
        match s {
            Shield::Light => LS,
            Shield::Medium => MS,
        }
    }
}

impl Main {
    fn update_c_stick(&mut self) -> Option<GCStickInput> {
        let input = match (self.c_stick.x, self.c_stick.y) {
            (AxisState::Null(_), AxisState::Null(_)) => (P0000, P0000),
            (AxisState::Active(x_dir, _), AxisState::Null(_)) => {
                if self.state & B0xxState::MODS == B0xxState::MOD_X {
                    match (self.a_stick.x, self.a_stick.y) {
                        (AxisState::Null(_), AxisState::Active(y_dir, _)) => {
                            (P8125.neg_not(x_dir), P2875.neg_not(y_dir))
                        }
                        _ => (Analog::MAX.neg_not(x_dir), P0000),
                    }
                } else {
                    (Analog::MAX.neg_not(x_dir), P0000)
                }
            }
            (AxisState::Null(_), AxisState::Active(y_dir, _)) => (P0000, Analog::MAX.neg_not(y_dir)),
            (AxisState::Active(x_dir, _), AxisState::Active(y_dir, _)) => {
                (P5250.neg_not(x_dir), P8500.neg_not(y_dir))
            }
        };
        self.c_stick.update(input)
    }

    fn update_a_stick(&mut self, crouch_walk_option_select: bool) -> Option<GCStickInput> {
        let input = match (self.a_stick.x, self.a_stick.y) {
            (AxisState::Null(_), AxisState::Null(_)) => (P0000, P0000),
            (AxisState::Active(x_dir, opposing_held), AxisState::Null(_)) => {
                let x = match (
                    self.state & B0xxState::MODS,
                    self.state.contains(B0xxState::B),
                    opposing_held,
                ) {
                    (B0xxState::MOD_X, _, false) | (B0xxState::MOD_Y, true, false) => P6625,
                    (B0xxState::MOD_Y, false, false) => P3375,
                    _ => Analog::MAX,
                };
                (x.neg_not(x_dir), P0000)
            }
            (AxisState::Null(_), AxisState::Active(y_dir, _)) => {
                let y = if self.state & B0xxState::MODS == B0xxState::MOD_X {
                    P5375
                } else if self.state & B0xxState::MODS == B0xxState::MOD_Y {
                    P7375
                } else {
                    Analog::MAX
                };
                (P0000, y.neg_not(y_dir))
            }
            // Diagonals.
            (AxisState::Active(x_dir, _), AxisState::Active(y_dir, _)) => {
                let (x, y) = match (
                    self.state & B0xxState::MODS,
                    self.state.intersects(B0xxState::LR),
                    self.c_stick.x,
                    self.c_stick.y,
                ) {
                    (B0xxState::MOD_X, true, _, _) => (P6375, P3750),
                    (
                        B0xxState::MOD_X,
                        false,
                        AxisState::Null(None),
                        AxisState::Active(NEGATIVE, RELEASED),
                    ) => (P7000, P3625),
                    (
                        B0xxState::MOD_X,
                        false,
                        AxisState::Active(NEGATIVE, RELEASED),
                        AxisState::Null(None),
                    ) => (P7875, P4875),
                    (
                        B0xxState::MOD_X,
                        false,
                        AxisState::Null(None),
                        AxisState::Active(POSITIVE, RELEASED),
                    ) => (P7000, P5125),
                    (
                        B0xxState::MOD_X,
                        false,
                        AxisState::Active(POSITIVE, RELEASED),
                        AxisState::Null(None),
                    ) => (P6125, P5250),
                    (B0xxState::MOD_X, _, _, _) => (P7375, P3125),

                    (B0xxState::MOD_Y, true, _, _) => {
                        if y_dir {
                            (P4750, P8750)
                        } else {
                            (P5000, P8500)
                        }
                    }
                    (
                        B0xxState::MOD_Y,
                        false,
                        AxisState::Active(POSITIVE, RELEASED),
                        AxisState::Null(None),
                    ) => (P6375, P7625),
                    (
                        B0xxState::MOD_Y,
                        false,
                        AxisState::Null(None),
                        AxisState::Active(POSITIVE, RELEASED),
                    ) => (P5125, P7000),
                    (
                        B0xxState::MOD_Y,
                        false,
                        AxisState::Active(NEGATIVE, RELEASED),
                        AxisState::Null(None),
                    ) => (P4875, P7875),
                    (
                        B0xxState::MOD_Y,
                        false,
                        AxisState::Null(None),
                        AxisState::Active(NEGATIVE, RELEASED),
                    ) => (P3625, P7000),
                    (B0xxState::MOD_Y, _, _, _) => (P3125, P7375),
                    _ => {
                        if !y_dir && crouch_walk_option_select {
                            (P7125, P6875)
                        } else {
                            (P7000, P7000)
                        }
                    }
                };
                (x.neg_not(x_dir), y.neg_not(y_dir))
            }
        };
        self.a_stick.update(input)
    }

    fn b0xx_to_gc(
        &mut self,
        B0xxEvent {
            time: _,
            btn,
            pressed,
        }: B0xxEvent,
        crouch_walk_option_select: bool,
    ) -> Option<GCInput> {
        let impure = match btn {
            B0xx::Pure(pure) => {
                return match pure {
                    Pure::Button(btn_pure) => {
                        Some(GCInput::Button(Button::Pure(btn_pure), pressed))
                    }
                    Pure::Shield(shield) => self
                        .shield_state
                        .transition(shield, pressed)
                        .map(GCInput::Trigger),
                };
            }
            B0xx::Impure(impure) => impure,
        };
        match impure {
            Impure::Button(btn) => {
                match btn {
                    ButtonImpure::B => {
                        self.state.set(B0xxState::B, pressed);
                    }
                    ButtonImpure::L => {
                        self.state.set(B0xxState::L, pressed);
                    }
                    ButtonImpure::R => {
                        self.state.set(B0xxState::R, pressed);
                    }
                }
                return Some(if let Some(new) = self.update_a_stick(crouch_walk_option_select) {
                    self.a_stick.gc_input = new;
                    if pressed {
                        GCInput::ModifiedPress(new, btn)
                    } else {
                        GCInput::ReleaseModifier(btn, new)
                    }
                } else {
                    GCInput::Button(Button::Impure(btn), pressed)
                });
            }
            Impure::Stick(Stick::C, axis, dir) => {
                if pressed && self.state.contains(B0xxState::MODS) {
                    self.state.dpad_insert(axis, dir);
                    return Some(GCInput::Button(Button::DPad(axis, dir), PRESSED));
                } else if !pressed && self.state.dpad_clear_if(axis, dir) {
                    return Some(GCInput::Button(Button::DPad(axis, dir), RELEASED));
                } else {
                    match axis {
                        Axis::X => self.c_stick.x.transition(dir, pressed),
                        Axis::Y => self.c_stick.y.transition(dir, pressed),
                    }
                }
            }
            Impure::Stick(Stick::A, Axis::X, dir) => self.a_stick.x.transition(dir, pressed),
            Impure::Stick(Stick::A, Axis::Y, dir) => self.a_stick.y.transition(dir, pressed),
            Impure::ModX => self.state.set(B0xxState::MOD_X, pressed),
            Impure::ModY => self.state.set(B0xxState::MOD_Y, pressed),
        }

        match (self.update_a_stick(crouch_walk_option_select), self.update_c_stick()) {
            (None, None) => None,
            (Some(new_a), None) => Some(GCInput::Stick(Stick::A, new_a)),
            (None, Some(new_c)) => Some(GCInput::Stick(Stick::C, new_c)),
            (Some(new_a), Some(new_c)) => Some(GCInput::CStickModifier { a: new_a, c: new_c }),
        }
    }
}

struct OutputSink {
    file: std::fs::File,
}

impl OutputSink {
    fn send(&mut self, o: GCInput) -> anyhow::Result<()> {
        fn convert(a: Analog) -> f64 {
            let a = a.get() as f64;
            0.5 + 0.5 * if a < 0.0 { a / 128. } else { a / 127. }
        }

        fn btn_name(btn: Button) -> &'static str {
            match btn {
                Button::Pure(btn) => btn_pure_name(btn),
                Button::Impure(btn) => btn_impure_name(btn),
                Button::DPad(axis, dir) => match (axis, dir) {
                    (Axis::X, POSITIVE) => "D_RIGHT",
                    (Axis::X, NEGATIVE) => "D_LEFT",
                    (Axis::Y, POSITIVE) => "D_UP",
                    (Axis::Y, NEGATIVE) => "D_DOWN",
                },
            }
        }

        fn btn_impure_name(btn: ButtonImpure) -> &'static str {
            use ButtonImpure::*;
            match btn {
                B => "B",
                L => "L",
                R => "R",
            }
        }

        fn btn_pure_name(btn: ButtonPure) -> &'static str {
            use ButtonPure::*;
            match btn {
                A => "A",
                X => "X",
                Y => "Y",
                Z => "Z",
                Start => "START",
            }
        }

        fn stick_name(stick: Stick) -> &'static str {
            match stick {
                Stick::A => "MAIN",
                Stick::C => "C",
            }
        }

        let cmd = match o {
            GCInput::Button(btn, pressed) => format!(
                "{} {}\n",
                if pressed { "PRESS" } else { "RELEASE" },
                btn_name(btn)
            ),
            GCInput::Stick(stick, (x, y)) => {
                format!("SET {} {} {}\n", stick_name(stick), convert(x), convert(y))
            }
            GCInput::Trigger(v) => format!("SET L {}\n", (v.get() as f64) / 128.),
            GCInput::ModifiedPress((x, y), btn) => format!(
                "SET MAIN {} {}\nPRESS {}\n",
                convert(x),
                convert(y),
                btn_impure_name(btn),
            ),
            GCInput::ReleaseModifier(btn, (x, y)) => format!(
                "RELEASE {}\nSET MAIN {} {}\n",
                btn_impure_name(btn),
                convert(x),
                convert(y),
            ),
            GCInput::CStickModifier {
                a: (ax, ay),
                c: (cx, cy),
            } => format!(
                "SET MAIN {} {}\nSET C {} {}\n",
                convert(ax),
                convert(ay),
                convert(cx),
                convert(cy),
            ),
        };
        debug!("writing: {}", cmd);
        let _ = self.file.write(cmd.as_bytes())?;
        Ok(())
    }
}

fn main() {
    let Args { log_level, crouch_walk_option_select } = argh::from_env();

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
                    if let Some(o) = main.b0xx_to_gc(e, crouch_walk_option_select) {
                        sink.send(o).expect("failed to write to pipe");
                    }
                }
            }
        }
    };
    futures::executor::block_on(fut);
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(&[
        (B0xx::Pure(Pure::Shield(Shield::Light)), PRESSED, Some(GCInput::Trigger(LS))),
        (B0xx::Pure(Pure::Shield(Shield::Medium)), PRESSED, Some(GCInput::Trigger(MS))),
        (B0xx::Pure(Pure::Shield(Shield::Medium)), RELEASED, Some(GCInput::Trigger(LS))),
        (B0xx::Pure(Pure::Shield(Shield::Light)), RELEASED, Some(GCInput::Trigger(Trigger::Z))),
    ]; "shield1")]
    #[test_case(&[
        (B0xx::Pure(Pure::Shield(Shield::Light)), PRESSED, Some(GCInput::Trigger(LS))),
        (B0xx::Pure(Pure::Shield(Shield::Medium)), PRESSED, Some(GCInput::Trigger(MS))),
        (B0xx::Pure(Pure::Shield(Shield::Light)), RELEASED, None),
        (B0xx::Pure(Pure::Shield(Shield::Light)), PRESSED, Some(GCInput::Trigger(LS))),
        (B0xx::Pure(Pure::Shield(Shield::Light)), RELEASED, Some(GCInput::Trigger(Trigger::Z))),
        (B0xx::Pure(Pure::Shield(Shield::Medium)), RELEASED, None),
    ]; "shield2")]
    #[test_case(&[
        (B0xx::Pure(Pure::Shield(Shield::Medium)), PRESSED, Some(GCInput::Trigger(MS))),
        (B0xx::Pure(Pure::Shield(Shield::Light)), PRESSED, Some(GCInput::Trigger(LS))),
        (B0xx::Pure(Pure::Shield(Shield::Medium)), RELEASED, None),
        (B0xx::Pure(Pure::Shield(Shield::Light)), RELEASED, Some(GCInput::Trigger(Trigger::Z))),
    ]; "shield3")]
    fn steps(steps: &[(B0xx, Pressed, Option<GCInput>)]) {
        let mut main = Main::default();
        for &(btn, pressed, want) in steps.into_iter() {
            assert_eq!(
                main.b0xx_to_gc(B0xxEvent::new_without_time(btn, pressed), false),
                want
            );
        }
    }

    #[test_case(&[], P7000, P7000; "a_stick")]
    #[test_case(&[B0xx::Impure(Impure::ModX), B0xx::Impure(Impure::ModY)], P7000, P7000; "a_stick_both_mod")]
    #[test_case(&[B0xx::Impure(Impure::ModX)], P7375, P3125; "mod_x")]
    #[test_case(&[B0xx::Impure(Impure::ModX), B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, NEGATIVE))], P7000, P3625; "mod_x1")]
    #[test_case(&[B0xx::Impure(Impure::ModX), B0xx::Impure(Impure::Stick(Stick::C, Axis::X, NEGATIVE))], P7875, P4875; "mod_x2")]
    #[test_case(&[B0xx::Impure(Impure::ModX), B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, POSITIVE))], P7000, P5125; "mod_x3")]
    #[test_case(&[B0xx::Impure(Impure::ModX), B0xx::Impure(Impure::Stick(Stick::C, Axis::X, POSITIVE))], P6125, P5250; "mod_x4")]
    #[test_case(&[B0xx::Impure(Impure::ModY), B0xx::Impure(Impure::Stick(Stick::C, Axis::X, POSITIVE))], P6375, P7625; "mod_y4")]
    #[test_case(&[B0xx::Impure(Impure::ModY), B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, POSITIVE))], P5125, P7000; "mod_y3")]
    #[test_case(&[B0xx::Impure(Impure::ModY), B0xx::Impure(Impure::Stick(Stick::C, Axis::X, NEGATIVE))], P4875, P7875; "mod_y2")]
    #[test_case(&[B0xx::Impure(Impure::ModY), B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, NEGATIVE))], P3625, P7000; "mod_y1")]
    #[test_case(&[B0xx::Impure(Impure::ModY)], P3125, P7375; "mod_y")]
    #[test_case(&[B0xx::Impure(Impure::ModX), B0xx::Impure(Impure::Button(ButtonImpure::L))], P6375, P3750; "mod_x_l")]
    #[test_case(&[B0xx::Impure(Impure::ModX), B0xx::Impure(Impure::Button(ButtonImpure::R))], P6375, P3750; "mod_x_r")]
    fn analog(buttons: &[B0xx], x_positive: Analog, y_positive: Analog) {
        for x in [POSITIVE, NEGATIVE] {
            for y in [POSITIVE, NEGATIVE] {
                let mut buttons = buttons
                    .iter()
                    .copied()
                    .chain(
                        [
                            B0xx::Impure(Impure::Stick(Stick::A, Axis::X, x)),
                            B0xx::Impure(Impure::Stick(Stick::A, Axis::Y, y)),
                        ]
                        .into_iter(),
                    )
                    .collect::<Vec<_>>();
                let want = (x_positive.neg_not(x), y_positive.neg_not(y));
                permutohedron::heap_recursive(&mut buttons, |buttons| {
                    let mut main = Main::default();
                    let got = buttons
                        .iter()
                        .fold(None, |_, &btn| {
                            main.b0xx_to_gc(B0xxEvent::new_without_time(btn, PRESSED), false)
                        })
                        .expect("final b0xx input resulted in null GC input");
                    let got = match got {
                        GCInput::ModifiedPress(a_stick, btn) => {
                            assert_eq!(B0xx::Impure(Impure::Button(btn)), *buttons.last().unwrap());
                            a_stick
                        }
                        GCInput::Stick(Stick::A, a_stick) => a_stick,
                        GCInput::CStickModifier { a, c: _ } => a,
                        _ => panic!("unexpected GC input on final b0xx input: {:?}", got),
                    };
                    assert_eq!(got, want);
                });
            }
        }
    }

    #[test_case(false, &[B0xx::Impure(Impure::ModY), B0xx::Impure(Impure::Button(ButtonImpure::L))], P4750, P8750, P5000, P8500; "mod_y_l")]
    #[test_case(false, &[B0xx::Impure(Impure::ModY), B0xx::Impure(Impure::Button(ButtonImpure::R))], P4750, P8750, P5000, P8500; "mod_y_r")]
    #[test_case(true, &[], P7000, P7000, P7125, P6875; "crouch_walk_option_select")]
    fn analog_top_bottom(crouch_walk_option_select: bool, buttons: &[B0xx], x_top: Analog, y_top: Analog, x_bottom: Analog, y_bottom: Analog) {
        for x in [POSITIVE, NEGATIVE] {
            for y in [POSITIVE, NEGATIVE] {
                let mut buttons = buttons
                    .iter()
                    .copied()
                    .chain(
                        [
                            B0xx::Impure(Impure::Stick(Stick::A, Axis::X, x)),
                            B0xx::Impure(Impure::Stick(Stick::A, Axis::Y, y)),
                        ]
                        .into_iter(),
                    )
                    .collect::<Vec<_>>();
                let want = if y {
                    (x_top.neg_not(x), y_top.neg_not(y))
                } else {
                    (x_bottom.neg_not(x), y_bottom.neg_not(y))
                };
                permutohedron::heap_recursive(&mut buttons, |buttons| {
                    let mut main = Main::default();
                    let got = buttons
                        .iter()
                        .fold(None, |_, &btn| {
                            main.b0xx_to_gc(B0xxEvent::new_without_time(btn, PRESSED), crouch_walk_option_select)
                        })
                        .expect("final b0xx input resulted in null GC input");
                    let got = match got {
                        GCInput::ModifiedPress(a_stick, btn) => {
                            assert_eq!(B0xx::Impure(Impure::Button(btn)), *buttons.last().unwrap());
                            a_stick
                        }
                        GCInput::Stick(Stick::A, a_stick) => a_stick,
                        GCInput::CStickModifier { a, c: _ } => a,
                        _ => panic!("unexpected GC input on final b0xx input: {:?}", got),
                    };
                    assert_eq!(got, want);
                });
            }
        }
    }

    #[test]
    fn c_stick_diagonals() {
        for x in [POSITIVE, NEGATIVE] {
            for y in [POSITIVE, NEGATIVE] {
                let mut buttons = [
                    B0xx::Impure(Impure::Stick(Stick::C, Axis::X, x)),
                    B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, y)),
                ];
                let c_stick = (P5250.neg_not(x), P8500.neg_not(y));
                permutohedron::heap_recursive(&mut buttons, |buttons| {
                    let mut main = Main::default();
                    let got = buttons
                        .iter()
                        .fold(None, |_, &btn| {
                            main.b0xx_to_gc(B0xxEvent::new_without_time(btn, PRESSED), false)
                        })
                        .expect("final b0xx input resulted in null GC input");
                    assert_eq!(got, GCInput::Stick(Stick::C, c_stick));
                });
            }
        }
    }

    #[test_case(&[], Stick::A, Analog::MAX, Analog::MAX; "a_stick")]
    #[test_case(&[B0xx::Impure(Impure::ModX)], Stick::A, P6625, P5375; "a_stick_mod_x")]
    #[test_case(&[B0xx::Impure(Impure::ModY)], Stick::A, P3375, P7375; "a_stick_mod_y")]
    #[test_case(&[], Stick::C, Analog::MAX, Analog::MAX; "c_stick")]
    fn cardinals(buttons: &[B0xx], stick: Stick, x_positive: Analog, y_positive: Analog) {
        for axis in [Axis::X, Axis::Y] {
            for dir in [POSITIVE, NEGATIVE] {
                let mut buttons = buttons
                    .iter()
                    .copied()
                    .chain(std::iter::once(B0xx::Impure(Impure::Stick(
                        stick, axis, dir,
                    ))))
                    .collect::<Vec<_>>();
                let want = match axis {
                    Axis::X => (x_positive.neg_not(dir), P0000),
                    Axis::Y => (P0000, y_positive.neg_not(dir)),
                };
                permutohedron::heap_recursive(&mut buttons, |buttons| {
                    let mut main = Main::default();
                    let got = buttons
                        .iter()
                        .fold(None, |_, &btn| {
                            main.b0xx_to_gc(B0xxEvent::new_without_time(btn, PRESSED), false)
                        })
                        .expect("final b0xx input resulted in null GC input");
                    assert_eq!(got, GCInput::Stick(stick, want));
                });
            }
        }
    }

    #[test]
    fn dpad() {
        for axis in [Axis::X, Axis::Y] {
            for dir in [POSITIVE, NEGATIVE] {
                let mut main = Main::default();
                let got = main.b0xx_to_gc(B0xxEvent::new_without_time(
                    B0xx::Impure(Impure::ModX),
                    PRESSED,
                ), false);
                assert_eq!(got, None);
                let got = main.b0xx_to_gc(B0xxEvent::new_without_time(
                    B0xx::Impure(Impure::ModY),
                    PRESSED,
                ), false);
                assert_eq!(got, None);
                let got = main.b0xx_to_gc(B0xxEvent::new_without_time(
                    B0xx::Impure(Impure::Stick(Stick::C, axis, dir)),
                    PRESSED,
                ), false);
                assert_eq!(got, Some(GCInput::Button(Button::DPad(axis, dir), PRESSED)));
            }
        }
    }

    #[test]
    fn tilt_fsmash() {
        for x_dir in [POSITIVE, NEGATIVE] {
            for y_dir in [POSITIVE, NEGATIVE] {
                let mut main = Main::default();
                let got = main.b0xx_to_gc(B0xxEvent::new_without_time(
                    B0xx::Impure(Impure::ModX),
                    PRESSED,
                ), false);
                assert_eq!(got, None);
                let got = main.b0xx_to_gc(B0xxEvent::new_without_time(
                    B0xx::Impure(Impure::Stick(Stick::A, Axis::Y, y_dir)),
                    PRESSED,
                ), false);
                assert_eq!(
                    got,
                    Some(GCInput::Stick(Stick::A, (P0000, P5375.neg_not(y_dir))))
                );
                let got = main.b0xx_to_gc(B0xxEvent::new_without_time(
                    B0xx::Impure(Impure::Stick(Stick::C, Axis::X, x_dir)),
                    PRESSED,
                ), false);
                assert_eq!(
                    got,
                    Some(GCInput::Stick(
                        Stick::C,
                        (P8125.neg_not(x_dir), P2875.neg_not(y_dir))
                    ))
                );
                let got = main.b0xx_to_gc(B0xxEvent::new_without_time(
                    B0xx::Impure(Impure::Stick(Stick::C, Axis::X, x_dir)),
                    RELEASED,
                ), false);
                assert_eq!(got, Some(GCInput::Stick(Stick::C, (P0000, P0000))));
            }
        }
    }

    #[test]
    fn accidental_side_b() {
        for dir in [POSITIVE, NEGATIVE] {
            let mut buttons = [
                B0xx::Impure(Impure::ModY),
                B0xx::Impure(Impure::Button(ButtonImpure::B)),
            ]
            .into_iter()
            .chain(std::iter::once(B0xx::Impure(Impure::Stick(
                Stick::A,
                Axis::X,
                dir,
            ))))
            .collect::<Vec<_>>();
            permutohedron::heap_recursive(&mut buttons, |buttons| {
                let mut main = Main::default();
                let got = buttons
                    .iter()
                    .fold(None, |_, &btn| {
                        main.b0xx_to_gc(B0xxEvent::new_without_time(btn, PRESSED), false)
                    })
                    .expect("final b0xx input resulted in null GC input");
                let want = match *buttons.last().unwrap() {
                    B0xx::Impure(Impure::Button(ButtonImpure::B)) => {
                        GCInput::ModifiedPress((P6625.neg_not(dir), P0000), ButtonImpure::B)
                    }
                    _ => GCInput::Stick(Stick::A, (P6625.neg_not(dir), P0000)),
                };
                assert_eq!(got, want);
            });
        }
    }

    #[test]
    fn ledgedash_optimization() {
        for modifier in [B0xx::Impure(Impure::ModX), B0xx::Impure(Impure::ModY)] {
            let mut buttons = [
                B0xx::Impure(Impure::Stick(Stick::A, Axis::X, POSITIVE)),
                B0xx::Impure(Impure::Stick(Stick::A, Axis::X, NEGATIVE)),
                modifier,
            ];
            permutohedron::heap_recursive(&mut buttons, |buttons| {
                let mut main = Main::default();
                let got = buttons.iter().fold(None, |_, &btn| {
                    main.b0xx_to_gc(B0xxEvent::new_without_time(btn, PRESSED), false)
                });
                let want = match *buttons.last().unwrap() {
                    B0xx::Impure(Impure::ModX) | B0xx::Impure(Impure::ModY) => None,
                    B0xx::Impure(Impure::Stick(Stick::A, Axis::X, dir)) => {
                        Some(GCInput::Stick(Stick::A, (Analog::MAX.neg_not(dir), P0000)))
                    }
                    btn => panic!("unexpected button: {:?}", btn),
                };
                assert_eq!(got, want);
            })
        }
    }
}
