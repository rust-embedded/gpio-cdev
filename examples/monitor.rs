// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use gpio_cdev::*;
use nix::poll::*;
use quicli::prelude::*;
use std::os::unix::io::AsRawFd;
use structopt::StructOpt;

type PollEventFlags = nix::poll::PollFlags;

#[derive(Debug, StructOpt)]
struct Cli {
    /// The gpiochip device (e.g. /dev/gpiochip0)
    chip: String,
    /// The offset of the GPIO lines for the provided chip
    lines: Vec<u32>,
}

fn do_main(args: Cli) -> anyhow::Result<()> {
    let mut chip = Chip::new(args.chip)?;

    // Get event handles for each line to monitor.
    let mut evt_handles: Vec<LineEventHandle> = args
        .lines
        .into_iter()
        .map(|off| {
            let line = chip.get_line(off).unwrap();
            line.events(
                LineRequestFlags::INPUT,
                EventRequestFlags::BOTH_EDGES,
                "monitor",
            )
            .unwrap()
        })
        .collect();

    // Create a vector of file descriptors for polling
    let mut pollfds: Vec<PollFd> = evt_handles
        .iter()
        .map(|h| {
            PollFd::new(
                h.as_raw_fd(),
                PollEventFlags::POLLIN | PollEventFlags::POLLPRI,
            )
        })
        .collect();

    loop {
        // poll for an event on any of the lines
        if poll(&mut pollfds, -1)? == 0 {
            println!("Timeout?!?");
        } else {
            for i in 0..pollfds.len() {
                if let Some(revts) = pollfds[i].revents() {
                    let h = &mut evt_handles[i];
                    if revts.contains(PollEventFlags::POLLIN) {
                        let event = h.get_event()?;
                        println!("[{}] {:?}", h.line().offset(), event);

                        // You can figure out the new level from the event,
                        // but this shows that you can use the event handle
                        // to read the value of the bit.
                        let val = h.get_value()?;
                        println!("    {}", val);
                    } else if revts.contains(PollEventFlags::POLLPRI) {
                        println!("[{}] Got a POLLPRI", h.line().offset());
                    }
                }
            }
        }
    }
}

fn main() -> CliResult {
    let args = Cli::from_args();
    do_main(args).or_else(|e| {
        error!("{:?}", e);
        Ok(())
    })
}
