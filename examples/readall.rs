// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use clap::Parser;
use gpio_cdev::{Chip, LineRequestFlags};

#[derive(Debug, Parser)]
struct Cli {
    /// The gpiochip device (e.g. /dev/gpiochip0)
    chip: String,
}

fn do_main(args: Cli) -> std::result::Result<(), gpio_cdev::Error> {
    let mut chip = Chip::new(args.chip)?;
    let ini_vals = vec![0; chip.num_lines() as usize];
    let handle = chip
        .get_all_lines()?
        .request(LineRequestFlags::INPUT, &ini_vals, "readall")?;
    println!("Values: {:?}", handle.get_values()?);

    Ok(())
}

fn main() -> std::result::Result<(), gpio_cdev::Error> {
    let args = Cli::parse();
    do_main(args)
}
