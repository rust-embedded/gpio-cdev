// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! The `gpio-cdev` crate provides access to the [GPIO character device
//! ABI](https://www.kernel.org/doc/Documentation/ABI/testing/gpio-cdev).  This API,
//! stabilized with Linux v4.4, deprecates the legacy sysfs interface to GPIOs that is
//! planned to be removed from the upstream kernel after
//! year 2020 (which is coming up quickly).
//!
//! This crate attempts to wrap this interface in a moderately direction fashion
//! while retaining safety and using Rust idioms (where doing so could be mapped
//! to the underlying abstraction without significant overhead or loss of
//! functionality).
//!
//! For additional context for why the kernel is moving from the sysfs API to the
//! character device API, please see the main [README on Github].
//!
//! # Examples
//!
//! The following example reads the state of a GPIO line/pin and writes the matching
//! state to another line/pin.
//!
//! ```no_run
//! use gpio_cdev::{Chip, LineRequestFlags, EventRequestFlags, EventType};
//!
//! // Lines are offset within gpiochip0; see docs for more info on chips/lines
//! fn mirror_gpio(inputline: u32, outputline: u32) -> Result<(), gpio_cdev::Error> {
//!     let mut chip = Chip::new("/dev/gpiochip0")?;
//!     let input = chip.get_line(inputline)?;
//!     let output = chip.get_line(outputline)?;
//!     let output_handle = output.request(LineRequestFlags::OUTPUT, 0, "mirror-gpio")?;
//!     for event in input.events(
//!         LineRequestFlags::INPUT,
//!         EventRequestFlags::BOTH_EDGES,
//!         "mirror-gpio",
//!     )? {
//!         let evt = event?;
//!         println!("{:?}", evt);
//!         match evt.event_type() {
//!             EventType::RisingEdge => {
//!                 output_handle.set_value(1)?;
//!             }
//!             EventType::FallingEdge => {
//!                 output_handle.set_value(0)?;
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//!
//! # fn main() -> Result<(), gpio_cdev::Error> {
//! #     mirror_gpio(0, 1)
//! # }
//! ```
//!
//! To get the state of a GPIO Line on a given chip:
//!
//! ```no_run
//! use gpio_cdev::{Chip, LineRequestFlags};
//!
//! # fn main() -> Result<(), gpio_cdev::Error> {
//! // Read the state of GPIO4 on a raspberry pi.  /dev/gpiochip0
//! // maps to the driver for the SoC (builtin) GPIO controller.
//! // The LineHandle returned by request must be assigned to a
//! // variable (in this case the variable handle) to ensure that
//! // the corresponding file descriptor is not closed.
//! let mut chip = Chip::new("/dev/gpiochip0")?;
//! let handle = chip
//!     .get_line(4)?
//!     .request(LineRequestFlags::INPUT, 0, "read-input")?;
//! for _ in 1..4 {
//!     println!("Value: {:?}", handle.get_value()?);
//! }
//! # Ok(()) }
//! ```
//!
//! [README on Github]: https://github.com/rust-embedded/rust-gpio-cdev

#![cfg_attr(docsrs, feature(doc_cfg))]

#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate nix;

use std::cmp::min;
use std::ffi::CStr;
use std::fs::{read_dir, File, ReadDir};
use std::io::Read;
use std::mem;
use std::ops::Index;
use std::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::ptr;
use std::slice;
use std::sync::Arc;

#[cfg(feature = "async-tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "async-tokio")))]
mod async_tokio;
pub mod errors; // pub portion is deprecated
mod ffi;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IoctlKind {
    ChipInfo,
    LineInfo,
    LineHandle,
    LineEvent,
    GetLine,
    SetLine,
}

#[cfg(feature = "async-tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "async-tokio")))]
pub use crate::async_tokio::AsyncLineEventHandle;
pub use errors::*;

