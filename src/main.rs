#![deny(unused_results)]

use std::io::Write as _;

use argh::FromArgs;
use either::Either;
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
    fn keyboard_to_b0xx(&self, c: evdev_rs::enums::EventCode) -> Option<B0xxRaw> {
        use evdev_rs::enums::{EventCode, EV_KEY};
        match c {
            EventCode::EV_KEY(EV_KEY::KEY_SEMICOLON) => Some(B0xxRaw::L),
            EventCode::EV_KEY(EV_KEY::KEY_O) => Some(B0xxRaw::Left),
            EventCode::EV_KEY(EV_KEY::KEY_E) => Some(B0xxRaw::Down),
            EventCode::EV_KEY(EV_KEY::KEY_U) => Some(B0xxRaw::Right),
            EventCode::EV_KEY(EV_KEY::KEY_LEFTSHIFT) => Some(B0xxRaw::MX),
            EventCode::EV_KEY(EV_KEY::KEY_LEFTCTRL) => Some(B0xxRaw::MY),
            EventCode::EV_KEY(EV_KEY::KEY_Y) | EventCode::EV_KEY(EV_KEY::KEY_F) => {
                Some(B0xxRaw::Start)
            }
            EventCode::EV_KEY(EV_KEY::KEY_G) => Some(B0xxRaw::R),
            EventCode::EV_KEY(EV_KEY::KEY_C) => Some(B0xxRaw::Y),
            EventCode::EV_KEY(EV_KEY::KEY_R) => Some(B0xxRaw::LS),
            EventCode::EV_KEY(EV_KEY::KEY_S) => Some(B0xxRaw::MS),
            EventCode::EV_KEY(EV_KEY::KEY_H) => Some(B0xxRaw::B),
            EventCode::EV_KEY(EV_KEY::KEY_T) => Some(B0xxRaw::X),
            EventCode::EV_KEY(EV_KEY::KEY_N) => Some(B0xxRaw::Z),
            EventCode::EV_KEY(EV_KEY::KEY_Z) => Some(B0xxRaw::Up),
            EventCode::EV_KEY(EV_KEY::KEY_ESC) => Some(B0xxRaw::CD),
            EventCode::EV_KEY(EV_KEY::KEY_BACKSPACE) => Some(B0xxRaw::CL),
            EventCode::EV_KEY(EV_KEY::KEY_DOWN) => Some(B0xxRaw::CU),
            EventCode::EV_KEY(EV_KEY::KEY_ENTER) => Some(B0xxRaw::CR),
            EventCode::EV_KEY(EV_KEY::KEY_SPACE) => Some(B0xxRaw::A),
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
enum B0xxRaw {
    A,
    B,
    L,
    R,
    X,
    Y,
    Z,
    Start,
    Left,
    Right,
    Down,
    Up,
    MX,
    MY,
    LS,
    MS,
    CU,
    CD,
    CL,
    CR,
}

impl From<B0xxRaw> for B0xx {
    fn from(t: B0xxRaw) -> B0xx {
        match t {
            B0xxRaw::A => B0xx::Pure(Pure::Button(ButtonPure::A)),
            B0xxRaw::X => B0xx::Pure(Pure::Button(ButtonPure::X)),
            B0xxRaw::Y => B0xx::Pure(Pure::Button(ButtonPure::Y)),
            B0xxRaw::Z => B0xx::Pure(Pure::Button(ButtonPure::Z)),
            B0xxRaw::Start => B0xx::Pure(Pure::Button(ButtonPure::Start)),
            B0xxRaw::B => B0xx::Impure(Impure::Button(ButtonImpure::B)),
            B0xxRaw::L => B0xx::Impure(Impure::Button(ButtonImpure::L)),
            B0xxRaw::R => B0xx::Impure(Impure::Button(ButtonImpure::R)),
            B0xxRaw::Left => B0xx::Impure(Impure::Stick(Stick::A, Axis::X, NEGATIVE)),
            B0xxRaw::Right => B0xx::Impure(Impure::Stick(Stick::A, Axis::X, POSITIVE)),
            B0xxRaw::Down => B0xx::Impure(Impure::Stick(Stick::A, Axis::Y, NEGATIVE)),
            B0xxRaw::Up => B0xx::Impure(Impure::Stick(Stick::A, Axis::Y, POSITIVE)),
            B0xxRaw::MX => B0xx::Impure(Impure::ModX),
            B0xxRaw::MY => B0xx::Impure(Impure::ModY),
            B0xxRaw::LS => B0xx::Pure(Pure::Shield(Shield::Light)),
            B0xxRaw::MS => B0xx::Pure(Pure::Shield(Shield::Medium)),
            B0xxRaw::CU => B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, POSITIVE)),
            B0xxRaw::CD => B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, NEGATIVE)),
            B0xxRaw::CR => B0xx::Impure(Impure::Stick(Stick::C, Axis::X, POSITIVE)),
            B0xxRaw::CL => B0xx::Impure(Impure::Stick(Stick::C, Axis::X, NEGATIVE)),
        }
    }
}

