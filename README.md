# gpio-cdev

[![Build Status](https://github.com/rust-embedded/gpio-cdev/workflows/Build/badge.svg)](https://github.com/rust-embedded/gpio-cdev/actions)
[![Version](https://img.shields.io/crates/v/gpio-cdev.svg)](https://crates.io/crates/gpio-cdev)
[![License](https://img.shields.io/crates/l/gpio-cdev.svg)](https://github.com/rust-embedded/gpio-cdev/blob/master/README.md#license)

- [API Documentation](https://docs.rs/gpio-cdev)

rust-gpio-cdev is a Rust library/crate providing access to [GPIO character device
ABI](https://www.kernel.org/doc/Documentation/ABI/testing/gpio-cdev).  This API,
stabilized with Linux v4.4, deprecates the legacy sysfs interface to GPIOs that is
planned to be removed from the upstream kernel after
year 2020 (which is coming up quickly).

Use of this API is encouraged over the sysfs API used by this crate's
predecessor [sysfs_gpio](https://crates.io/crates/sysfs_gpio) if you don't need
to target older kernels.  For more information on differences see [Sysfs GPIO vs
GPIO Character Device](#sysfs-gpio-vs-gpio-character-device).

## Installation

Add the following to your `Cargo.toml`

```
[dependencies]
gpio-cdev = "0.4"
```

Note that the following features are available:

* `async-tokio`: Adds a Stream interface for consuming GPIO events in async code
  within a tokio runtime.

## Examples

There are several additional examples available in the [examples
directory](https://github.com/rust-embedded/rust-gpio-cdev/tree/master/examples).

### Read State

```rust
use gpio_cdev::{Chip, LineRequestFlags};

// Read the state of GPIO4 on a raspberry pi.  /dev/gpiochip0
// maps to the driver for the SoC (builtin) GPIO controller.
let mut chip = Chip::new("/dev/gpiochip0")?;
let handle = chip
    .get_line(4)?
    .request(LineRequestFlags::INPUT, 0, "read-input")?;
for _ in 1..4 {
    println!("Value: {:?}", handle.get_value()?);
}
```

### Mirror State (Read/Write)

```rust
use gpio_cdev::{Chip, LineRequestFlags, EventRequestFlags, EventType};

// Lines are offset within gpiochip0; see docs for more info on chips/lines
//
// This function will synchronously follow the state of one line
// on gpiochip0 and mirror its state on another line.  With this you
// could, for instance, control the state of an LED with a button
// if hooked up to the right pins on a raspberry pi.
fn mirror_gpio(inputline: u32, outputline: u32) -> Result<(), gpio_cdev::Error> {
    let mut chip = Chip::new("/dev/gpiochip0")?;
    let input = chip.get_line(inputline)?;
    let output = chip.get_line(outputline)?;
    let output_handle = output.request(LineRequestFlags::OUTPUT, 0, "mirror-gpio")?;
    for event in input.events(
        LineRequestFlags::INPUT,
        EventRequestFlags::BOTH_EDGES,
        "mirror-gpio",
    )? {
        let evt = event?;
        println!("{:?}", evt);
        match evt.event_type() {
            EventType::RisingEdge => {
                output_handle.set_value(1)?;
            }
            EventType::FallingEdge => {
                output_handle.set_value(0)?;
            }
        }
    }

    Ok(())
}
```

### Async Usage

Note that this requires the addition of the `async-tokio` feature.

```rust
use futures::stream::StreamExt;
use gpio_cdev::{Chip, AsyncLineEventHandle};

async fn gpiomon(chip: String, line: u32) -> gpio_cdev::Result<()> {
    let mut chip = Chip::new(args.chip)?;
    let line = chip.get_line(args.line)?;
    let mut events = AsyncLineEventHandle::new(line.events(
        LineRequestFlags::INPUT,
        EventRequestFlags::BOTH_EDGES,
        "gpioevents",
    )?)?;

    while let Some(event) = events.next().await {
        let event = event?;
        println!("GPIO Event: {:?}", event);
    }

    Ok(())
}
```

## Sysfs GPIO vs GPIO Character Device

Compared to the sysfs gpio interface (as made available by the sysfs_gpio crate)
the character device has several advantages and critical design differences
(some of which are driving the deprecation in the kernel).

Since many people are familiar with the sysfs interface (which is easily
accessible via basic commands in the shell) and few people are familiar with the
GPIO character device, an exploration of the two and key differences here may
prove useful.

### Getting Access to a GPIO

In the Linux kernel, individual GPIOs are exposed via drivers that on probe register
themselves as GPIO chips with the gpio subsystem.  Each of these chips provides
access to a set of GPIOs.  At present, when this chip is registered a global
base number is assigned to this driver.  The comments from the linux kernel
[`gpio_chip_add_data`](https://elixir.bootlin.com/linux/v4.9.85/source/drivers/gpio/gpiolib.c#L1087)
sum up the situation nicely when assigning the a base number to a GPIO chip
on registration.

    /*
     * TODO: this allocates a Linux GPIO number base in the global
     * GPIO numberspace for this chip. In the long run we want to
     * get *rid* of this numberspace and use only descriptors, but
     * it may be a pipe dream. It will not happen before we get rid
     * of the sysfs interface anyways.
     */

The entire sysfs interface to GPIO is based around offsets from the base number
assigned to a GPIO chip.  The base number is completely dependent on the order
in which the chip was registered with the subsystem and the number of GPIOs that
each of the previous chips registered.  The only reason this is usable at all is
that most GPIOs are accessed via SoC hardware that is registered consistently
during boot.  It's not great; in fact, it's not even good.

The GPIO character device ABI provides access to GPIOs owned by a GPIO chip via
a bus device, `/sys/bus/gpiochipN` (or `/dev/gpiochipN`).  Within a chip, the
programmer will still need to know some details about how to access the GPIO but
things are generally sane.  Figuring out which bus device is the desired GPIO
chip can be done by iterating over all that are present and/or setting up
appropriate udev rules.  One good example of this is the [`lsgpio` utility in
the kernel source](https://github.com/torvalds/linux/blob/master/tools/gpio/lsgpio.c).

In sysfs each GPIO within a chip would be exported and used individually. The
GPIO character device allows for one or more GPIOs (referenced via offsets) to
be read, written, configured, and monitored via a "linehandle" fd that is
created dynamically on request.

### "Exporting" a GPIO

Using the sysfs API, one would write the global GPIO number to the "export" file
to perform further operations using new files on the filesystem.  Using the
gpiochip character device, a handle for performing operations on one or more
GPIO offsets within a chip are available via a "linehandle" fd created using the
`GPIO_GET_LINEHANDLE_IOCTL`. A consequence of this is that a line will remember
its state only for as long as the fd is open; the line's state will be reset
once the fd is closed.

When a linehandle is requested, additional information is also included about
how the individual GPIOs will be used (input, output, as-is, active-low, open
drain, open source, etc).  Multiple lines can be grouped together in a single
request but they must all be configured the same way if being used in that way.
See `struct gpioevent_request`.

### Reading/Writing GPIOs

Via sysfs, GPIOs could be read/written using the value file.  For GPIO character
devices, the `GPIOHANDLE_GET_LINE_VALUES_IOCTL` and
`GPIOHANDLE_SET_LINE_VALUES_IOCTL` may be used to get/set the state of one or
more offsets within the chip.

### Input Events

Via sysfs, one could setup things up using the trigger file to notify userspace
(by polling on the value file) of a single event based on how things were setup.
With GPIO character devices, one can setup a `gpio_eventrequest` that will create
a new anonymous file (fd provided) for event notifications on a lines within a
gpiochip.  Contrary to sysfs gpio events, the event file will queue multiple events
and include with the event (best effort) nanosecond-precision timing and an
identifier with event type.

With this information one could more reasonably consider interpreting a basic
digital signal from userspace (with rising and falling edges) from userspace
using the queueing with timing information captured in the kernel.  Previously, one
would need to quickly handle the event notification, make another system call
to the value file to see the state, etc. which had far too many variables involved
to be considered reliable.

## Minimum Supported Rust Version (MSRV)

This crate is guaranteed to compile on stable Rust 1.46.0 and up. It *might*
compile with older versions but that may change in any new patch release.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

## Code of Conduct

Contribution to this crate is organized under the terms of the [Rust Code of
Conduct][CoC], the maintainer of this crate, the [Embedded Linux Team][team], promises
to intervene to uphold that code of conduct.

[CoC]: CODE_OF_CONDUCT.md
[team]: https://github.com/rust-embedded/wg#the-embedded-linux-team
