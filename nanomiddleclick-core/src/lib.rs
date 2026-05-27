mod config;
mod gesture_fsm;

pub use config::{Config, MouseClickMode};
pub use gesture_fsm::{GestureEngine, GestureOutcome, TouchContact};
pub use nanomiddleclick_input::{
    MouseAction, MouseEventKind, TouchDeviceKind, TouchSource,
};
