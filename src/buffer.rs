// SPDX-License-Identifier: BSD-3-Clause

// MSB-first bit reader for MPEG streams.
//
// Ported to Rust from PL_MPEG (https://github.com/phoboslab/pl_mpeg) by
// Dominic Szablewski, originally MIT licensed. Adapted here into an
// appendable, bounded streaming buffer for fragmented elementary streams.

/// A bit reader over an owned byte buffer.
pub struct Buffer {
    bytes: Vec<u8>,
    /// Current read position, in bits.
    bit_index: usize,
    /// Bytes discarded from the front of the stream.
    stream_offset: u64,
    /// No more input will be appended.
    finished: bool,
    /// A bit/VLC read encountered malformed or truncated picture data.
    failed: bool,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            bytes: Vec::new(),
            bit_index: 0,
            stream_offset: 0,
            finished: false,
            failed: false,
        }
    }

    pub fn append(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub fn finish(&mut self) {
        self.finished = true;
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn failed(&self) -> bool {
        self.failed
    }

    pub fn buffered_bytes(&self) -> usize {
        self.bytes.len()
    }

    pub fn stream_position(&self) -> u64 {
        self.stream_offset + (self.bit_index >> 3) as u64
    }

    /// Total length of the buffer in bits.
    fn len_bits(&self) -> usize {
        self.bytes.len() << 3
    }

    /// Whether at least `count` more bits are available.
    pub fn has(&self, count: usize) -> bool {
        self.len_bits().saturating_sub(self.bit_index) >= count
    }

    /// Read `count` bits (0..=32) MSB-first as an unsigned integer.
    /// Returns 0 and poisons the current picture if insufficient data remains.
    pub fn read(&mut self, mut count: usize) -> u32 {
        if count > 32 || !self.has(count) {
            self.failed = true;
            return 0;
        }
        let mut value: u32 = 0;
        while count > 0 {
            let current_byte = u32::from(self.bytes[self.bit_index >> 3]);
            let remaining = 8 - (self.bit_index & 7); // remaining bits in this byte
            let read = remaining.min(count); // bits read in this run
            let shift = remaining - read;
            let mask = 0xffu32 >> (8 - read);
            value = (value << read) | ((current_byte & (mask << shift)) >> shift);
            self.bit_index += read;
            count -= read;
        }
        value
    }

    /// Align the read position to the next byte boundary.
    pub fn align(&mut self) {
        self.bit_index = ((self.bit_index + 7) >> 3) << 3;
    }

    /// Skip `count` bits if available.
    pub fn skip(&mut self, count: usize) {
        if self.has(count) {
            self.bit_index += count;
        } else {
            self.failed = true;
        }
    }

    /// Align, then scan forward for the next `00 00 01 xx` start code.
    /// Consumes up to and including the 4 marker bytes and returns `xx`,
    /// or -1 if none is found.
    pub fn next_start_code(&mut self) -> i32 {
        self.align();
        while self.has(4 << 3) {
            let byte_index = self.bit_index >> 3;
            if self.bytes[byte_index] == 0x00
                && self.bytes[byte_index + 1] == 0x00
                && self.bytes[byte_index + 2] == 0x01
            {
                self.bit_index = (byte_index + 4) << 3;
                return i32::from(self.bytes[byte_index + 3]);
            }
            self.bit_index += 8;
        }
        -1
    }

    /// Scan forward until the start code `code` (or end of buffer).
    pub fn find_start_code(&mut self, code: i32) -> i32 {
        loop {
            let current = self.next_start_code();
            if current == code || current == -1 {
                return current;
            }
        }
    }

    /// Peek whether start code `code` appears ahead, without consuming.
    pub fn has_start_code(&mut self, code: i32) -> i32 {
        let previous = self.bit_index;
        let current = self.find_start_code(code);
        self.bit_index = previous;
        current
    }

    /// Decode a value using a binary VLC table (`PL_MPEG` layout). Each entry is
    /// `(index, value)`; a positive `index` points to the next node (`index +
    /// bit`), `index <= 0` terminates with `value`.
    pub fn read_vlc(&mut self, table: &[(i16, i16)]) -> i16 {
        let mut index: i16 = 0;
        for _ in 0..32 {
            if !self.has(1) {
                self.failed = true;
                return 0;
            }
            let bit = i16::from(self.read(1) != 0);
            let node = index + bit;
            if node < 0 {
                self.failed = true;
                return 0;
            }
            let Some(&(next, value)) = table.get(usize::from(node.unsigned_abs())) else {
                self.failed = true;
                return 0;
            };
            if next < 0 {
                self.failed = true;
                return 0;
            }
            if next == 0 {
                return value;
            }
            index = next;
        }
        self.failed = true;
        0
    }

    /// Decode a value using an unsigned binary VLC table.
    pub fn read_vlc_uint(&mut self, table: &[(i16, u16)]) -> u16 {
        let mut index: i16 = 0;
        for _ in 0..32 {
            if !self.has(1) {
                self.failed = true;
                return 0;
            }
            let bit = i16::from(self.read(1) != 0);
            let node = index + bit;
            if node < 0 {
                self.failed = true;
                return 0;
            }
            let Some(&(next, value)) = table.get(usize::from(node.unsigned_abs())) else {
                self.failed = true;
                return 0;
            };
            if next < 0 {
                self.failed = true;
                return 0;
            }
            if next == 0 {
                return value;
            }
            index = next;
        }
        self.failed = true;
        0
    }

    /// Peek `bit_count` bits without consuming them; return whether non-zero.
    pub fn peek_non_zero(&mut self, bit_count: usize) -> bool {
        if !self.has(bit_count) {
            return false;
        }
        let saved = self.bit_index;
        let val = self.read(bit_count);
        self.bit_index = saved;
        val != 0
    }

    /// Drop byte-aligned data already consumed while retaining any partial
    /// byte and the unconsumed tail. Decoder reference pictures carry every
    /// dependency needed after a picture boundary, so compressed bytes do
    /// not need to grow for the lifetime of a stream.
    pub fn discard_read_bytes(&mut self) {
        let consumed = self.bit_index >> 3;
        if consumed == 0 {
            return;
        }
        self.bytes.drain(..consumed);
        self.bit_index &= 7;
        self.stream_offset += consumed as u64;
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_bits_msb_first() {
        // 0b1011_0010, 0b1100_0000
        let mut b = Buffer::new();
        b.append(&[0b1011_0010, 0b1100_0000]);
        assert_eq!(b.read(1), 1);
        assert_eq!(b.read(3), 0b011);
        assert_eq!(b.read(4), 0b0010);
        assert_eq!(b.read(2), 0b11);
    }

    #[test]
    fn test_has_and_ended() {
        let mut b = Buffer::new();
        b.append(&[0xff, 0xff]);
        assert!(b.has(16));
        assert!(!b.has(17));
        b.read(16);
        assert!(!b.has(1));
    }

    #[test]
    fn test_read_past_end_returns_zero() {
        let mut b = Buffer::new();
        b.append(&[0xab]);
        assert_eq!(b.read(16), 0); // not enough bits
        assert_eq!(b.read(8), 0xab);
    }

    #[test]
    fn test_align() {
        let mut b = Buffer::new();
        b.append(&[0xff, 0x0f]);
        b.read(3);
        b.align();
        assert_eq!(b.read(8), 0x0f);
    }

    #[test]
    fn test_next_start_code() {
        // padding, then 00 00 01 B3, then a byte
        let mut b = Buffer::new();
        b.append(&[0x12, 0x34, 0x00, 0x00, 0x01, 0xb3, 0x99]);
        assert_eq!(b.next_start_code(), 0xb3);
        assert_eq!(b.read(8), 0x99);
    }

    #[test]
    fn test_find_start_code_skips_others() {
        // 00 00 01 BA ... 00 00 01 E0
        let mut b = Buffer::new();
        b.append(&[0x00, 0x00, 0x01, 0xba, 0x55, 0x00, 0x00, 0x01, 0xe0, 0x42]);
        assert_eq!(b.find_start_code(0xe0), 0xe0);
        assert_eq!(b.read(8), 0x42);
    }

    #[test]
    fn test_has_start_code_does_not_consume() {
        let mut b = Buffer::new();
        b.append(&[0x00, 0x00, 0x01, 0xe0, 0x42]);
        assert_eq!(b.has_start_code(0xe0), 0xe0);
        // position unchanged: still at the start
        assert_eq!(b.next_start_code(), 0xe0);
    }

    #[test]
    fn start_code_survives_every_fragment_boundary() {
        let stream = [0x55, 0x00, 0x00, 0x01, 0xB3, 0x99];
        for split in 0..5 {
            let mut b = Buffer::new();
            b.append(&stream[..split]);
            assert_eq!(b.next_start_code(), -1);
            b.append(&stream[split..]);
            assert_eq!(b.next_start_code(), 0xB3, "split {split}");
            assert_eq!(b.read(8), 0x99);
        }
    }

    #[test]
    fn discard_preserves_unconsumed_data_and_absolute_position() {
        let mut b = Buffer::new();
        b.append(&[1, 2, 3, 4]);
        assert_eq!(b.read(12), 0x10);
        b.discard_read_bytes();
        assert_eq!(b.stream_position(), 1);
        assert_eq!(b.read(4), 2);
        assert_eq!(b.read(8), 3);
    }
}