enum GCButton {
    A,
    B,
    DUp,
    DDown,
    DLeft,
    DRight,
    L,
    R,
    X,
    Y,
    Z,
    Start,
}

impl From<Button> for GCButton {
    fn from(button: Button) -> GCButton {
        match button {
            Button::Pure(ButtonPure::A) => GCButton::A,
            Button::Pure(ButtonPure::X) => GCButton::X,
            Button::Pure(ButtonPure::Y) => GCButton::Y,
            Button::Pure(ButtonPure::Z) => GCButton::Z,
            Button::Pure(ButtonPure::Start) => GCButton::Start,
            Button::Impure(ButtonImpure::B) => GCButton::B,
            Button::Impure(ButtonImpure::L) => GCButton::L,
            Button::Impure(ButtonImpure::R) => GCButton::R,
            Button::DPad(Axis::Y, POSITIVE) => GCButton::DUp,
            Button::DPad(Axis::Y, NEGATIVE) => GCButton::DDown,
            Button::DPad(Axis::X, POSITIVE) => GCButton::DRight,
            Button::DPad(Axis::X, NEGATIVE) => GCButton::DLeft,
        }
    }
}

impl From<ButtonImpure> for GCButton {
    fn from(button: ButtonImpure) -> GCButton {
        match button {
            ButtonImpure::B => GCButton::B,
            ButtonImpure::L => GCButton::L,
            ButtonImpure::R => GCButton::R,
        }
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
    btn: B0xxRaw,
    pressed: Pressed,
}

impl B0xxEvent {
    #[cfg(test)]
    fn new_without_time(btn: B0xxRaw, pressed: Pressed) -> Self {
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

enum DolphinPipeInput {
    Button(GCButton, Pressed),
    Trigger(Trigger),
    Stick(Stick, GCStickInput),
}

impl DolphinPipeInput {
    fn into_input_string(self) -> String {
        match self {
            Self::Button(button, pressed) => format!(
                "{} {}\n",
                if pressed { "PRESS" } else { "RELEASE" },
                match button {
                    GCButton::A => "A",
                    GCButton::B => "B",
                    GCButton::DUp => "D_Up",
                    GCButton::DDown => "D_Down",
                    GCButton::DLeft => "D_Left",
                    GCButton::DRight => "D_Right",
                    GCButton::L => "L",
                    GCButton::R => "R",
                    GCButton::X => "X",
                    GCButton::Y => "Y",
                    GCButton::Z => "Z",
                    GCButton::Start => "START",
                }
            ),
            Self::Trigger(trigger) => format!("SET L {}\n", (trigger.get() as f64) / 128.),
            Self::Stick(stick, (x, y)) => {
                fn convert(a: Analog) -> f64 {
                    let a = a.get() as f64;
                    0.5 + 0.5 * if a < 0.0 { a / 128. } else { a / 127. }
                }

                format!(
                    "SET {} {} {}",
                    match stick {
                        Stick::A => "MAIN",
                        Stick::C => "C",
                    },
                    convert(x),
                    convert(y)
                )
            }
        }
    }
}

/// Intermediary representation of a possibly composite input.
#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
enum Input {
    Button(Button, Pressed),
    Stick(Stick, GCStickInput),
    Trigger(Trigger),
    ModifiedPress(AStickInput, ButtonImpure),
    ReleaseModifier(ButtonImpure, AStickInput),
    CStickModifier { a: AStickInput, c: CStickInput },
}

impl Input {
    fn into_pipe_inputs(self) -> impl IntoIterator<Item = DolphinPipeInput> {
        match self {
            Self::Button(button, pressed) => Either::Left(std::iter::once(
                DolphinPipeInput::Button(button.into(), pressed),
            )),
            Self::Trigger(trigger) => {
                Either::Left(std::iter::once(DolphinPipeInput::Trigger(trigger)))
            }
            Self::Stick(stick, stick_input) => {
                Either::Left(std::iter::once(DolphinPipeInput::Stick(stick, stick_input)))
            }
            Self::ModifiedPress(a_stick_input, button_impure) => Either::Right(
                [
                    DolphinPipeInput::Stick(Stick::A, a_stick_input),
                    DolphinPipeInput::Button(button_impure.into(), PRESSED),
                ]
                .into_iter(),
            ),
            Self::ReleaseModifier(button_impure, a_stick_input) => Either::Right(
                [
                    DolphinPipeInput::Button(button_impure.into(), RELEASED),
                    DolphinPipeInput::Stick(Stick::A, a_stick_input),
                ]
                .into_iter(),
            ),
            Self::CStickModifier { a, c } => Either::Right(
                [
                    DolphinPipeInput::Stick(Stick::C, c),
                    DolphinPipeInput::Stick(Stick::A, a),
                ]
                .into_iter(),
            ),
        }
    }
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

        const MODS = Self::MOD_X.bits | Self::MOD_Y.bits;
        const LR = Self::L.bits | Self::R.bits;
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
    // No direction is active, but the direction if present is held.
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
    fn active(self) -> Option<Direction> {
        match self {
            Self::Null(_) => None,
            Self::Active(dir, _) => Some(dir),
        }
    }

    fn active_unique(self) -> Option<Direction> {
        match self {
            Self::Null(_) => None,
            Self::Active(dir, opposite) => (!opposite).then_some(dir),
        }
    }

    fn state_in_dir(self, dir: Direction) -> AxisButtonState {
        match self {
            Self::Null(optional_pressed_dir) => {
                AxisButtonState::Inactive(optional_pressed_dir == Some(dir))
            }
            Self::Active(active_dir, opposite_pressed) => {
                if active_dir == dir {
                    AxisButtonState::Active
                } else {
                    AxisButtonState::Inactive(opposite_pressed)
                }
            }
        }
    }

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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum AxisButtonState {
    Active,
    Inactive(Pressed),
}

impl AxisButtonState {
    fn from_pressed(pressed: Pressed) -> Self {
        if pressed {
            Self::Active
        } else {
            Self::Inactive(RELEASED)
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
enum DualModeAxisState {
    Neither(AxisState),
    // The direction that is still enabled and its state.
    Single(Direction, AxisButtonState),
    Both,
}

impl std::default::Default for DualModeAxisState {
    fn default() -> Self {
        Self::Neither(Default::default())
    }
}

impl DualModeAxisState {
    // Active is defined as the active direction regardless of SOCD handling,
    // or disabled directions being held.
    fn active(self) -> Option<Direction> {
        match self {
            Self::Both => None,
            Self::Single(dir, state) => (state == AxisButtonState::Active).then_some(dir),
            Self::Neither(axis_state) => axis_state.active(),
        }
    }

    // Returns the direction that is active if the opposing direction is either
    // disabled or not held.
    fn active_unique(self) -> Option<Direction> {
        match self {
            Self::Both => None,
            Self::Single(dir, state) => (state == AxisButtonState::Active).then_some(dir),
            Self::Neither(axis_state) => axis_state.active_unique(),
        }
    }

    // TODO: This function is complicated and needs unit tests.
    /// Returns true iff alt mode was released as a result of the transition.
    ///
    /// # Panics
    ///
    /// Panics if the input is inconsistent with current state. No-ops are
    /// ignored and do not cause a panic.
    fn transition(&mut self, dir: Direction, pressed: Pressed, alt_on_pressed: bool) -> bool {
        let (new_state, alt_released) = (|s| {
            match s {
                Self::Both => {
                    if pressed && !alt_on_pressed {
                        panic!(
                            "both directions are in alt mode but direction {} is pressed",
                            dir
                        );
                    }
                    if !pressed {
                        return (
                            Self::Single(!dir, AxisButtonState::Inactive(RELEASED)),
                            true,
                        );
                    }
                }
                Self::Single(normal_dir, state) => {
                    if dir == normal_dir {
                        if pressed && alt_on_pressed {
                            return (Self::Both, false);
                        }
                        return (
                            Self::Single(dir, AxisButtonState::from_pressed(pressed)),
                            false,
                        );
                    } else {
                        if pressed {
                            if !alt_on_pressed {
                                panic!("direction {} is in alt mode but pressed normally", dir);
                            }
                        } else {
                            return (
                                match state {
                                    AxisButtonState::Active => {
                                        Self::Neither(AxisState::Active(normal_dir, RELEASED))
                                    }
                                    AxisButtonState::Inactive(inactive_pressed) => Self::Neither(
                                        AxisState::Null(inactive_pressed.then_some(normal_dir)),
                                    ),
                                },
                                true,
                            );
                        }
                    }
                }
                Self::Neither(mut axis_state) => {
                    if pressed && alt_on_pressed {
                        return (Self::Single(!dir, axis_state.state_in_dir(dir)), false);
                    }
                    axis_state.transition(dir, pressed);
                    return (Self::Neither(axis_state), false);
                }
            }
            return (s, false);
        })(*self);
        *self = new_state;
        alt_released
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

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
struct CStickState {
    x: DualModeAxisState,
    y: DualModeAxisState,
    gc_input: GCStickInput,
}

// Simplify the callsite by using a more specific form.
impl CStickState {
    // Returns the unique axis and direction that is active and no other
    // buttons are pressed.
    fn unique_cardinal(&self) -> Option<(Axis, Direction)> {
        match (self.x.active_unique(), self.y.active_unique()) {
            (Some(dir), None) => Some((Axis::X, dir)),
            (None, Some(dir)) => Some((Axis::Y, dir)),
            (None, None) | (Some(_), Some(_)) => None,
        }
    }

    fn update(&mut self, input: GCStickInput) -> Option<GCStickInput> {
        (self.gc_input != input).then(|| {
            self.gc_input = input;
            input
        })
    }

    fn transition(
        &mut self,
        axis: Axis,
        dir: Direction,
        pressed: Pressed,
        dpad_enabled: bool,
    ) -> bool {
        return match axis {
            Axis::X => self.x.transition(dir, pressed, dpad_enabled),
            Axis::Y => self.y.transition(dir, pressed, dpad_enabled),
        };
    }
}

#[derive(Default)]
struct Main {
    state: B0xxState,
    a_stick: StickState,
    c_stick: CStickState,
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
        let input = match (self.c_stick.x.active(), self.c_stick.y.active()) {
            (None, None) => (P0000, P0000),
            (Some(x_dir), None) => {
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
            (None, Some(y_dir)) => (P0000, Analog::MAX.neg_not(y_dir)),
            (Some(x_dir), Some(y_dir)) => (P5250.neg_not(x_dir), P8500.neg_not(y_dir)),
        };
        // TODO: GCStickInput should be stored separately to the CStick state.
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
                    self.c_stick.unique_cardinal(),
                ) {
                    (B0xxState::MOD_X, true, _) => (P6375, P3750),
                    (B0xxState::MOD_X, false, Some((Axis::Y, NEGATIVE))) => (P7000, P3625),
                    (B0xxState::MOD_X, false, Some((Axis::X, NEGATIVE))) => (P7875, P4875),
                    (B0xxState::MOD_X, false, Some((Axis::Y, POSITIVE))) => (P7000, P5125),
                    (B0xxState::MOD_X, false, Some((Axis::X, POSITIVE))) => (P6125, P5250),
                    (B0xxState::MOD_X, false, None) => (P7375, P3125),

                    (B0xxState::MOD_Y, true, _) => {
                        if y_dir {
                            (P4750, P8750)
                        } else {
                            (P5000, P8500)
                        }
                    }
                    (B0xxState::MOD_Y, false, Some((Axis::X, POSITIVE))) => (P6375, P7625),
                    (B0xxState::MOD_Y, false, Some((Axis::Y, POSITIVE))) => (P5125, P7000),
                    (B0xxState::MOD_Y, false, Some((Axis::X, NEGATIVE))) => (P4875, P7875),
                    (B0xxState::MOD_Y, false, Some((Axis::Y, NEGATIVE))) => (P3625, P7000),
                    (B0xxState::MOD_Y, false, None) => (P3125, P7375),
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

    fn process_b0xx(
        &mut self,
        B0xxEvent {
            time: _,
            btn,
            pressed,
        }: B0xxEvent,
        crouch_walk_option_select: bool,
    ) -> Option<Input> {
        let impure = match btn.into() {
            B0xx::Pure(pure) => {
                return match pure {
                    Pure::Button(btn_pure) => Some(Input::Button(Button::Pure(btn_pure), pressed)),
                    Pure::Shield(shield) => self
                        .shield_state
                        .transition(shield, pressed)
                        .map(Input::Trigger),
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
                return Some(
                    if let Some(new) = self.update_a_stick(crouch_walk_option_select) {
                        self.a_stick.gc_input = new;
                        if pressed {
                            Input::ModifiedPress(new, btn)
                        } else {
                            Input::ReleaseModifier(btn, new)
                        }
                    } else {
                        Input::Button(Button::Impure(btn), pressed)
                    },
                );
            }
            Impure::Stick(Stick::C, axis, dir) => {
                let dpad_enabled = self.state.contains(B0xxState::MODS);
                let dpad_released = self.c_stick.transition(axis, dir, pressed, dpad_enabled);

                if dpad_enabled && pressed {
                    return Some(Input::Button(Button::DPad(axis, dir), PRESSED));
                }
                if dpad_released {
                    return Some(Input::Button(Button::DPad(axis, dir), RELEASED));
                }
            }
            Impure::Stick(Stick::A, Axis::X, dir) => self.a_stick.x.transition(dir, pressed),
            Impure::Stick(Stick::A, Axis::Y, dir) => self.a_stick.y.transition(dir, pressed),
            Impure::ModX => self.state.set(B0xxState::MOD_X, pressed),
            Impure::ModY => self.state.set(B0xxState::MOD_Y, pressed),
        }

        match (
            self.update_a_stick(crouch_walk_option_select),
            self.update_c_stick(),
        ) {
            (None, None) => None,
            (Some(new_a), None) => Some(Input::Stick(Stick::A, new_a)),
            (None, Some(new_c)) => Some(Input::Stick(Stick::C, new_c)),
            (Some(new_a), Some(new_c)) => Some(Input::CStickModifier { a: new_a, c: new_c }),
        }
    }
}

struct OutputSink {
    file: std::fs::File,
}

impl OutputSink {
    fn send(&mut self, pipe_input: DolphinPipeInput) -> anyhow::Result<()> {
        let cmd = pipe_input.into_input_string();
        debug!("writing: {}", cmd);
        let _ = self.file.write(cmd.as_bytes())?;
        Ok(())
    }
}

fn main() {
    let Args {
        log_level,
        crouch_walk_option_select,
    } = argh::from_env();

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
                    if let Some(input) = main.process_b0xx(e, crouch_walk_option_select) {
                        for pipe_input in input.into_pipe_inputs() {
                            sink.send(pipe_input).expect("failed to write to pipe");
                        }
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
    use itertools::Itertools as _;
    use test_case::test_case;

    const CARDINALS: [(Axis, Direction); 4] = [
        (Axis::X, POSITIVE),
        (Axis::X, NEGATIVE),
        (Axis::Y, POSITIVE),
        (Axis::Y, NEGATIVE),
    ];

    const DIAGONALS: [(Direction, Direction); 4] = [
        (POSITIVE, POSITIVE),
        (POSITIVE, NEGATIVE),
        (NEGATIVE, NEGATIVE),
        (NEGATIVE, POSITIVE),
    ];

    impl From<B0xx> for B0xxRaw {
        fn from(b: B0xx) -> B0xxRaw {
            match b {
                B0xx::Pure(Pure::Button(ButtonPure::A)) => B0xxRaw::A,
                B0xx::Pure(Pure::Button(ButtonPure::X)) => B0xxRaw::X,
                B0xx::Pure(Pure::Button(ButtonPure::Y)) => B0xxRaw::Y,
                B0xx::Pure(Pure::Button(ButtonPure::Z)) => B0xxRaw::Z,
                B0xx::Pure(Pure::Button(ButtonPure::Start)) => B0xxRaw::Start,
                B0xx::Impure(Impure::Button(ButtonImpure::B)) => B0xxRaw::B,
                B0xx::Impure(Impure::Button(ButtonImpure::L)) => B0xxRaw::L,
                B0xx::Impure(Impure::Button(ButtonImpure::R)) => B0xxRaw::R,
                B0xx::Impure(Impure::Stick(Stick::A, Axis::X, NEGATIVE)) => B0xxRaw::Left,
                B0xx::Impure(Impure::Stick(Stick::A, Axis::X, POSITIVE)) => B0xxRaw::Right,
                B0xx::Impure(Impure::Stick(Stick::A, Axis::Y, NEGATIVE)) => B0xxRaw::Down,
                B0xx::Impure(Impure::Stick(Stick::A, Axis::Y, POSITIVE)) => B0xxRaw::Up,
                B0xx::Impure(Impure::ModX) => B0xxRaw::MX,
                B0xx::Impure(Impure::ModY) => B0xxRaw::MY,
                B0xx::Pure(Pure::Shield(Shield::Light)) => B0xxRaw::LS,
                B0xx::Pure(Pure::Shield(Shield::Medium)) => B0xxRaw::MS,
                B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, POSITIVE)) => B0xxRaw::CU,
                B0xx::Impure(Impure::Stick(Stick::C, Axis::Y, NEGATIVE)) => B0xxRaw::CD,
                B0xx::Impure(Impure::Stick(Stick::C, Axis::X, POSITIVE)) => B0xxRaw::CR,
                B0xx::Impure(Impure::Stick(Stick::C, Axis::X, NEGATIVE)) => B0xxRaw::CL,
            }
        }
    }

    impl From<(Stick, Axis, Direction)> for B0xxRaw {
        fn from((stick, axis, dir): (Stick, Axis, Direction)) -> B0xxRaw {
            match (stick, axis, dir) {
                (Stick::A, Axis::X, POSITIVE) => B0xxRaw::Right,
                (Stick::A, Axis::X, NEGATIVE) => B0xxRaw::Left,
                (Stick::A, Axis::Y, POSITIVE) => B0xxRaw::Up,
                (Stick::A, Axis::Y, NEGATIVE) => B0xxRaw::Down,
                (Stick::C, Axis::X, POSITIVE) => B0xxRaw::CR,
                (Stick::C, Axis::X, NEGATIVE) => B0xxRaw::CL,
                (Stick::C, Axis::Y, POSITIVE) => B0xxRaw::CU,
                (Stick::C, Axis::Y, NEGATIVE) => B0xxRaw::CD,
            }
        }
    }

    #[test_case(&[
        (B0xxRaw::LS, PRESSED, Some(Input::Trigger(LS))),
        (B0xxRaw::MS, PRESSED, Some(Input::Trigger(MS))),
        (B0xxRaw::MS, RELEASED, Some(Input::Trigger(LS))),
        (B0xxRaw::LS, RELEASED, Some(Input::Trigger(Trigger::Z))),
    ]; "shield1")]
    #[test_case(&[
        (B0xxRaw::LS, PRESSED, Some(Input::Trigger(LS))),
        (B0xxRaw::MS, PRESSED, Some(Input::Trigger(MS))),
        (B0xxRaw::LS, RELEASED, None),
        (B0xxRaw::LS, PRESSED, Some(Input::Trigger(LS))),
        (B0xxRaw::LS, RELEASED, Some(Input::Trigger(Trigger::Z))),
        (B0xxRaw::MS, RELEASED, None),
    ]; "shield2")]
    #[test_case(&[
        (B0xxRaw::MS, PRESSED, Some(Input::Trigger(MS))),
        (B0xxRaw::LS, PRESSED, Some(Input::Trigger(LS))),
        (B0xxRaw::MS, RELEASED, None),
        (B0xxRaw::LS, RELEASED, Some(Input::Trigger(Trigger::Z))),
    ]; "shield3")]
    fn steps(steps: &[(B0xxRaw, Pressed, Option<Input>)]) {
        let mut main = Main::default();
        for &(btn, pressed, want) in steps.into_iter() {
            assert_eq!(
                main.process_b0xx(B0xxEvent::new_without_time(btn, pressed), false),
                want
            );
        }
    }