unsafe fn rstr_lcpy(dst: *mut libc::c_char, src: &str, length: usize) {
    let copylen = min(src.len() + 1, length);
    ptr::copy_nonoverlapping(src.as_bytes().as_ptr().cast(), dst, copylen - 1);
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

/// A GPIO Chip maps to the actual device driver instance in hardware that
/// one interacts with to interact with individual GPIOs.  Often these chips
/// map to IP chunks on an SoC but could also be enumerated within the kernel
/// via something like a PCI or USB bus.
///
/// The Linux kernel itself enumerates GPIO character devices at two paths:
/// 1. `/dev/gpiochipN`
/// 2. `/sys/bus/gpiochipN`
///
/// It is best not to assume that a device will always be enumerated in the
/// same order (especially if it is connected via a bus).  In order to reliably
/// find the correct chip, there are a few approaches that one could reasonably
/// take:
///
/// 1. Create a udev rule that will match attributes of the device and
///    setup a symlink to the device.
/// 2. Iterate over all available chips using the [`chips()`] call to find the
///    device with matching criteria.
/// 3. For simple cases, just using the enumerated path is fine (demo work).  This
///    is discouraged for production.
///
/// [`chips()`]: fn.chips.html
#[derive(Debug)]
pub struct Chip {
    inner: Arc<InnerChip>,
}

/// Iterator over chips
#[derive(Debug)]
pub struct ChipIterator {
    readdir: ReadDir,
}

impl Iterator for ChipIterator {
    type Item = Result<Chip>;

    fn next(&mut self) -> Option<Result<Chip>> {
        for entry in &mut self.readdir {
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

/// Iterate over all GPIO chips currently present on this system
pub fn chips() -> Result<ChipIterator> {
    Ok(ChipIterator {
        readdir: read_dir("/dev")?,
    })
}

impl Chip {
    /// Open the GPIO Chip at the provided path (e.g. `/dev/gpiochip<N>`)
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let f = File::open(path.as_ref())?;
        let mut info: ffi::gpiochip_info = unsafe { mem::zeroed() };
        ffi::gpio_get_chipinfo_ioctl(f.as_raw_fd(), &mut info)?;

        Ok(Self {
            inner: Arc::new(InnerChip {
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
            }),
        })
    }

    /// Get the fs path of this character device (e.g. `/dev/gpiochipN`)
    pub fn path(&self) -> &Path {
        self.inner.path.as_path()
    }

    /// The name of the device driving this GPIO chip in the kernel
    pub fn name(&self) -> &str {
        self.inner.name.as_str()
    }

    /// A functional name for this GPIO chip, such as a product number.  Might
    /// be an empty string.
    ///
    /// As an example, the SoC GPIO chip on a Raspberry Pi is "pinctrl-bcm2835"
    pub fn label(&self) -> &str {
        self.inner.label.as_str()
    }

    /// The number of lines/pins indexable through this chip
    ///
    /// Not all of these may be usable depending on how the hardware is
    /// configured/muxed.
    pub fn num_lines(&self) -> u32 {
        self.inner.lines
    }

    /// Get a handle to the GPIO line at a given offset
    ///
    /// The actual physical line corresponding to a given offset
    /// is completely dependent on how the driver/hardware for
    /// the chip works as well as the associated board layout.
    ///
    /// For a device like the NXP i.mx6 SoC GPIO controller there
    /// are several banks of GPIOs with each bank containing 32
    /// GPIOs.  For this hardware and driver something like
    /// `GPIO2_5` would map to offset 37.
    pub fn get_line(&mut self, offset: u32) -> Result<Line> {
        Line::new(self.inner.clone(), offset)
    }

    /// Get a handle to multiple GPIO line at a given offsets
    ///
    /// The group of lines can be manipulated simultaneously.
    pub fn get_lines(&mut self, offsets: &[u32]) -> Result<Lines> {
        Lines::new(self.inner.clone(), offsets)
    }

    /// Get a handle to all the GPIO lines on the chip
    ///
    /// The group of lines can be manipulated simultaneously.
    pub fn get_all_lines(&mut self) -> Result<Lines> {
        let offsets: Vec<u32> = (0..self.num_lines()).collect();
        self.get_lines(&offsets)
    }

    /// Get an interator over all lines that can be potentially access for this
    /// chip.
    pub fn lines(&self) -> LineIterator {
        LineIterator {
            chip: self.inner.clone(),
            idx: 0,
        }
    }
}

/// Iterator over GPIO Lines for a given chip.
#[derive(Debug)]
pub struct LineIterator {
    chip: Arc<InnerChip>,
    idx: u32,
}

impl Iterator for LineIterator {
    type Item = Line;

    fn next(&mut self) -> Option<Line> {
        if self.idx < self.chip.lines {
            let idx = self.idx;
            self.idx += 1;
            // Since we checked the index, we know this will be Ok
            Some(Line::new(self.chip.clone(), idx).unwrap())
        } else {
            None
        }
    }
}

/// Access to a specific GPIO Line
///
/// GPIO Lines must be obtained through a parent [`Chip`] and
/// represent an actual GPIO pin/line accessible via that chip.
/// Not all accessible lines for a given chip may actually
/// map to hardware depending on how the board is setup
/// in the kernel.
///
#[derive(Debug, Clone)]
pub struct Line {
    chip: Arc<InnerChip>,
    offset: u32,
}

/// Information about a specific GPIO Line
///
/// Wraps kernel [`struct gpioline_info`].
///
/// [`struct gpioline_info`]: https://elixir.bootlin.com/linux/v4.9.127/source/include/uapi/linux/gpio.h#L36
#[derive(Debug)]
pub struct LineInfo {
    line: Line,
    flags: LineFlags,
    name: Option<String>,
    consumer: Option<String>,
}

bitflags! {
    /// Line Request Flags
    ///
    /// Maps to kernel [`GPIOHANDLE_REQUEST_*`] flags.
    ///
    /// [`GPIOHANDLE_REQUEST_*`]: https://elixir.bootlin.com/linux/v4.9.127/source/include/uapi/linux/gpio.h#L58
    #[derive(Debug, Clone)]
    pub struct LineRequestFlags: u32 {
        const INPUT = (1 << 0);
        const OUTPUT = (1 << 1);
        const ACTIVE_LOW = (1 << 2);
        const OPEN_DRAIN = (1 << 3);
        const OPEN_SOURCE = (1 << 4);
        // https://elixir.bootlin.com/linux/v5.5/source/include/uapi/linux/gpio.h#L62
        /// Available since Linux Kernel 5.5
        const BIAS_PULL_UP = (1 << 5);
        /// Available since Linux Kernel 5.5
        const BIAS_PULL_DOWN = (1 << 6);
        /// Available since Linux Kernel 5.5
        const BIAS_PULL_DISABLE = (1 << 7);
    }
}

bitflags! {
    /// Event request flags
    ///
    /// Maps to kernel [`GPIOEVENT_REQEST_*`] flags.
    ///
    /// [`GPIOEVENT_REQUEST_*`]: https://elixir.bootlin.com/linux/v4.9.127/source/include/uapi/linux/gpio.h#L109
    pub struct EventRequestFlags: u32 {
        const RISING_EDGE = (1 << 0);
        const FALLING_EDGE = (1 << 1);
        const BOTH_EDGES = Self::RISING_EDGE.bits() | Self::FALLING_EDGE.bits();
    }
}

bitflags! {
    /// Informational Flags
    ///
    /// Maps to kernel [`GPIOLINE_FLAG_*`] flags.
    ///
    /// [`GPIOLINE_FLAG_*`]: https://elixir.bootlin.com/linux/v4.9.127/source/include/uapi/linux/gpio.h#L29
    #[derive(Debug)]
    pub struct LineFlags: u32 {
        const KERNEL = (1 << 0);
        const IS_OUT = (1 << 1);
        const ACTIVE_LOW = (1 << 2);
        const OPEN_DRAIN = (1 << 3);
        const OPEN_SOURCE = (1 << 4);
        // https://elixir.bootlin.com/linux/v5.5/source/include/uapi/linux/gpio.h#L30
        /// Available since Linux Kernel 5.5
        const BIAS_PULL_UP = (1 << 5);
        /// Available since Linux Kernel 5.5
        const BIAS_PULL_DOWN = (1 << 6);
        /// Available since Linux Kernel 5.5
        const BIAS_DISABLE = (1 << 7);
    }
}

/// In or Out
#[derive(Debug, Clone, Copy, PartialEq)]
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
    fn new(chip: Arc<InnerChip>, offset: u32) -> Result<Self> {
        if offset >= chip.lines {
            return Err(offset_err(offset));
        }
        Ok(Self { chip, offset })
    }

    /// Get info about the line from the kernel.
    pub fn info(&self) -> Result<LineInfo> {
        let mut line_info = ffi::gpioline_info {
            line_offset: self.offset,
            flags: 0,
            name: [0; 32],
            consumer: [0; 32],
        };
        ffi::gpio_get_lineinfo_ioctl(self.chip.file.as_raw_fd(), &mut line_info)?;

        Ok(LineInfo {
            line: self.clone(),
            flags: LineFlags::from_bits_truncate(line_info.flags),
            name: unsafe { cstrbuf_to_string(&line_info.name[..]) },
            consumer: unsafe { cstrbuf_to_string(&line_info.consumer[..]) },
        })
    }

    /// Offset of this line within its parent chip
    pub fn offset(&self) -> u32 {
        self.offset
    }

    /// Get a handle to this chip's parent
    pub fn chip(&self) -> Chip {
        Chip {
            inner: self.chip.clone(),
        }
    }

    /// Request access to interact with this line from the kernel
    ///
    /// This is similar to the "export" operation present in the sysfs
    /// API with the key difference that we are also able to configure
    /// the GPIO with `flags` to specify how the line will be used
    /// at the time of request.
    ///
    /// For an output, the `default` parameter specifies the value
    /// the line should have when it is configured as an output.  The
    /// `consumer` string should describe the process consuming the
    /// line (this will be truncated to 31 characters if too long).
    ///
    /// # Errors
    ///
    /// The main source of errors here is if the kernel returns an
    /// error to the ioctl performing the request here.  This will
    /// result in an [`Error`] being returned with [`ErrorKind::Ioctl`].
    ///
    /// One possible cause for an error here would be if the line is
    /// already in use.  One can check for this prior to making the
    /// request using [`is_kernel`].
    ///
    /// [`Error`]: errors/struct.Error.html
    /// [`ErrorKind::Ioctl`]: errors/enum.ErrorKind.html#variant.Ioctl
    /// [`is_kernel`]: struct.Line.html#method.is_kernel
    pub fn request(
        &self,
        flags: LineRequestFlags,
        default: u8,
        consumer: &str,
    ) -> Result<LineHandle> {
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
            );
        }
        ffi::gpio_get_linehandle_ioctl(self.chip.file.as_raw_fd(), &mut request)?;
        Ok(LineHandle {
            line: self.clone(),
            flags,
            file: unsafe { File::from_raw_fd(request.fd) },
        })
    }

    /// Get an event handle that can be used as a blocking iterator over
    /// the events (state changes) for this Line
    ///
    /// When used as an iterator, it blocks while there is not another event
    /// available from the kernel for this line matching the subscription
    /// criteria specified in the `event_flags`.  The line will be configured
    /// with the specified `handle_flags` and `consumer` label.
    ///
    /// Note that as compared with the sysfs interface, the character
    /// device interface maintains a queue of events in the kernel so
    /// events may happen (e.g. a line changing state faster than can
    /// be picked up in userspace in real-time).  These events will be
    /// returned on the iterator in order with the event containing the
    /// associated timestamp attached with high precision within the
    /// kernel (from an ISR for most drivers).
    ///
    /// # Example
    ///
    /// ```no_run
    /// # fn main() -> Result<(), gpio_cdev::Error> {
    /// use gpio_cdev::{Chip, LineRequestFlags, EventRequestFlags};
    /// use std::io;
    ///
    /// let mut chip = Chip::new("/dev/gpiochip0")?;
    /// let input = chip.get_line(0)?;
    ///
    /// // Show all state changes for this line forever
    /// for event in input.events(
    ///     LineRequestFlags::INPUT,
    ///     EventRequestFlags::BOTH_EDGES,
    ///     "rust-gpio"
    /// )? {
    ///     println!("{:?}", event?);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn events(
        &self,
        handle_flags: LineRequestFlags,
        event_flags: EventRequestFlags,
        consumer: &str,
    ) -> Result<LineEventHandle> {
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
            );
        }
        ffi::gpio_get_lineevent_ioctl(self.chip.file.as_raw_fd(), &mut request)?;

        Ok(LineEventHandle {
            line: self.clone(),
            file: unsafe { File::from_raw_fd(request.fd) },
        })
    }

    #[cfg(feature = "async-tokio")]
    #[cfg_attr(docsrs, doc(cfg(feature = "async-tokio")))]
    pub fn async_events(
        &self,
        handle_flags: LineRequestFlags,
        event_flags: EventRequestFlags,
        consumer: &str,
    ) -> Result<AsyncLineEventHandle> {
        let events = self.events(handle_flags, event_flags, consumer)?;
        AsyncLineEventHandle::new(events)
    }
}

