//! This crate provides audio tools in Rust.
//!
//! What we want:
//! - Audio DSP graph implementation.
//! - DSP: backend support (for the moment jack), filters and plugins (faust, vst, ldspa).
//! - Library: audio files libraries, including metadata scanning.
//! - User interface: generic controllers over graph supporting MIDI and GUI.
//!
#![feature(associated_type_bounds)]
#![feature(ptr_offset_from)]

pub mod data;
pub mod dsp;
pub mod format;
pub mod rpc;
pub mod ui;

#[cfg(feature="build")]
pub mod build;

