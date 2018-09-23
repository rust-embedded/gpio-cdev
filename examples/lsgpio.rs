// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Clone of functionality of linux/tools/gpio/lsgpio.c

extern crate gpio_cdev;

use gpio_cdev::*;

fn main() {
    let chip_iterator = match chips() {
        Ok(chips) => chips,
        Err(e) => {
            println!("Failed to get chip iterator: {:?}", e);
            return;
        }
    };

    for chip in chip_iterator {
        if let Ok(chip) = chip {
            println!(
                "GPIO chip: {}, \"{}\", \"{}\", {} GPIO Lines",
                chip.path().to_string_lossy(),
                chip.name(),
                chip.label(),
                chip.num_lines()
            );
            for line in chip.lines() {
                match line {
                    Ok(line) => {
                        let mut flags = vec![];

                        if line.is_kernel() {
                            flags.push("kernel");
                        }

                        if line.direction() == LineDirection::Out {
                            flags.push("output");
                        }

                        if line.is_active_low() {
                            flags.push("active-low");
                        }
                        if line.is_open_drain() {
                            flags.push("open-drain");
                        }
                        if line.is_open_source() {
                            flags.push("open-source");
                        }

                        let usage = if flags.is_empty() {
                            format!("[{}]", flags.join(" "))
                        } else {
                            "".to_owned()
                        };

                        println!(
                            "\tline {lineno:>3}: {name} {consumer} {usage}",
                            lineno = line.offset(),
                            name = line.name().unwrap_or("unused"),
                            consumer = line.consumer().unwrap_or("unused"),
                            usage = usage,
                        );
                    }
                    Err(e) => println!("\tError getting line: {:?}", e),
                }
            }
        }
    }
}