impl LineInfo {
    /// Get a handle to the line that this info represents
    pub fn line(&self) -> &Line {
        &self.line
    }

    /// Name assigned to this chip if assigned
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The name of this GPIO line, such as the output pin of the line on the
    /// chip, a rail or a pin header name on a board, as specified by the gpio
    /// chip.
    pub fn consumer(&self) -> Option<&str> {
        self.consumer.as_deref()
    }

    /// Get the direction of this GPIO if configured
    ///
    /// Lines are considered to be inputs if not explicitly
    /// marked as outputs in the line info flags by the kernel.
    pub fn direction(&self) -> LineDirection {
        if self.flags.contains(LineFlags::IS_OUT) {
            LineDirection::Out
        } else {
            LineDirection::In
        }
    }

    /// True if the any flags for the device are set (input or output)
    pub fn is_used(&self) -> bool {
        !self.flags.is_empty()
    }

    /// True if this line is being used by something else in the kernel
    ///
    /// If another driver or subsystem in the kernel is using the line
    /// then it cannot be used via the cdev interface. See [relevant kernel code].
    ///
    /// [relevant kernel code]: https://elixir.bootlin.com/linux/v4.9.127/source/drivers/gpio/gpiolib.c#L938
    pub fn is_kernel(&self) -> bool {
        self.flags.contains(LineFlags::KERNEL)
    }