    #[test_case(&[], P7000, P7000; "a_stick")]
    #[test_case(&[B0xxRaw::MX, B0xxRaw::MY], P7000, P7000; "a_stick_both_mod")]
    #[test_case(&[B0xxRaw::MX], P7375, P3125; "mod_x")]
    #[test_case(&[B0xxRaw::MX, B0xxRaw::CD], P7000, P3625; "mod_x1")]
    #[test_case(&[B0xxRaw::MX, B0xxRaw::CL], P7875, P4875; "mod_x2")]
    #[test_case(&[B0xxRaw::MX, B0xxRaw::CU], P7000, P5125; "mod_x3")]
    #[test_case(&[B0xxRaw::MX, B0xxRaw::CR], P6125, P5250; "mod_x4")]
    #[test_case(&[B0xxRaw::MY, B0xxRaw::CR], P6375, P7625; "mod_y4")]
    #[test_case(&[B0xxRaw::MY, B0xxRaw::CU], P5125, P7000; "mod_y3")]
    #[test_case(&[B0xxRaw::MY, B0xxRaw::CL], P4875, P7875; "mod_y2")]
    #[test_case(&[B0xxRaw::MY, B0xxRaw::CD], P3625, P7000; "mod_y1")]
    #[test_case(&[B0xxRaw::MY], P3125, P7375; "mod_y")]
    #[test_case(&[B0xxRaw::MX, B0xxRaw::L], P6375, P3750; "mod_x_l")]
    #[test_case(&[B0xxRaw::MX, B0xxRaw::R], P6375, P3750; "mod_x_r")]
    fn analog(buttons: &[B0xxRaw], x_positive: Analog, y_positive: Analog) {
        for x in [POSITIVE, NEGATIVE] {
            for y in [POSITIVE, NEGATIVE] {
                let mut buttons = buttons
                    .iter()
                    .copied()
                    .chain(
                        [(Stick::A, Axis::X, x).into(), (Stick::A, Axis::Y, y).into()].into_iter(),
                    )
                    .collect::<Vec<_>>();
                let want = (x_positive.neg_not(x), y_positive.neg_not(y));
                permutohedron::heap_recursive(&mut buttons, |buttons| {
                    let mut main = Main::default();
                    let got = buttons
                        .iter()
                        .fold(None, |_, &btn| {
                            main.process_b0xx(B0xxEvent::new_without_time(btn, PRESSED), false)
                        })
                        .expect("final b0xx input resulted in null GC input");
                    let got = match got {
                        Input::ModifiedPress(a_stick, btn) => {
                            assert_eq!(
                                B0xx::Impure(Impure::Button(btn)),
                                (*buttons.last().unwrap()).into()
                            );
                            a_stick
                        }
                        Input::Stick(Stick::A, a_stick) => a_stick,
                        Input::CStickModifier { a, c: _ } => a,
                        _ => panic!("unexpected GC input on final b0xx input: {:?}", got),
                    };
                    assert_eq!(got, want);
                });
            }
        }
    }

