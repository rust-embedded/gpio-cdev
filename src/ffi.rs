// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use libc;

pub const GPIOHANDLES_MAX: usize = 64;

// struct gpiochip_info
#[repr(C)]
pub struct gpiochip_info {
    pub name: [libc::c_char; 32],
    pub label: [libc::c_char; 32],
    pub lines: libc::uint32_t,
}

#[repr(C)]
pub struct gpioline_info {
    pub line_offset: libc::uint32_t,
    pub flags: libc::uint32_t,
    pub name: [libc::c_char; 32],
    pub consumer: [libc::c_char; 32],
}

#[repr(C)]
pub struct gpiohandle_request {
    pub lineoffsets: [libc::uint32_t; GPIOHANDLES_MAX],
    pub flags: libc::uint32_t,
    pub default_values: [libc::uint8_t; GPIOHANDLES_MAX],
    pub consumer_label: [libc::c_char; 32],
    pub lines: libc::uint32_t,
    pub fd: libc::c_int,
}

#[repr(C)]
pub struct gpiohandle_data {
    pub values: [libc::uint8_t; GPIOHANDLES_MAX],
}

bitflags! {
    pub struct GpioEventRequestFlags: libc::uint32_t {
        const RISING_EDGE = (1 << 0);
        const FALLING_EDGE = (1 << 1);
        const BOTH_EDGES = Self::RISING_EDGE.bits | Self::FALLING_EDGE.bits;
    }
}

#[repr(C)]
pub struct gpioevent_request {
    lineoffset: libc::uint32_t,
    handleflags: libc::uint32_t,
    eventflags: libc::uint32_t,
    consumer_label: [libc::c_char; 32],
    fd: libc::c_int,
}

#[repr(C)]
pub enum GpioEventType {
    RisingEdge = 0x01,
    FallingEdge = 0x02,
}

#[repr(C)]
pub struct gpioevent_data {
    timestamp: libc::uint64_t,
    id: libc::uint32_t,
}

ioctl!(read gpio_get_chipinfo_ioctl with 0xB4, 0x01; gpiochip_info);
ioctl!(readwrite gpio_get_lineinfo_ioctl with 0xB4, 0x02; gpioline_info);
ioctl!(readwrite gpio_get_linehandle_ioctl with 0xB4, 0x03; gpiohandle_request);
ioctl!(readwrite gpio_get_lineevent_ioctl with 0xB4, 0x04; gpioevent_request);

ioctl!(readwrite gpiohandle_get_line_values_ioctl with 0xB4, 0x08; gpiohandle_data);
ioctl!(readwrite gpiohandle_set_line_values_ioctl with 0xB4, 0x09; gpiohandle_data);