    /// True if this line is marked as active low in the kernel
    pub fn is_active_low(&self) -> bool {
        self.flags.contains(LineFlags::ACTIVE_LOW)
    }

    /// True if this line is marked as open drain in the kernel
    pub fn is_open_drain(&self) -> bool {
        self.flags.contains(LineFlags::OPEN_DRAIN)
    }

    /// True if this line is marked as open source in the kernel
    pub fn is_open_source(&self) -> bool {
        self.flags.contains(LineFlags::OPEN_SOURCE)
    }
}

/// Handle for interacting with a "requested" line
///
/// In order for userspace to read/write the value of a GPIO
/// it must be requested from the chip using [`Line::request`].
/// On success, the kernel creates an anonymous file descriptor
/// for interacting with the requested line.  This structure
/// is the go-between for callers and that file descriptor.
///
/// [`Line::request`]: struct.Line.html#method.request
#[derive(Debug)]
pub struct LineHandle {
    line: Line,
    flags: LineRequestFlags,
    file: File,
}

impl LineHandle {
    /// Request the current state of this Line from the kernel
    ///
    /// This call is expected to succeed for both input and output
    /// lines.  It should be noted, however, that some drivers may
    /// not be able to give any useful information when the value
    /// is requested for an output line.
    ///
    /// This value should be 0 or 1 which a "1" representing that
    /// the line is active.  Usually this means that the line is
    /// at logic-level high but it could mean the opposite if the
    /// line has been marked as being `ACTIVE_LOW`.
    pub fn get_value(&self) -> Result<u8> {
        let mut data: ffi::gpiohandle_data = unsafe { mem::zeroed() };
        ffi::gpiohandle_get_line_values_ioctl(self.file.as_raw_fd(), &mut data)?;
        Ok(data.values[0])
    }

