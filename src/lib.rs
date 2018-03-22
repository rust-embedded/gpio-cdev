// Copyright (c) 2018 The rust-gpio-CDC Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate error_chain;
extern crate libc;
#[macro_use]
extern crate nix;

use std::sync::Arc;
use std::ffi::CStr;
use std::fs::File;
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::Path;

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
        let mut f = File::open(path)?;
        let info: ffi::gpiochip_info = unsafe { mem::uninitialized() };
        let _ = unsafe { ffi::gpio_get_chipinfo_ioctl(f.as_raw_fd(), &mut info)? };

        Ok(Chip {
            inner: Arc::new(Box::new(InnerChip {
                file: f,
                name: CStr::from_ptr(info.name.as_ptr()).to_string_lossy().into_owned(),
                label: CStr::from_ptr(info.label.as_ptr()).to_string_lossy().into_owned(),
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
            name: mem::zeroed(),
            consumer: mem::zeroed(),
        };
        let _ = unsafe { ffi::gpio_get_lineinfo_ioctl(self.inner.file.as_raw_fd(), &mut line_info)? };

        Ok(Line {
            chip: self.inner.clone(),
            offset: offset,
            flags: LineFlags::from_bits_truncate(line_info.flags),
            name: CStr::from_ptr(line_info.name.as_ptr()).to_string_lossy().into_owned(),
            consumer: CStr::from_ptr(line_info.consumer.as_ptr()).to_string_lossy().into_owned(),
        })
    }
}

#[derive(Debug)]
pub struct Line {
    chip: Arc<Box<InnerChip>>,
    offset: u32,
    flags: LineFlags,
    name: String,
    consumer: String,
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
}

#[derive(Debug)]
pub struct LineHandle {
    file: File,
}
