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
    /// The offset of the GPIO lines for the provided chip
    lines: Vec<u32>,
}

fn do_main(args: Cli) -> std::result::Result<(), gpio_cdev::Error> {
    let mut chip = Chip::new(args.chip)?;
    let ini_vals = vec![0; args.lines.len()];
    let handle =
        chip.get_lines(&args.lines)?
            .request(LineRequestFlags::INPUT, &ini_vals, "multiread")?;
    println!("Values: {:?}", handle.get_values()?);

    Ok(())
}

fn main() -> CliResult {
    let args = Cli::from_args();
    do_main(args).or_else(|e| {
        error!("{:?}", e);
        Ok(())
    })
}