    #[test_case(false, &[B0xxRaw::MY, B0xxRaw::L], P4750, P8750, P5000, P8500; "mod_y_l")]
    #[test_case(false, &[B0xxRaw::MY, B0xxRaw::R], P4750, P8750, P5000, P8500; "mod_y_r")]
    #[test_case(true, &[], P7000, P7000, P7125, P6875; "crouch_walk_option_select")]
    fn analog_top_bottom(
        crouch_walk_option_select: bool,
        buttons: &[B0xxRaw],
        x_top: Analog,
        y_top: Analog,
        x_bottom: Analog,
        y_bottom: Analog,
    ) {
        for x in [POSITIVE, NEGATIVE] {
            for y in [POSITIVE, NEGATIVE] {
                let mut buttons = buttons
                    .iter()
                    .copied()
                    .chain(
                        [(Stick::A, Axis::X, x).into(), (Stick::A, Axis::Y, y).into()].into_iter(),
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
                            main.process_b0xx(
                                B0xxEvent::new_without_time(btn, PRESSED),
                                crouch_walk_option_select,
                            )
                        })
                        .expect("final b0xx input resulted in null GC input");
                    let got = match got {
                        Input::ModifiedPress(a_stick, btn) => {
                            assert_eq!(
                                B0xx::Impure(Impure::Button(btn)),
                                (*buttons.last().unwrap()).into()
                            );
                            a_stick
                        }
                        Input::Stick(Stick::A, a_stick) => a_stick,
                        Input::CStickModifier { a, c: _ } => a,
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
                let mut buttons = [(Stick::C, Axis::X, x).into(), (Stick::C, Axis::Y, y).into()];
                let c_stick = (P5250.neg_not(x), P8500.neg_not(y));
                permutohedron::heap_recursive(&mut buttons, |buttons| {
                    let mut main = Main::default();
                    let got = buttons
                        .iter()
                        .fold(None, |_, &btn| {
                            main.process_b0xx(B0xxEvent::new_without_time(btn, PRESSED), false)
                        })
                        .expect("final b0xx input resulted in null GC input");
                    assert_eq!(got, Input::Stick(Stick::C, c_stick));
                });
            }
        }
    }