    /// Request that the line be driven to the specified value
    ///
    /// The value should be 0 or 1 with 1 representing a request
    /// to make the line "active".  Usually "active" means
    /// logic level high unless the line has been marked as `ACTIVE_LOW`.
    ///
    /// Calling `set_value` on a line that is not an output will
    /// likely result in an error (from the kernel).
    pub fn set_value(&self, value: u8) -> Result<()> {
        let mut data: ffi::gpiohandle_data = unsafe { mem::zeroed() };
        data.values[0] = value;
        ffi::gpiohandle_set_line_values_ioctl(self.file.as_raw_fd(), &mut data)?;
        Ok(())
    }

    /// Get the Line information associated with this handle.
    pub fn line(&self) -> &Line {
        &self.line
    }

    /// Get the flags with which this handle was created
    pub fn flags(&self) -> LineRequestFlags {
        self.flags.clone()
    }
}

impl AsRawFd for LineHandle {
    /// Gets the raw file descriptor for the `LineHandle`.
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

/// A collection of lines that can be accesses simultaneously
///
/// This is a collection of lines, all from the same GPIO chip that can
/// all be accessed simultaneously
#[derive(Debug)]
pub struct Lines {
    lines: Vec<Line>,
}

impl Lines {
    fn new(chip: Arc<InnerChip>, offsets: &[u32]) -> Result<Self> {
        let res: Result<Vec<Line>> = offsets
            .iter()
            .map(|off| Line::new(chip.clone(), *off))
            .collect();
        let lines = res?;
        Ok(Self { lines })
    }

