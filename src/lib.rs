// RustVoxel - an original voxel sandbox in pure Rust (zero crates).
// Library crate: shared game logic used by both the client binary and the
// dedicated server binary. Platform FFI modules are gated behind `windows`
// or the `fficheck` feature so tests run everywhere.
#![allow(dead_code)]

pub mod client;
pub mod entity_data;
pub mod entity_models;
pub mod font;
pub mod json;
pub mod math;
pub mod mcproto;
pub mod mesher;
pub mod mobs;
pub mod models;
pub mod nbt;
pub mod net;
pub mod noise;
pub mod pack;
pub mod player;
pub mod raycast;
pub mod server;
pub mod ui;
pub mod vanilla_data;
pub mod world;

#[cfg(any(windows, feature = "fficheck"))]
pub mod app;
#[cfg(any(windows, feature = "fficheck"))]
pub mod gl;
#[cfg(any(windows, feature = "fficheck"))]
pub mod hud;
#[cfg(any(windows, feature = "fficheck"))]
pub mod renderer;
#[cfg(any(windows, feature = "fficheck"))]
pub mod win32;
