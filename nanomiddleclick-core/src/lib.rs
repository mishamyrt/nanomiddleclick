mod config;
mod gesture_fsm;

pub use config::Config;
pub use gesture_fsm::{
    GestureEngine, GestureOutcome, MouseAction, MouseEventKind, TouchContact,
    TouchSource,
};