    /// Get a handle to the parent chip for the lines
    pub fn chip(&self) -> Chip {
        self.lines[0].chip()
    }

    /// Get the number of lines in the collection
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Get the number of lines in the collection
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Request access to interact with these lines from the kernel
    ///
    /// This is similar to the "export" operation present in the sysfs
    /// API with the key difference that we are also able to configure
    /// the GPIO with `flags` to specify how the line will be used
    /// at the time of request.
    ///
    /// For an output, the `default` parameter specifies the value
    /// each line should have when it is configured as an output.  The
    /// `consumer` string should describe the process consuming the
    /// line (this will be truncated to 31 characters if too long).
    ///
    /// # Errors
    ///
    /// The main source of errors here is if the kernel returns an
    /// error to the ioctl performing the request here.  This will
    /// result in an [`Error`] being returned with [`ErrorKind::Ioctl`].
    ///
    /// One possible cause for an error here would be if the lines are
    /// already in use.  One can check for this prior to making the
    /// request using [`is_kernel`].
    ///
    /// [`Error`]: errors/struct.Error.html
    /// [`ErrorKind::Ioctl`]: errors/enum.ErrorKind.html#variant.Ioctl
    /// [`is_kernel`]: struct.Line.html#method.is_kernel
    pub fn request(
        &self,
        flags: LineRequestFlags,
        default: &[u8],
        consumer: &str,
    ) -> Result<MultiLineHandle> {
        let n = self.lines.len();
        if default.len() != n {
            return Err(invalid_err(n, default.len()));
        }
        // prepare the request; the kernel consumes some of these values and will
        // set the fd for us.
        let mut request = ffi::gpiohandle_request {
            lineoffsets: unsafe { mem::zeroed() },
            flags: flags.bits(),
            default_values: unsafe { mem::zeroed() },
            consumer_label: unsafe { mem::zeroed() },
            lines: n as u32,
            fd: 0,
        };
        #[allow(clippy::needless_range_loop)] // clippy does not understand this loop correctly
        for i in 0..n {
            request.lineoffsets[i] = self.lines[i].offset();
            request.default_values[i] = default[i];
        }
        unsafe {
            rstr_lcpy(
                request.consumer_label[..].as_mut_ptr(),
                consumer,
                request.consumer_label.len(),
            );
        }
        ffi::gpio_get_linehandle_ioctl(self.lines[0].chip().inner.file.as_raw_fd(), &mut request)?;
        let lines = self.lines.clone();
        Ok(MultiLineHandle {
            lines: Self { lines },
            file: unsafe { File::from_raw_fd(request.fd) },
        })
    }
}

impl Index<usize> for Lines {
    type Output = Line;

