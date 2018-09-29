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

#[repr(C)]
pub struct gpioevent_request {
    pub lineoffset: libc::uint32_t,
    pub handleflags: libc::uint32_t,
    pub eventflags: libc::uint32_t,
    pub consumer_label: [libc::c_char; 32],
    pub fd: libc::c_int,
}

#[repr(C)]
pub struct gpioevent_data {
    pub timestamp: libc::uint64_t,
    pub id: libc::uint32_t,
}

ioctl_read!(gpio_get_chipinfo_ioctl, 0xB4, 0x01, gpiochip_info);
ioctl_readwrite!(gpio_get_lineinfo_ioctl, 0xB4, 0x02, gpioline_info);
ioctl_readwrite!(gpio_get_linehandle_ioctl, 0xB4, 0x03, gpiohandle_request);
ioctl_readwrite!(gpio_get_lineevent_ioctl, 0xB4, 0x04, gpioevent_request);

ioctl_readwrite!(gpiohandle_get_line_values_ioctl, 0xB4, 0x08, gpiohandle_data);
ioctl_readwrite!(gpiohandle_set_line_values_ioctl, 0xB4, 0x09, gpiohandle_data);
