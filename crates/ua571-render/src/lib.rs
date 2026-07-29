//! Shared GRiD-style monochrome renderer (desktop pixel UI + WebAssembly canvas).

#![forbid(unsafe_code)]

pub mod draw;
pub mod fb;
pub mod font;

pub use draw::render;
pub use fb::{Framebuffer, HEIGHT, WIDTH};
