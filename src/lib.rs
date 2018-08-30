// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
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
use std::fs::{read_dir, File, ReadDir};
use std::mem;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice;
use std::sync::Arc;

mod ffi;
pub mod errors;

use errors::*;

unsafe fn rstr_lcpy(dst: *mut libc::c_char, src: &str, length: usize) {
    let copylen = min(src.len() + 1, length);
    ptr::copy_nonoverlapping(
        src.as_bytes().as_ptr() as *const libc::c_char,
        dst,
        copylen - 1,
    );
    slice::from_raw_parts_mut(dst, length)[copylen - 1] = 0;
}

#[derive(Debug)]
struct InnerChip {
    pub path: PathBuf,
    pub file: File,
    pub name: String,
    pub label: String,
    pub lines: u32,
}

#[derive(Debug)]
pub struct Chip {
    inner: Arc<Box<InnerChip>>,
}

pub struct ChipIterator {
    readdir: ReadDir,
}

impl Iterator for ChipIterator {
    type Item = Result<Chip>;

    fn next(&mut self) -> Option<Result<Chip>> {
        while let Some(entry) = self.readdir.next() {
            match entry {
                Ok(entry) => {
                    if entry
                        .path()
                        .as_path()
                        .to_string_lossy()
                        .contains("gpiochip")
                    {
                        return Some(Chip::new(entry.path()));
                    }
                }
                Err(e) => {
                    return Some(Err(e.into()));
                }
            }
        }

        None
    }
}

pub fn chips() -> Result<ChipIterator> {
    Ok(ChipIterator {
        readdir: read_dir("/dev")?,
    })
}
impl Chip {
    /// Open the GPIO Chip at the provided path (/dev/gpiochip<N>)
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Chip> {
        let f = File::open(path.as_ref())?;
        let mut info: ffi::gpiochip_info = unsafe { mem::uninitialized() };
        let _ = unsafe { ffi::gpio_get_chipinfo_ioctl(f.as_raw_fd(), &mut info)? };

        Ok(Chip {
            inner: Arc::new(Box::new(InnerChip {
                file: f,
                path: path.as_ref().to_path_buf(),
                name: unsafe {
                    CStr::from_ptr(info.name.as_ptr())
                        .to_string_lossy()
                        .into_owned()
                },
                label: unsafe {
                    CStr::from_ptr(info.label.as_ptr())
                        .to_string_lossy()
                        .into_owned()
                },
                lines: info.lines,
            })),
        })
    }

    pub fn path(&self) -> &Path {
        self.inner.path.as_path()
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
        Line::new(self.inner.clone(), offset)
    }

    pub fn lines(&self) -> LineIterator {
        LineIterator {
            chip: self.inner.clone(),
            idx: 0,
        }
    }
}

pub struct LineIterator {
    chip: Arc<Box<InnerChip>>,
    idx: u32,
}

impl Iterator for LineIterator {
    type Item = Result<Line>;

