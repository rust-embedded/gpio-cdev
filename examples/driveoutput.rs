// Copyright (c) 2018 The rust-gpio-CDC Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate gpio_cdev;
#[macro_use] extern crate quicli;

use gpio_cdev::*;
use quicli::prelude::*;

#[derive(Debug, StructOpt)]
struct Cli {
    /// The gpiochip device (e.g. /dev/gpiochip0)
    chip: String,
    /// The offset of the GPIO line for the provided chip
    line: u32,
    /// The value to write
    value: u8,
}

fn do_main(args: Cli) -> errors::Result<()> {
    let mut chip = Chip::new(args.chip)?;

    // NOTE: we set the default value to the desired state so
    // setting it separately is not required
    let _handle = chip.get_line(args.line)?.request(RequestFlags::OUTPUT, args.value, "readinput")?;

    println!("Output being driven... Enter to exit");
    let mut buf = String::new();
    drop(::std::io::stdin().read_line(&mut buf)?);

    Ok(())
}

main!(|args: Cli| {
    match do_main(args) {
        Ok(()) => {},
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
});
