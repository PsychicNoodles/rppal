// Copyright (c) 2017-2018 Rene van der Meer
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the "Software"),
// to deal in the Software without restriction, including without limitation
// the rights to use, copy, modify, merge, publish, distribute, sublicense,
// and/or sell copies of the Software, and to permit persons to whom the
// Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
// THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

#![allow(dead_code)]

use libc::{c_int, c_ulong, ioctl};
use std::io;
use std::mem::size_of;
use std::result;

pub type Result<T> = result::Result<T, io::Error>;

const NRBITS: u8 = 8;
const TYPEBITS: u8 = 8;
const SIZEBITS: u8 = 14;
const DIRBITS: u8 = 2;

const NRSHIFT: u8 = 0;
const TYPESHIFT: u8 = (NRSHIFT + NRBITS);
const SIZESHIFT: u8 = (TYPESHIFT + TYPEBITS);
const DIRSHIFT: u8 = (SIZESHIFT + SIZEBITS);

const DIR_NONE: c_ulong = 0;
const DIR_WRITE: c_ulong = 1 << DIRSHIFT;
const DIR_READ: c_ulong = 2 << DIRSHIFT;

const SIZE_U8: c_ulong = (size_of::<u8>() as c_ulong) << SIZESHIFT;
const SIZE_U32: c_ulong = (size_of::<u32>() as c_ulong) << SIZESHIFT;

fn parse_retval(retval: c_int) -> Result<i32> {
    if retval == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(retval)
    }
}

pub mod spidev {
    use super::*;

    const TYPE_SPI: c_ulong = (b'k' as c_ulong) << TYPESHIFT;

    const NR_MESSAGE: c_ulong = 0 << NRSHIFT;
    const NR_MODE: c_ulong = 1 << NRSHIFT;
    const NR_LSB_FIRST: c_ulong = 2 << NRSHIFT;
    const NR_BITS_PER_WORD: c_ulong = 3 << NRSHIFT;
    const NR_MAX_SPEED_HZ: c_ulong = 4 << NRSHIFT;
    const NR_MODE32: c_ulong = 5 << NRSHIFT;

    const REQ_RD_MODE: c_ulong = (DIR_READ | TYPE_SPI | NR_MODE | SIZE_U8);
    const REQ_RD_LSB_FIRST: c_ulong = (DIR_READ | TYPE_SPI | NR_LSB_FIRST | SIZE_U8);
    const REQ_RD_BITS_PER_WORD: c_ulong = (DIR_READ | TYPE_SPI | NR_BITS_PER_WORD | SIZE_U8);
    const REQ_RD_MAX_SPEED_HZ: c_ulong = (DIR_READ | TYPE_SPI | NR_MAX_SPEED_HZ | SIZE_U32);
    const REQ_RD_MODE_32: c_ulong = (DIR_READ | TYPE_SPI | NR_MODE32 | SIZE_U32);

    const REQ_WR_MODE: c_ulong = (DIR_WRITE | TYPE_SPI | NR_MODE | SIZE_U8);
    const REQ_WR_LSB_FIRST: c_ulong = (DIR_WRITE | TYPE_SPI | NR_LSB_FIRST | SIZE_U8);
    const REQ_WR_BITS_PER_WORD: c_ulong = (DIR_WRITE | TYPE_SPI | NR_BITS_PER_WORD | SIZE_U8);
    const REQ_WR_MAX_SPEED_HZ: c_ulong = (DIR_WRITE | TYPE_SPI | NR_MAX_SPEED_HZ | SIZE_U32);
    const REQ_WR_MODE_32: c_ulong = (DIR_WRITE | TYPE_SPI | NR_MODE32 | SIZE_U32);

    const REQ_WR_MESSAGE: c_ulong = (DIR_WRITE | TYPE_SPI | NR_MESSAGE);

    pub const MODE_CPHA: u8 = 0x01;
    pub const MODE_CPOL: u8 = 0x02;

    pub const MODE_0: u8 = 0;
    pub const MODE_1: u8 = MODE_CPHA;
    pub const MODE_2: u8 = MODE_CPOL;
    pub const MODE_3: u8 = MODE_CPOL | MODE_CPHA;

    pub const MODE_CS_HIGH: u8 = 0x04;
    pub const MODE_LSB_FIRST: u8 = 0x08;
    pub const MODE_3WIRE: u8 = 0x10;
    pub const MODE_LOOP: u8 = 0x20;
    pub const MODE_NO_CS: u8 = 0x40;
    pub const MODE_READY: u8 = 0x80;
    pub const MODE_TX_DUAL: u32 = 0x100;
    pub const MODE_TX_QUAD: u32 = 0x200;
    pub const MODE_RX_DUAL: u32 = 0x400;
    pub const MODE_RX_QUAD: u32 = 0x800;

