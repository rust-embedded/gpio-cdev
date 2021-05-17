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
    /// The offset and value of each GPIO line for the provided chip
    /// in the form "off=<0|1>"
    line_values: Vec<String>,
}

// Use like:
//   muiltioutput /dev/gpiochip0 0=1 1=1 2=0 3=1 4=0
//
// to set lines 0, 1, & 3 high
//              2 & 4 low
//
fn do_main(args: Cli) -> std::result::Result<(), gpio_cdev::Error> {
    let mut chip = Chip::new(args.chip)?;
    let mut offsets = Vec::new();
    let mut values = Vec::new();

    for arg in args.line_values {
        let lv: Vec<&str> = arg.split("=").collect();
        offsets.push(lv[0].parse::<u32>().unwrap());
        values.push(lv[1].parse::<u8>().unwrap());
    }

    // NOTE: we set the default values to the desired states so
    // setting them separately is not required
    let _handle =
        chip.get_lines(&offsets)?
            .request(LineRequestFlags::OUTPUT, &values, "multioutput")?;

    println!("Output lines being driven... Enter to exit");
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