    fn index(&self, i: usize) -> &Line {
        &self.lines[i]
    }
}

/// Handle for interacting with a "requested" line
///
/// In order for userspace to read/write the value of a GPIO
/// it must be requested from the chip using [`Line::request`].
/// On success, the kernel creates an anonymous file descriptor
/// for interacting with the requested line.  This structure
/// is the go-between for callers and that file descriptor.
///
/// [`Line::request`]: struct.Line.html#method.request
#[derive(Debug)]
pub struct MultiLineHandle {
    lines: Lines,
    file: File,
}

impl MultiLineHandle {
    /// Request the current state of this Line from the kernel
    ///
    /// This call is expected to succeed for both input and output
    /// lines.  It should be noted, however, that some drivers may
    /// not be able to give any useful information when the value
    /// is requested for an output line.
    ///
    /// This value should be 0 or 1 which a "1" representing that
    /// the line is active.  Usually this means that the line is
    /// at logic-level high but it could mean the opposite if the
    /// line has been marked as being `ACTIVE_LOW`.
    pub fn get_values(&self) -> Result<Vec<u8>> {
        let mut data: ffi::gpiohandle_data = unsafe { mem::zeroed() };
        ffi::gpiohandle_get_line_values_ioctl(self.file.as_raw_fd(), &mut data)?;
        let n = self.num_lines();
        let values: Vec<u8> = (0..n).map(|i| data.values[i]).collect();
        Ok(values)
    }

    /// Request that the line be driven to the specified value
    ///
    /// The value should be 0 or 1 with 1 representing a request
    /// to make the line "active".  Usually "active" means
    /// logic level high unless the line has been marked as `ACTIVE_LOW`.
    ///
    /// Calling `set_value` on a line that is not an output will
    /// likely result in an error (from the kernel).
    pub fn set_values(&self, values: &[u8]) -> Result<()> {
        let n = self.num_lines();
        if values.len() != n {
            return Err(invalid_err(n, values.len()));
        }
        let mut data: ffi::gpiohandle_data = unsafe { mem::zeroed() };
        data.values[..n].clone_from_slice(&values[..n]);
        ffi::gpiohandle_set_line_values_ioctl(self.file.as_raw_fd(), &mut data)?;
        Ok(())
    }

    /// Get the number of lines associated with this handle
    pub fn num_lines(&self) -> usize {
        self.lines.len()
    }

    /// Get the Line information associated with this handle.
    pub fn lines(&self) -> &Lines {
        &self.lines
    }
}

impl AsRawFd for MultiLineHandle {
    /// Gets the raw file descriptor for the `LineHandle`.
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

/// Did the Line rise (go active) or fall (go inactive)?
///
/// Maps to kernel [`GPIOEVENT_EVENT_*`] definitions.
///
/// [`GPIOEVENT_EVENT_*`]: https://elixir.bootlin.com/linux/v4.9.127/source/include/uapi/linux/gpio.h#L136
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventType {
    RisingEdge,
    FallingEdge,
}

/// Information about a change to the state of a Line
///
/// Wraps kernel [`struct gpioevent_data`].
///
/// [`struct gpioevent_data`]: https://elixir.bootlin.com/linux/v4.9.127/source/include/uapi/linux/gpio.h#L142
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
    /// Best estimate of event occurrence time, in nanoseconds
    ///
    /// In most cases, the timestamp for the event is captured
    /// in an interrupt handler so it should be very accurate.
    ///
    /// The nanosecond timestamp value should are captured
    /// using the `CLOCK_MONOTONIC` offsets in the kernel and
    /// should be compared against `CLOCK_MONOTONIC` values.
    /// Note that kernel versions prior to 5.7 used
    /// `CLOCK_REALTIME` offsets instead.
    pub fn timestamp(&self) -> u64 {
        self.0.timestamp
    }