    #[derive(Debug, PartialEq, Copy, Clone)]
    #[repr(C)]
    pub struct TransferSegment {
        // Pointer to transmit buffer, or 0.
        tx_buf: u64,
        // Pointer to receive buffer, or 0.
        rx_buf: u64,
        // Number of bytes to transfer in this segment.
        len: u32,
        // Set a different clock speed for this segment. Default = 0.
        speed_hz: u32,
        // Add a delay before the (optional) SS change and the next segment.
        delay_usecs: u16,
        // Not used, since we only support 8 bits (or 9 bits in LoSSI mode). Default = 0.
        bits_per_word: u8,
        // Set to 1 to briefly set SS High (inactive) between this segment and the next. If this is the last segment, keep SS Low (active).
        cs_change: u8,
        // Number of bits used for writing (dual/quad SPI). Default = 0.
        tx_nbits: u8,
        // Number of bits used for reading (dual/quad SPI). Default = 0.
        rx_nbits: u8,
        // Padding. Set to 0 for forward compatibility.
        pad: u16,
    }

    impl TransferSegment {
        pub fn new(read_buffer: Option<&mut [u8]>, write_buffer: Option<&[u8]>) -> TransferSegment {
            // Len will contain the length of the shortest of the supplied buffers
            let mut len: u32 = 0;

            let tx_buf = if let Some(buffer) = write_buffer {
                len = buffer.len() as u32;
                buffer.as_ptr() as u64
            } else {
                0
            };

            let rx_buf = if let Some(buffer) = read_buffer {
                if len > buffer.len() as u32 {
                    len = buffer.len() as u32;
                }
                buffer.as_ptr() as u64
            } else {
                0
            };

            TransferSegment {
                tx_buf,
                rx_buf,
                len,
                speed_hz: 0,
                delay_usecs: 0,
                bits_per_word: 0,
                cs_change: 0,
                tx_nbits: 0,
                rx_nbits: 0,
                pad: 0,
            }
        }

        pub fn len(&self) -> u32 {
            self.len
        }
    }

    pub unsafe fn mode(fd: c_int, value: &mut u8) -> Result<i32> {
        parse_retval(ioctl(fd, REQ_RD_MODE, value))
    }

    pub unsafe fn set_mode(fd: c_int, value: u8) -> Result<i32> {
        parse_retval(ioctl(fd, REQ_WR_MODE, &value))
    }

    pub unsafe fn lsb_first(fd: c_int, value: &mut u8) -> Result<i32> {
        parse_retval(ioctl(fd, REQ_RD_LSB_FIRST, value))
    }

    pub unsafe fn set_lsb_first(fd: c_int, value: u8) -> Result<i32> {
        parse_retval(ioctl(fd, REQ_WR_LSB_FIRST, &value))
    }

    pub unsafe fn bits_per_word(fd: c_int, value: &mut u8) -> Result<i32> {
        parse_retval(ioctl(fd, REQ_RD_BITS_PER_WORD, value))
    }

    pub unsafe fn set_bits_per_word(fd: c_int, value: u8) -> Result<i32> {
        parse_retval(ioctl(fd, REQ_WR_BITS_PER_WORD, &value))
    }

    pub unsafe fn clock_speed(fd: c_int, value: &mut u32) -> Result<i32> {
        parse_retval(ioctl(fd, REQ_RD_MAX_SPEED_HZ, value))
    }

    pub unsafe fn set_clock_speed(fd: c_int, value: u32) -> Result<i32> {
        parse_retval(ioctl(fd, REQ_WR_MAX_SPEED_HZ, &value))
    }

    pub unsafe fn mode32(fd: c_int, value: &mut u32) -> Result<i32> {
        parse_retval(ioctl(fd, REQ_RD_MODE_32, value))
    }

    pub unsafe fn set_mode32(fd: c_int, value: u32) -> Result<i32> {
        parse_retval(ioctl(fd, REQ_WR_MODE_32, &value))
    }

    pub unsafe fn transfer(fd: c_int, segments: &[TransferSegment]) -> Result<i32> {
        parse_retval(ioctl(
            fd,
            REQ_WR_MESSAGE
                | (((segments.len() * size_of::<TransferSegment>()) as c_ulong) << SIZESHIFT),
            segments,
        ))
    }
}