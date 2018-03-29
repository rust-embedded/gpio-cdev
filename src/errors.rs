// Copyright (c) 2018 The rust-gpio-CDC Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

error_chain! {
    types {
        Error,
        ErrorKind,
        ResultExt,
        Result;
    }

    foreign_links {
        Nix(::nix::Error);
        Io(::std::io::Error);
    }
}
