# plmpeg

`plmpeg` is a safe, pure-Rust MPEG-1 video elementary-stream decoder. It is
small enough for emulators and embedded media paths that need to feed a
hardware-style FIFO rather than hand a whole file to a multimedia framework.

The decoder:

- accepts input split at arbitrary byte boundaries;
- emits planar Y/Cb/Cr 4:2:0 frames in display order;
- retains only compressed data needed for the incoming picture;
- reports malformed input without panicking;
- has no C compiler, FFI, demuxer, audio, threading, or platform dependencies;
- forbids unsafe Rust.

```rust,no_run
use plmpeg::{DecodeStatus, Decoder};

let mut decoder = Decoder::new();
let mpeg1_elementary_stream_chunk: &[u8] = &[];
decoder.push(mpeg1_elementary_stream_chunk)?;

loop {
    match decoder.decode()? {
        DecodeStatus::FrameReady => {
            let frame = decoder.frame().expect("FrameReady carries a frame");
            // frame.y, frame.cb, and frame.cr are planar 4:2:0 data.
        }
        DecodeStatus::NeedMoreData => break,
        DecodeStatus::EndOfStream => break,
    }
}

# Ok::<(), plmpeg::DecodeError>(())
```

Call `finish()` after the last input byte, then continue decoding until
`EndOfStream`; this flushes the last reference frame. `reset()` makes the
decoder ready for an unrelated stream.

## Scope and attribution

This crate decodes MPEG-1 video. It intentionally does not demultiplex MPEG
program streams, decode MP2 audio, schedule presentation, or convert pixels.

The reconstruction core descends from Native32Emu's BSD-3-Clause pure-Rust
port of Dominic Szablewski's MIT-licensed PL_MPEG. See [NOTICE.md](NOTICE.md),
[LICENSE](LICENSE), and [LICENSE-PL_MPEG](LICENSE-PL_MPEG).
