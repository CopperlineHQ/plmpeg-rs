use plmpeg::{
    DecodeError, DecodeStatus, Decoder, Frame, FrameRate, Plane, Sequence, SequenceError,
};

const STREAM: &str = include_str!("fixtures/streaming-48x32.m1v.b64");
const REFERENCE: &str = include_str!("fixtures/streaming-48x32.ref.yuv.b64");

fn decode_base64(text: &str) -> Vec<u8> {
    let mut output = Vec::new();
    let mut accumulator = 0_u32;
    let mut bits = 0_u32;
    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b' ' | b'\t' | b'\r' | b'\n' => continue,
            _ => panic!("invalid base64 test fixture"),
        };
        accumulator = (accumulator << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push(u8::try_from(accumulator >> bits).expect("decoded base64 byte"));
            accumulator &= (1 << bits) - 1;
        }
    }
    output
}

fn append_visible(output: &mut Vec<u8>, plane: &Plane, width: usize, height: usize) {
    for row in plane.data.chunks_exact(plane.width).take(height) {
        output.extend_from_slice(&row[..width]);
    }
}

fn visible_frame(frame: &Frame) -> Vec<u8> {
    let mut output = Vec::with_capacity(frame.width * frame.height * 3 / 2);
    append_visible(&mut output, &frame.y, frame.width, frame.height);
    let chroma_width = frame.width.div_ceil(2);
    let chroma_height = frame.height.div_ceil(2);
    append_visible(&mut output, &frame.cb, chroma_width, chroma_height);
    append_visible(&mut output, &frame.cr, chroma_width, chroma_height);
    output
}

fn drain(decoder: &mut Decoder, frames: &mut Vec<Vec<u8>>) -> Result<DecodeStatus, DecodeError> {
    loop {
        match decoder.decode()? {
            DecodeStatus::FrameReady => {
                frames.push(visible_frame(
                    decoder.frame().expect("frame ready without frame"),
                ));
            }
            status => return Ok(status),
        }
    }
}

fn decode_in_chunks(input: &[u8], chunk_size: usize) -> (Sequence, Vec<Vec<u8>>) {
    let mut decoder = Decoder::new();
    let mut frames = Vec::new();
    for chunk in input.chunks(chunk_size) {
        decoder.push(chunk).expect("append compressed input");
        assert_eq!(
            drain(&mut decoder, &mut frames).expect("decode fragment"),
            DecodeStatus::NeedMoreData
        );
    }

    decoder.finish();
    assert_eq!(
        drain(&mut decoder, &mut frames).expect("flush decoder"),
        DecodeStatus::EndOfStream
    );
    assert_eq!(decoder.frames_decoded(), frames.len() as u64);
    (decoder.sequence().expect("sequence metadata"), frames)
}

#[test]
fn fragmented_decode_is_deterministic_and_reference_bounded() {
    let input = decode_base64(STREAM);
    let reference = decode_base64(REFERENCE);
    let (sequence, expected) = decode_in_chunks(&input, input.len());

    assert_eq!(
        sequence,
        Sequence {
            width: 48,
            height: 32,
            frame_rate: FrameRate {
                numerator: 25,
                denominator: 1,
            },
            pixel_aspect_ratio: 1.0,
        }
    );
    assert_eq!(expected.len(), 8);

    for chunk_size in [1, 2, 3, 7, 31, 512] {
        let (_, fragmented) = decode_in_chunks(&input, chunk_size);
        assert_eq!(fragmented, expected, "chunk size {chunk_size}");
    }

    let frame_size = sequence.width * sequence.height * 3 / 2;
    assert_eq!(reference.len(), expected.len() * frame_size);
    for (index, (decoded, reference)) in expected
        .iter()
        .zip(reference.chunks_exact(frame_size))
        .enumerate()
    {
        let max_delta = decoded
            .iter()
            .zip(reference)
            .map(|(&actual, &wanted)| actual.abs_diff(wanted))
            .max()
            .unwrap_or_default();
        assert!(max_delta <= 3, "frame {index}: sample delta {max_delta}");
    }
}

#[test]
fn finish_rejects_more_input_and_reset_recovers() {
    let mut decoder = Decoder::new();
    decoder.finish();
    assert_eq!(decoder.push(&[0]), Err(DecodeError::InputAfterEnd));

    decoder.reset();
    assert_eq!(decoder.push(&[0]), Ok(()));
}

#[test]
fn reports_a_truncated_sequence_header() {
    let mut decoder = Decoder::new();
    decoder
        .push(&[0x00, 0x00, 0x01, 0xB3, 0x01])
        .expect("append header fragment");
    decoder.finish();
    assert_eq!(
        decoder.decode(),
        Err(DecodeError::InvalidSequence(SequenceError::TruncatedHeader))
    );
}

#[test]
fn mutated_streams_fail_safely() {
    let original = decode_base64(STREAM);
    for seed in 1_u32..=128 {
        let mut input = original.clone();
        let mut state = seed;
        for _ in 0..8 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let position = usize::try_from(state).expect("u32 fits usize") % input.len();
            input[position] ^= (state >> 24) as u8 | 1;
        }

        let mut decoder = Decoder::new();
        for chunk in input.chunks(13) {
            if decoder.push(chunk).is_err() || decoder.decode().is_err() {
                break;
            }
        }
        decoder.finish();
        for _ in 0..16 {
            match decoder.decode() {
                Ok(DecodeStatus::FrameReady) => {}
                Ok(DecodeStatus::NeedMoreData | DecodeStatus::EndOfStream) | Err(_) => break,
            }
        }
    }
}
