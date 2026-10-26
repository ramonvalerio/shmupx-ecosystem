//! Bounded context `input`: hotkeys globais de joystick/controle em segundo plano
//! (push-to-talk do RobotX AI). Ver docs/adr/003-agente-robotx-e-salas.md.

pub mod gamepad;
pub mod hotkey;

pub use gamepad::spawn_listener;
pub use hotkey::{HotkeyBinding, HotkeyEvent};
