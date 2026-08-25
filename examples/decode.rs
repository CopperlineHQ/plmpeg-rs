//! Decode an MPEG-1 video elementary stream to planar YUV 4:2:0.

use plmpeg::{DecodeStatus, Decoder, Frame, Plane};
use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufWriter, Write};

fn write_plane(
    output: &mut impl Write,
    plane: &Plane,
    width: usize,
    height: usize,
) -> std::io::Result<()> {
    for row in plane.data.chunks_exact(plane.width).take(height) {
        output.write_all(&row[..width])?;
    }
    Ok(())
}

fn write_frame(output: &mut impl Write, frame: &Frame) -> std::io::Result<()> {
    write_plane(output, &frame.y, frame.width, frame.height)?;
    let chroma_width = frame.width.div_ceil(2);
    let chroma_height = frame.height.div_ceil(2);
    write_plane(output, &frame.cb, chroma_width, chroma_height)?;
    write_plane(output, &frame.cr, chroma_width, chroma_height)
}

fn required_arg(args: &mut impl Iterator<Item = OsString>, name: &str) -> Result<OsString, String> {
    args.next().ok_or_else(|| format!("missing {name}"))
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let input_path = required_arg(&mut args, "input.m1v")?;
    let output_path = required_arg(&mut args, "output.yuv")?;
    let chunk_size = match args.next() {
        Some(value) => value.to_string_lossy().parse::<usize>()?,
        None => 4_096,
    };
    if chunk_size == 0 {
        return Err("chunk size must be non-zero".into());
    }
    if args.next().is_some() {
        return Err("usage: decode <input.m1v> <output.yuv> [chunk-size]".into());
    }

    let input = std::fs::read(input_path)?;
    let mut output = BufWriter::new(File::create(output_path)?);
    let mut decoder = Decoder::new();
    let mut frame_count = 0_u64;

    for chunk in input.chunks(chunk_size) {
        decoder.push(chunk)?;
        while decoder.decode()? == DecodeStatus::FrameReady {
            write_frame(&mut output, decoder.frame().ok_or("missing decoded frame")?)?;
            frame_count += 1;
        }
    }

    decoder.finish();
    while decoder.decode()? == DecodeStatus::FrameReady {
        write_frame(&mut output, decoder.frame().ok_or("missing decoded frame")?)?;
        frame_count += 1;
    }
    output.flush()?;

    let sequence = decoder.sequence().ok_or("missing sequence metadata")?;
    eprintln!(
        "decoded {frame_count} frames at {}x{} ({:.6} fps)",
        sequence.width,
        sequence.height,
        sequence.frame_rate.as_f64()
    );
    Ok(())
}