    #[test_case(&[], Stick::A, Analog::MAX, Analog::MAX; "a_stick")]
    #[test_case(&[B0xxRaw::MX], Stick::A, P6625, P5375; "a_stick_mod_x")]
    #[test_case(&[B0xxRaw::MY], Stick::A, P3375, P7375; "a_stick_mod_y")]
    #[test_case(&[], Stick::C, Analog::MAX, Analog::MAX; "c_stick")]
    fn cardinals(buttons: &[B0xxRaw], stick: Stick, x_positive: Analog, y_positive: Analog) {
        for axis in [Axis::X, Axis::Y] {
            for dir in [POSITIVE, NEGATIVE] {
                let mut buttons = buttons
                    .iter()
                    .copied()
                    .chain(std::iter::once((stick, axis, dir).into()))
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
                            main.process_b0xx(B0xxEvent::new_without_time(btn, PRESSED), false)
                        })
                        .expect("final b0xx input resulted in null GC input");
                    assert_eq!(got, Input::Stick(stick, want));
                });
            }
        }
    }

    #[test]
    fn dpad() {
        for axis in [Axis::X, Axis::Y] {
            for dir in [POSITIVE, NEGATIVE] {
                let mut main = Main::default();
                let got =
                    main.process_b0xx(B0xxEvent::new_without_time(B0xxRaw::MX, PRESSED), false);
                assert_eq!(got, None);
                let got =
                    main.process_b0xx(B0xxEvent::new_without_time(B0xxRaw::MY, PRESSED), false);
                assert_eq!(got, None);
                let got = main.process_b0xx(
                    B0xxEvent::new_without_time((Stick::C, axis, dir).into(), PRESSED),
                    false,
                );
                assert_eq!(got, Some(Input::Button(Button::DPad(axis, dir), PRESSED)));
            }
        }
    }

    // When a C-stick button is acting as dpad, and one of the modifiers is
    // released, diagonals should not be modified.
    #[test]
    fn dpad_not_modify() {
        for ((c_axis, c_dir), (x_dir, y_dir)) in CARDINALS.into_iter().cartesian_product(DIAGONALS)
        {
            let mut main = Main::default();
            let _ = main.process_b0xx(B0xxEvent::new_without_time(B0xxRaw::MX, PRESSED), false);
            let _ = main.process_b0xx(B0xxEvent::new_without_time(B0xxRaw::MY, PRESSED), false);
            let _ = main.process_b0xx(
                B0xxEvent::new_without_time((Stick::C, c_axis, c_dir).into(), PRESSED),
                false,
            );
            let _ = main.process_b0xx(B0xxEvent::new_without_time(B0xxRaw::MY, RELEASED), false);
            let _ = main.process_b0xx(
                B0xxEvent::new_without_time((Stick::A, Axis::X, x_dir).into(), PRESSED),
                false,
            );
            let got = main.process_b0xx(
                B0xxEvent::new_without_time((Stick::A, Axis::Y, y_dir).into(), PRESSED),
                false,
            );
            let want = (P7375.neg_not(x_dir), P3125.neg_not(y_dir));
            assert_eq!(got, Some(Input::Stick(Stick::A, want)),);
        }
    }

    #[test]
    fn tilt_fsmash() {
        for x_dir in [POSITIVE, NEGATIVE] {
            for y_dir in [POSITIVE, NEGATIVE] {
                let mut main = Main::default();
                let got =
                    main.process_b0xx(B0xxEvent::new_without_time(B0xxRaw::MX, PRESSED), false);
                assert_eq!(got, None);
                let got = main.process_b0xx(
                    B0xxEvent::new_without_time((Stick::A, Axis::Y, y_dir).into(), PRESSED),
                    false,
                );
                assert_eq!(
                    got,
                    Some(Input::Stick(Stick::A, (P0000, P5375.neg_not(y_dir))))
                );
                let got = main.process_b0xx(
                    B0xxEvent::new_without_time((Stick::C, Axis::X, x_dir).into(), PRESSED),
                    false,
                );
                assert_eq!(
                    got,
                    Some(Input::Stick(
                        Stick::C,
                        (P8125.neg_not(x_dir), P2875.neg_not(y_dir))
                    ))
                );
                let got = main.process_b0xx(
                    B0xxEvent::new_without_time((Stick::C, Axis::X, x_dir).into(), RELEASED),
                    false,
                );
                assert_eq!(got, Some(Input::Stick(Stick::C, (P0000, P0000))));
            }
        }
    }

    #[test]
    fn accidental_side_b() {
        for dir in [POSITIVE, NEGATIVE] {
            let left_right = (Stick::A, Axis::X, dir).into();
            let mut buttons = vec![B0xxRaw::MY, B0xxRaw::B, left_right];
            permutohedron::heap_recursive(&mut buttons, |buttons| {
                let mut main = Main::default();
                let got = buttons
                    .iter()
                    .fold(None, |_, &btn| {
                        main.process_b0xx(B0xxEvent::new_without_time(btn, PRESSED), false)
                    })
                    .expect("final b0xx input resulted in null GC input");
                let want = match *buttons.last().unwrap() {
                    B0xxRaw::B => {
                        Input::ModifiedPress((P6625.neg_not(dir), P0000), ButtonImpure::B)
                    }
                    _ => Input::Stick(Stick::A, (P6625.neg_not(dir), P0000)),
                };
                assert_eq!(got, want);
            });
        }
    }

    #[test]
    fn ledgedash_optimization() {
        for modifier in [B0xxRaw::MX, B0xxRaw::MY] {
            let mut buttons = [B0xxRaw::Left, B0xxRaw::Right, modifier];
            permutohedron::heap_recursive(&mut buttons, |buttons| {
                let mut main = Main::default();
                let got = buttons.iter().fold(None, |_, &btn| {
                    main.process_b0xx(B0xxEvent::new_without_time(btn, PRESSED), false)
                });
                let want = match (*buttons.last().unwrap()).into() {
                    B0xx::Impure(Impure::ModX) | B0xx::Impure(Impure::ModY) => None,
                    B0xx::Impure(Impure::Stick(Stick::A, Axis::X, dir)) => {
                        Some(Input::Stick(Stick::A, (Analog::MAX.neg_not(dir), P0000)))
                    }
                    btn => panic!("unexpected button: {:?}", btn),
                };
                assert_eq!(got, want);
            })
        }
    }
}
