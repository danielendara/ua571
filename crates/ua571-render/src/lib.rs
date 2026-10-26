//! Shared GRiD-style monochrome renderer (desktop pixel UI + WebAssembly canvas).

#![forbid(unsafe_code)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

pub mod draw;
pub mod fb;
pub mod font;

pub use draw::render;
pub use fb::{Framebuffer, HEIGHT, WIDTH};
