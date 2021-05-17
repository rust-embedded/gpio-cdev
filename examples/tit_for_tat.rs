// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use gpio_cdev::{Chip, EventRequestFlags, EventType, LineRequestFlags};
use quicli::prelude::*;
use std::thread::sleep;
use std::time::Duration;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Cli {
    /// The gpiochip device (e.g. /dev/gpiochip0)
    chip: String,
    /// The offset of the GPIO input line for the provided chip
    inputline: u32,
    /// The offset of the GPIO output line for the provided chip
    outputline: u32,
    /// Sleep time after each actuation in milliseconds
    sleeptime: u64,
}

fn do_main(args: Cli) -> std::result::Result<(), gpio_cdev::Error> {
    let mut chip = Chip::new(args.chip)?;
    let input = chip.get_line(args.inputline)?;
    let output = chip.get_line(args.outputline)?;
    let output_handle = output.request(LineRequestFlags::OUTPUT, 0, "tit_for_tat")?;

    // To show off the buffering characteristics of the new interface we introduce a delay
    // after each change is handled.  When we fall behind, we will "replay" the input
    // events
    for event in input.events(
        LineRequestFlags::INPUT,
        EventRequestFlags::BOTH_EDGES,
        "tit_for_tat",
    )? {
        let evt = event?;
        println!("{:?}", evt);
        match evt.event_type() {
            EventType::RisingEdge => {
                output_handle.set_value(1)?;
                sleep(Duration::from_millis(args.sleeptime));
            }
            EventType::FallingEdge => {
                output_handle.set_value(0)?;
                sleep(Duration::from_millis(args.sleeptime));
            }
        }
    }

    Ok(())
}

fn main() -> CliResult {
    let args = Cli::from_args();
    do_main(args).or_else(|e| {
        error!("{:?}", e);
        Ok(())
    })
}