    /// Was this a rising or a falling edge?
    pub fn event_type(&self) -> EventType {
        if self.0.id == 0x01 {
            EventType::RisingEdge
        } else {
            EventType::FallingEdge
        }
    }
}

/// Handle for retrieving events from the kernel for a line
///
/// In order for userspace to retrieve incoming events on a GPIO,
/// an event handle must be requested from the chip using
/// [`Line::events`].
/// On success, the kernel creates an anonymous file descriptor
/// for reading events. This structure is the go-between for callers
/// and that file descriptor.
///
/// [`Line::events`]: struct.Line.html#method.events
#[derive(Debug)]
pub struct LineEventHandle {
    line: Line,
    file: File,
}

impl LineEventHandle {
    /// Retrieve the next event from the kernel for this line
    ///
    /// This blocks while there is not another event available from the
    /// kernel for the line which matches the subscription criteria
    /// specified in the `event_flags` when the handle was created.
    pub fn get_event(&mut self) -> Result<LineEvent> {
        match self.read_event() {
            Ok(Some(event)) => Ok(event),
            Ok(None) => Err(event_err(nix::errno::Errno::EIO)),
            Err(e) => Err(e.into()),
        }
    }

    /// Request the current state of this Line from the kernel
    ///
    /// This value should be 0 or 1 which a "1" representing that
    /// the line is active.  Usually this means that the line is
    /// at logic-level high but it could mean the opposite if the
    /// line has been marked as being `ACTIVE_LOW`.
    pub fn get_value(&self) -> Result<u8> {
        let mut data: ffi::gpiohandle_data = unsafe { mem::zeroed() };
        ffi::gpiohandle_get_line_values_ioctl(self.file.as_raw_fd(), &mut data)?;
        Ok(data.values[0])
    }

    /// Get the Line information associated with this handle.
    pub fn line(&self) -> &Line {
        &self.line
    }

    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn file2(&mut self) -> &File {
        &self.file
    }

    /// Helper function which returns the line event if a complete event was read, Ok(None) if not
    /// enough data was read or the error returned by `read()`.
    pub(crate) fn read_event(&mut self) -> std::io::Result<Option<LineEvent>> {
        let mut data: ffi::gpioevent_data = unsafe { mem::zeroed() };
        let data_as_buf = unsafe {
            slice::from_raw_parts_mut(
                (&mut data as *mut ffi::gpioevent_data).cast(),
                mem::size_of::<ffi::gpioevent_data>(),
            )
        };
        let bytes_read = self.file.read(data_as_buf)?;
        if bytes_read == mem::size_of::<ffi::gpioevent_data>() {
            Ok(Some(LineEvent(data)))
        } else {
            Ok(None)
        }
    }
}

impl AsRawFd for LineEventHandle {
    /// Gets the raw file descriptor for the `LineEventHandle`.
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

impl AsFd for LineEventHandle {
    /// Gets the raw file descriptor for the `LineEventHandle`.
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.file.as_fd()
    }
}

impl Iterator for LineEventHandle {
    type Item = Result<LineEvent>;

    fn next(&mut self) -> Option<Result<LineEvent>> {
        match self.read_event() {
            Ok(None) => None,
            Ok(Some(event)) => Some(Ok(event)),
            Err(e) => Some(Err(e.into())),
        }
    }
}
