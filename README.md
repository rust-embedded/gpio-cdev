# rust-gpio-cdev

[![Build Status]](https://travis-ci.org/posborne/rust-gpio-cdev.svg?branch=master)](https://travis-ci.org/posborne/rust-gpio-cdev)
[![Version](https://img.shields.io/crates/v/gpio-cdev.svg)](https://crates.io/crates/gpio-cdev)
[![License](https://img.shields.io/crates/l/gpio-cdev.svg)](https://github.com/posborne/rust-gpio-cdev/blob/master/README.md#license)

- [API Documentation](https://docs.rs/gpio-cdev)

rust-gpio-cdev is a Rust library/crate providing access to [GPIO character device
ABI](https://www.kernel.org/doc/Documentation/ABI/testing/gpio-cdev).  This API,
stabilized with Linux v4.4, deprecates the legacy sysfs interface to GPIOs that
is now deprecated and is planned to be removed from the upstream kernel after
year 2020 (which is coming up quickly).

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
sum up the situation nicely when assignign the a base number to a GPIO chip
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
`GPIO_GET_LINEHANDLE_IOCTL`.

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
