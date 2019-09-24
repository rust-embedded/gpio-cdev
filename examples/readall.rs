// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate gpio_cdev;
#[macro_use]
extern crate quicli;

use std::cmp::min;
use gpio_cdev::*;
use quicli::prelude::*;

#[derive(Debug, StructOpt)]
struct Cli {
    /// The gpiochip device (e.g. /dev/gpiochip0)
    chip: String,
}

fn do_main(args: Cli) -> errors::Result<()> {
    let mut chip = Chip::new(args.chip)?;
    let nlines = chip.num_lines() as usize;

    println!("Found {:?} with {} lines.", chip.path(), nlines);

    // Note that the kernel has a hard limit of 64 lines in a request.
    // That presents a limit to the size of a Lines struct.
    // So chip.get_all_lines() will fail if the chip has more than 64
    // lines. In that case we can read smaller blocks of lines, or
    // one at a time.

    match chip.get_all_lines() {
        Ok(lines) => {
            let ini_vals = vec![ 0; nlines ];
            let handle = lines.request(LineRequestFlags::INPUT, &ini_vals, "readall")?;
            let values = handle.get_values()?;
            println!("{:?}", values);
        }
        Err(_err) => {
            let mut nread: usize = 0;

            // Block size can be 1 to Lines::MAX_LINES (64)
            let block_sz: usize = 4;

            while nread < nlines {
                let n = min(block_sz, nlines-nread);
                let istart = nread as u32;
                let iend = istart + n as u32;
                let ini_vals = vec![ 0; n ];

                let handle = chip
                    .get_range_lines(istart..iend)?
                    .request(LineRequestFlags::INPUT, &ini_vals, "readall")?;

                let values = handle.get_values()?;
                println!("({}) {:?}", istart, values);
                nread += n;
            }
        }
    }

    Ok(())
}

main!(|args: Cli| match do_main(args) {
    Ok(()) => {}
    Err(e) => {
        println!("Error: {:?}", e);
    }
});