    fn next(&mut self) -> Option<Result<Line>> {
        if self.idx < self.chip.lines {
            // always increment; we don't want to error forever
            // if we can't get some of the lines.
            let idx = self.idx;
            self.idx += 1;
            Some(Line::new(self.chip.clone(), idx))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct Line {
    chip: Arc<Box<InnerChip>>,
    offset: u32,
    flags: LineFlags,
    name: Option<String>,
    consumer: Option<String>,
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

// Event request flags
bitflags! {
    pub struct EventRequestFlags: libc::uint32_t {
        const RISING_EDGE = (1 << 0);
        const FALLING_EDGE = (1 << 1);
        const BOTH_EDGES = Self::RISING_EDGE.bits | Self::FALLING_EDGE.bits;
    }
}

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

#[derive(Debug, PartialEq)]
pub enum LineDirection {
    In,
    Out,
}

unsafe fn cstrbuf_to_string(buf: &[libc::c_char]) -> Option<String> {
    if buf[0] == 0 {
        None
    } else {
        Some(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }
}

impl Line {
    fn new(chip: Arc<Box<InnerChip>>, offset: u32) -> Result<Line> {
        let mut line_info = ffi::gpioline_info {
            line_offset: offset,
            flags: 0,
            name: [0; 32],
            consumer: [0; 32],
        };
        let _ = unsafe { ffi::gpio_get_lineinfo_ioctl(chip.file.as_raw_fd(), &mut line_info)? };

        Ok(Line {
            chip: chip,
            offset: offset,
            flags: LineFlags::from_bits_truncate(line_info.flags),
            name: unsafe { cstrbuf_to_string(&line_info.name[..]) },
            consumer: unsafe { cstrbuf_to_string(&line_info.consumer[..]) },
        })
    }

    pub fn refresh(self) -> Result<Line> {
        Line::new(self.chip, self.offset)
    }

    pub fn offset(&self) -> u32 {
        self.offset
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|s| s.as_str())
    }

    pub fn consumer(&self) -> Option<&str> {
        self.consumer.as_ref().map(|s| s.as_str())
    }

    pub fn is_used(&self) -> bool {
        !self.flags.is_empty()
    }

    pub fn is_kernel(&self) -> bool {
        self.flags.contains(LineFlags::KERNEL)
    }

    pub fn is_active_low(&self) -> bool {
        self.flags.contains(LineFlags::ACTIVE_LOW)
    }

    pub fn is_open_drain(&self) -> bool {
        self.flags.contains(LineFlags::OPEN_DRAIN)
    }

    pub fn is_open_source(&self) -> bool {
        self.flags.contains(LineFlags::OPEN_SOURCE)
    }

    pub fn direction(&self) -> LineDirection {
        match self.flags.contains(LineFlags::IS_OUT) {
            true => LineDirection::Out,
            false => LineDirection::In,
        }
    }

    pub fn chip(&self) -> Chip {
        Chip {
            inner: self.chip.clone(),
        }
    }

    pub fn request(&self, flags: RequestFlags, default: u8, consumer: &str) -> Result<LineHandle> {
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
        request.default_values[0] = default;
        unsafe {
            rstr_lcpy(
                request.consumer_label[..].as_mut_ptr(),
                consumer,
                request.consumer_label.len(),
            )
        };
        unsafe { ffi::gpio_get_linehandle_ioctl(self.chip.file.as_raw_fd(), &mut request) }?;
        Ok(LineHandle {
            line: self.clone().refresh()?, // TODO: revisit
            flags: flags,
            file: unsafe { File::from_raw_fd(request.fd) },
        })
    }

    pub fn events(
        &self,
        handle_flags: RequestFlags,
        event_flags: EventRequestFlags,
        consumer: &str,
    ) -> Result<LineEventIterator> {
        let mut request = ffi::gpioevent_request {
            lineoffset: self.offset,
            handleflags: handle_flags.bits(),
            eventflags: event_flags.bits(),
            consumer_label: unsafe { mem::zeroed() },
            fd: 0,
        };

        unsafe {
            rstr_lcpy(
                request.consumer_label[..].as_mut_ptr(),
                consumer,
                request.consumer_label.len(),
            )
        };
        unsafe { ffi::gpio_get_lineevent_ioctl(self.chip.file.as_raw_fd(), &mut request) }?;

        Ok(LineEventIterator {
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

#[derive(Debug)]
pub enum EventType {
    RisingEdge,
    FallingEdge,
}

pub struct LineEvent(ffi::gpioevent_data);

impl std::fmt::Debug for LineEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "LineEvent {{ timestamp: {:?}, event_type: {:?} }}",
            self.timestamp(),
            self.event_type()
        )
    }
}

impl LineEvent {
    pub fn timestamp(&self) -> u64 {
        self.0.timestamp
    }

    pub fn event_type(&self) -> EventType {
        if self.0.id == 0x01 {
            EventType::RisingEdge
        } else {
            EventType::FallingEdge
        }
    }
}

#[derive(Debug)]
pub struct LineEventIterator {
    file: File,
}

impl Iterator for LineEventIterator {
    type Item = Result<LineEvent>;

    fn next(&mut self) -> Option<Result<LineEvent>> {
        let mut data: ffi::gpioevent_data = unsafe { mem::zeroed() };
        let mut data_as_buf = unsafe {
            slice::from_raw_parts_mut(
                &mut data as *mut ffi::gpioevent_data as *mut u8,
                mem::size_of::<ffi::gpioevent_data>(),
            )
        };
        match nix::unistd::read(self.file.as_raw_fd(), &mut data_as_buf) {
            Ok(bytes_read) => if bytes_read != mem::size_of::<ffi::gpioevent_data>() {
                None
            } else {
                Some(Ok(LineEvent(data)))
            },
            Err(e) => Some(Err(e.into())),
        }
    }
}

#[derive(Debug)]
pub struct LinePoll {
    line: Line,
    file: File,
}

impl LinePoll {}
