// SPDX-License-Identifier: BSD-3-Clause

//! A safe, pure-Rust, incremental MPEG-1 video elementary-stream decoder.
//!
//! [`Decoder`] accepts arbitrarily fragmented input and emits reconstructed
//! Y/Cb/Cr 4:2:0 frames in display order. It deliberately does not include a
//! program-stream demultiplexer, an audio decoder, presentation scheduling,
//! or RGB conversion; those policies belong to the embedding application.

#![forbid(unsafe_code)]

mod buffer;
mod decoder;

pub use decoder::{
    DecodeError, DecodeStatus, Decoder, Frame, FrameRate, PictureError, Plane, Sequence,
    SequenceError,
};
