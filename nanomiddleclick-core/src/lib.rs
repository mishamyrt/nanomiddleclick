mod config;
mod gesture_fsm;

pub use config::{Config, MouseClickMode};
pub use gesture_fsm::{
    GestureEngine, GestureOutcome, MouseAction, MouseEventKind, TouchContact,
    TouchDeviceKind, TouchSource,
};
