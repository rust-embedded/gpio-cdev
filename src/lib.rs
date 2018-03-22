// Copyright (c) 2018 The rust-gpio-CDC Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate error_chain;
extern crate libc;
#[macro_use]
extern crate nix;

use std::cmp::min;
use std::ffi::CStr;
use std::fs::File;
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;
use std::ptr;
use std::slice;
use std::sync::Arc;

mod ffi;

mod errors {
    error_chain! {
        types {
            Error,
            ErrorKind,
            ResultExt,
            Result;
        }

        foreign_links {
            Nix(::nix::Error);
            Io(::std::io::Error);

        }
    }
}

pub use errors::*;

// Informational Flags
bitflags! {
    pub struct LineFlags: libc::uint32_t {
        const KERNEL = (1 << 0);
        const IS_OUT = (1 << 1);
        const ACTIVE_LOW = (1 << 2);
        const OPEN_DRAIN = (1 << 3);
        const OPEN_SOURCE = (1 << 4);
    }
}

#[derive(Debug)]
struct InnerChip {
    pub file: File,
    pub name: String,
    pub label: String,
    pub lines: u32,
}

#[derive(Debug)]
pub struct Chip {
    inner: Arc<Box<InnerChip>>
}

impl Chip {

    /// Open the GPIO Chip at the provided path (/dev/gpiochip<N>)
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Chip> {
        let f = File::open(path)?;
        let mut info: ffi::gpiochip_info = unsafe { mem::uninitialized() };
        let _ = unsafe { ffi::gpio_get_chipinfo_ioctl(f.as_raw_fd(), &mut info)? };

        Ok(Chip {
            inner: Arc::new(Box::new(InnerChip {
                file: f,
                name: unsafe { CStr::from_ptr(info.name.as_ptr()).to_string_lossy().into_owned() },
                label: unsafe { CStr::from_ptr(info.label.as_ptr()).to_string_lossy().into_owned() },
                lines: info.lines,
            }))
        })
    }

    pub fn name(&self) -> &str {
        self.inner.name.as_str()
    }

    pub fn label(&self) -> &str {
        self.inner.label.as_str()
    }

    pub fn num_lines(&self) -> u32 {
        self.inner.lines
    }

    pub fn get_line(&mut self, offset: u32) -> Result<Line> {
        let mut line_info = ffi::gpioline_info {
            line_offset: offset,
            flags: 0,
            name: [0; 32],
            consumer: [0; 32],
        };
        let _ = unsafe { ffi::gpio_get_lineinfo_ioctl(self.inner.file.as_raw_fd(), &mut line_info)? };

        Ok(Line {
            chip: self.inner.clone(),
            offset: offset,
            flags: LineFlags::from_bits_truncate(line_info.flags),
            name: unsafe { CStr::from_ptr(line_info.name.as_ptr()).to_string_lossy().into_owned() },
            consumer: unsafe { CStr::from_ptr(line_info.consumer.as_ptr()).to_string_lossy().into_owned() },
        })
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    chip: Arc<Box<InnerChip>>,
    offset: u32,
    flags: LineFlags,
    name: String,
    consumer: String,
}

// Line Request Flags
bitflags! {
    pub struct RequestFlags: libc::uint32_t {
        const INPUT = (1 << 0);
        const OUTPUT = (1 << 1);
        const ACTIVE_LOW = (1 << 2);
        const OPEN_DRAIN = (1 << 3);
        const OPEN_SOURCE = (1 << 4);
    }
}

unsafe fn rstr_lcpy(dst: *mut libc::c_char, src: &str, length: usize) {
    // NOTE: unsafe because dst pointer is not null checked
    let copylen = min(src.len() + 1, length);
    ptr::copy_nonoverlapping(src.as_bytes().as_ptr() as *const libc::c_char, dst, copylen - 1);
    slice::from_raw_parts_mut(dst, length)[copylen - 1] = 0;
}

impl Line {
    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn flags(&self) -> LineFlags {
        self.flags
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn consumer(&self) -> &str {
        self.consumer.as_str()
    }

    pub fn chip(&self) -> Chip {
        Chip {
            inner: self.chip.clone()
        }
    }

    pub fn request(&self, flags: RequestFlags, consumer: &str) -> Result<LineHandle> {
        // prepare the request; the kernel consumes some of these values and will
        // set the fd for us.
        let mut request = ffi::gpiohandle_request {
            lineoffsets: unsafe { mem::zeroed() },
            flags: flags.bits(),
            default_values: unsafe { mem::zeroed() },
            consumer_label: unsafe { mem::zeroed() },
            lines: 1,
            fd: 0,
        };
        request.lineoffsets[0] = self.offset;
        unsafe { rstr_lcpy(request.consumer_label[..].as_mut_ptr(), consumer, request.consumer_label.len()) };
        let _ = unsafe { ffi::gpio_get_linehandle_ioctl(self.chip.file.as_raw_fd(), &mut request) }?;
        Ok(LineHandle {
            line: self.clone(),
            flags: flags,
            file: unsafe { File::from_raw_fd(request.fd) },
        })
    }
}

#[derive(Debug)]
pub struct LineHandle {
    line: Line,
    flags: RequestFlags,
    file: File,
}

impl LineHandle {
    pub fn get_value(&self) -> Result<u8> {
        let mut data: ffi::gpiohandle_data = unsafe { mem::zeroed() };
        let _ = unsafe { ffi::gpiohandle_get_line_values_ioctl(self.file.as_raw_fd(), &mut data)? };
        Ok(data.values[0])
    }

    pub fn set_value(&self, value: u8) -> Result<()> {
        let mut data: ffi::gpiohandle_data = unsafe { mem::zeroed() };
        data.values[0] = value;
        let _ = unsafe { ffi::gpiohandle_set_line_values_ioctl(self.file.as_raw_fd(), &mut data)? };
        Ok(())
    }

    pub fn line(&self) -> &Line {
        &self.line
    }
}
