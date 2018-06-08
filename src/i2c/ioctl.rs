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
use std::result;

pub type Result<T> = result::Result<T, io::Error>;

fn parse_retval(retval: c_int) -> Result<i32> {
    if retval == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(retval)
    }
}

// Capabilities returned by REQ_FUNCS
const FUNC_I2C: c_ulong = 0x01;
const FUNC_10BIT_ADDR: c_ulong = 0x02;
const FUNC_PROTOCOL_MANGLING: c_ulong = 0x04;
const FUNC_SMBUS_PEC: c_ulong = 0x08;
const FUNC_NOSTART: c_ulong = 0x10;
const FUNC_SLAVE: c_ulong = 0x20;
const FUNC_SMBUS_BLOCK_PROC_CALL: c_ulong = 0x8000;
const FUNC_SMBUS_QUICK: c_ulong = 0x010000;
const FUNC_SMBUS_READ_BYTE: c_ulong = 0x020000;
const FUNC_SMBUS_WRITE_BYTE: c_ulong = 0x040000;
const FUNC_SMBUS_READ_BYTE_DATA: c_ulong = 0x080000;
const FUNC_SMBUS_WRITE_BYTE_DATA: c_ulong = 0x100000;
const FUNC_SMBUS_READ_WORD_DATA: c_ulong = 0x200000;
const FUNC_SMBUS_WRITE_WORD_DATA: c_ulong = 0x400000;
const FUNC_SMBUS_PROC_CALL: c_ulong = 0x800000;
const FUNC_SMBUS_READ_BLOCK_DATA: c_ulong = 0x01000000;
const FUNC_SMBUS_WRITE_BLOCK_DATA: c_ulong = 0x02000000;
const FUNC_SMBUS_READ_I2C_BLOCK: c_ulong = 0x04000000;
const FUNC_SMBUS_WRITE_I2C_BLOCK: c_ulong = 0x08000000;
const FUNC_SMBUS_HOST_NOTIFY: c_ulong = 0x10000000;

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct Capabilities {
    funcs: c_ulong,
}

impl Capabilities {
    fn new(funcs: c_ulong) -> Capabilities {
        Capabilities { funcs }
    }

    pub fn i2c(&self) -> bool {
        (self.funcs & FUNC_I2C) > 0
    }

    pub fn slave(&self) -> bool {
        (self.funcs & FUNC_SLAVE) > 0
    }

    pub fn addr_10bit(&self) -> bool {
        (self.funcs & FUNC_10BIT_ADDR) > 0
    }

    pub fn i2c_block_read(&self) -> bool {
        (self.funcs & FUNC_SMBUS_READ_I2C_BLOCK) > 0
    }

    pub fn i2c_block_write(&self) -> bool {
        (self.funcs & FUNC_SMBUS_WRITE_I2C_BLOCK) > 0
    }

    pub fn protocol_mangling(&self) -> bool {
        (self.funcs & FUNC_PROTOCOL_MANGLING) > 0
    }

    pub fn nostart(&self) -> bool {
        (self.funcs & FUNC_NOSTART) > 0
    }

    pub fn smbus_quick_command(&self) -> bool {
        (self.funcs & FUNC_SMBUS_QUICK) > 0
    }

    pub fn smbus_receive_byte(&self) -> bool {
        (self.funcs & FUNC_SMBUS_READ_BYTE) > 0
    }

    pub fn smbus_send_byte(&self) -> bool {
        (self.funcs & FUNC_SMBUS_WRITE_BYTE) > 0
    }

    pub fn smbus_read_byte(&self) -> bool {
        (self.funcs & FUNC_SMBUS_READ_BYTE_DATA) > 0
    }

    pub fn smbus_write_byte(&self) -> bool {
        (self.funcs & FUNC_SMBUS_WRITE_BYTE_DATA) > 0
    }

    pub fn smbus_read_word(&self) -> bool {
        (self.funcs & FUNC_SMBUS_READ_WORD_DATA) > 0
    }

    pub fn smbus_write_word(&self) -> bool {
        (self.funcs & FUNC_SMBUS_WRITE_WORD_DATA) > 0
    }

    pub fn smbus_process_call(&self) -> bool {
        (self.funcs & FUNC_SMBUS_PROC_CALL) > 0
    }

    pub fn smbus_block_read(&self) -> bool {
        (self.funcs & FUNC_SMBUS_READ_BLOCK_DATA) > 0
    }

    pub fn smbus_block_write(&self) -> bool {
        (self.funcs & FUNC_SMBUS_WRITE_BLOCK_DATA) > 0
    }

    pub fn smbus_block_process_call(&self) -> bool {
        (self.funcs & FUNC_SMBUS_BLOCK_PROC_CALL) > 0
    }

    pub fn smbus_pec(&self) -> bool {
        (self.funcs & FUNC_SMBUS_PEC) > 0
    }

    pub fn smbus_host_notify(&self) -> bool {
        (self.funcs & FUNC_SMBUS_HOST_NOTIFY) > 0
    }
}

// ioctl() requests supported by i2cdev
const REQ_RETRIES: c_ulong = 0x0701; // How many retries when waiting for an ACK
const REQ_TIMEOUT: c_ulong = 0x0702; // Timeout in 10ms units
const REQ_SLAVE: c_ulong = 0x0706; // Set slave address
const REQ_SLAVE_FORCE: c_ulong = 0x0703; // Set slave address, even if it's already in use by a driver
const REQ_TENBIT: c_ulong = 0x0704; // Use 10-bit slave addresses
const REQ_FUNCS: c_ulong = 0x0705; // Read I2C bus capabilities
const REQ_RDWR: c_ulong = 0x0707; // Combined read/write transfer with a single STOP
const REQ_PEC: c_ulong = 0x0708; // SMBus: Use Packet Error Checking
const REQ_SMBUS: c_ulong = 0x0720; // SMBus: Transfer

// TODO: Check if 10-bit addresses are supported by i2cdev and the underlying drivers

// All ioctl commands take an unsigned long parameter, except for
// REQ_FUNCS (pointer to an unsigned long), REQ_RDWR (pointer to
// ic2_rdwr_ioctl_data) and REQ_SMBUS (pointer to i2c_smbus_ioctl_data)

pub unsafe fn set_slave_address(fd: c_int, value: c_ulong) -> Result<i32> {
    parse_retval(ioctl(fd, REQ_SLAVE, value))
}

pub unsafe fn set_10bit(fd: c_int, value: c_ulong) -> Result<i32> {
    parse_retval(ioctl(fd, REQ_TENBIT, value))
}

pub unsafe fn set_pec(fd: c_int, value: c_ulong) -> Result<i32> {
    parse_retval(ioctl(fd, REQ_PEC, value))
}

pub unsafe fn funcs(fd: c_int) -> Result<Capabilities> {
    let mut funcs: c_ulong = 0;

    parse_retval(ioctl(fd, REQ_FUNCS, &mut funcs))?;

    Ok(Capabilities::new(funcs))
}
