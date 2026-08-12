// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use clap::Parser;
use futures::stream::StreamExt;
use gpio_cdev::{AsyncLineEventHandle, Chip, EventRequestFlags, LineRequestFlags};

#[derive(Debug, Parser)]
struct Cli {
    /// The gpiochip device (e.g. /dev/gpiochip0)
    chip: String,
    /// The offset of the GPIO line for the provided chip
    line: u32,
}

async fn do_main(args: Cli) -> std::result::Result<(), gpio_cdev::Error> {
    let mut chip = Chip::new(args.chip)?;
    let line = chip.get_line(args.line)?;
    let mut events = AsyncLineEventHandle::new(line.events(
        LineRequestFlags::INPUT,
        EventRequestFlags::BOTH_EDGES,
        "gpioevents",
    )?)?;

    while let Some(event) = events.next().await {
        println!("{:?}", event?);
    }

    Ok(())
}

#[tokio::main]
async fn main() -> std::result::Result<(), gpio_cdev::Error> {
    let args = Cli::parse();
    do_main(args).await
}
