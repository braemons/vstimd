//! The IPC surface: the ZMQ transport plus the protobuf command dispatcher
//! it feeds. `handle_request` is an inherent method on `SceneState`, split
//! across the command-group modules here so the dispatcher and the response
//! helpers it builds replies from stay together.
//!
//! `proto` deliberately stays at the crate root: the scene and the web
//! control surface speak it too, so it is not owned by this transport.

mod animation_commands;
mod config_commands;
mod convert;
mod dispatch;
mod grating_commands;
mod scene_commands;
mod shape_commands;
mod snapshot;
mod text_commands;
mod vtl_commands;

pub mod response;
pub mod zmq_server;

pub use zmq_server::{DEFAULT_ZMQ_PORT, spawn_zmq_thread};
