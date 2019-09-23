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
    pub lines: u32,
}

#[repr(C)]
pub struct gpioline_info {
    pub line_offset: u32,
    pub flags: u32,
    pub name: [libc::c_char; 32],
    pub consumer: [libc::c_char; 32],
}

#[repr(C)]
pub struct gpiohandle_request {
    pub lineoffsets: [u32; GPIOHANDLES_MAX],
    pub flags: u32,
    pub default_values: [u8; GPIOHANDLES_MAX],
    pub consumer_label: [libc::c_char; 32],
    pub lines: u32,
    pub fd: libc::c_int,
}

#[repr(C)]
pub struct gpiohandle_data {
    pub values: [u8; GPIOHANDLES_MAX],
}

#[repr(C)]
pub struct gpioevent_request {
    pub lineoffset: u32,
    pub handleflags: u32,
    pub eventflags: u32,
    pub consumer_label: [libc::c_char; 32],
    pub fd: libc::c_int,
}

#[repr(C)]
pub struct gpioevent_data {
    pub timestamp: u64,
    pub id: u32,
}

ioctl_read!(gpio_get_chipinfo_ioctl, 0xB4, 0x01, gpiochip_info);
ioctl_readwrite!(gpio_get_lineinfo_ioctl, 0xB4, 0x02, gpioline_info);
ioctl_readwrite!(gpio_get_linehandle_ioctl, 0xB4, 0x03, gpiohandle_request);
ioctl_readwrite!(gpio_get_lineevent_ioctl, 0xB4, 0x04, gpioevent_request);

ioctl_readwrite!(gpiohandle_get_line_values_ioctl, 0xB4, 0x08, gpiohandle_data);
ioctl_readwrite!(gpiohandle_set_line_values_ioctl, 0xB4, 0x09, gpiohandle_data);
