// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use gpio_cdev::{Chip, LineRequestFlags};
use quicli::prelude::*;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Cli {
    /// The gpiochip device (e.g. /dev/gpiochip0)
    chip: String,
    /// The offset of the GPIO line for the provided chip
    line: u32,
    /// The value to write
    value: u8,
}

fn do_main(args: Cli) -> std::result::Result<(), gpio_cdev::Error> {
    let mut chip = Chip::new(args.chip)?;

    // NOTE: we set the default value to the desired state so
    // setting it separately is not required. The LineHandle
    // instance that is returned by request must be owned by a
    // variable for the duration of the time that the line will
    // be used. If the instance is not assigned to a variable,
    // then the LineHandle will be immediately dropped after
    // request returns and the pin will appear to do nothing.
    let _handle =
        chip.get_line(args.line)?
            .request(LineRequestFlags::OUTPUT, args.value, "driveoutput")?;

    println!("Output being driven... Enter to exit");
    let mut buf = String::new();
    ::std::io::stdin().read_line(&mut buf)?;

    Ok(())
}

fn main() -> CliResult {
    let args = Cli::from_args();
    do_main(args).or_else(|e| {
        error!("{:?}", e);
        Ok(())
    })
}
