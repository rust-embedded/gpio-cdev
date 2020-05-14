// Copyright (c) 2018 The rust-gpio-cdev Project Developers.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Wrapper for asynchronous programming using Tokio.

use futures::ready;
use futures::stream::Stream;
use futures::task::{Context, Poll};
use mio::event::Evented;
use mio::unix::EventedFd;
use mio::{PollOpt, Ready, Token};
use tokio::io::PollEvented;

use std::io;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;

use super::errors::event_err;
use super::{LineEvent, LineEventHandle, Result};

struct PollWrapper {
    handle: LineEventHandle,
}

impl Evented for PollWrapper {
    fn register(
        &self,
        poll: &mio::Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.handle.file.as_raw_fd()).register(poll, token, interest, opts)
    }

    fn reregister(
        &self,
        poll: &mio::Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.handle.file.as_raw_fd()).reregister(poll, token, interest, opts)
    }

    fn deregister(&self, poll: &mio::Poll) -> io::Result<()> {
        EventedFd(&self.handle.file.as_raw_fd()).deregister(poll)
    }
}

/// Wrapper around a `LineEventHandle` which implements a `futures::stream::Stream` for interrupts.
///
/// # Example
///
/// The following example waits for state changes on an input line.
///
/// ```no_run
/// # type Result<T> = std::result::Result<T, gpio_cdev::errors::Error>;
/// use futures::stream::StreamExt;
/// use gpio_cdev::{AsyncLineEventHandle, Chip, EventRequestFlags, LineRequestFlags};
///
/// async fn print_events(line: u32) -> Result<()> {
///     let mut chip = Chip::new("/dev/gpiochip0")?;
///     let line = chip.get_line(line)?;
///     let mut events = AsyncLineEventHandle::new(line.events(
///         LineRequestFlags::INPUT,
///         EventRequestFlags::BOTH_EDGES,
///         "gpioevents",
///     )?)?;
///
///     loop {
///         match events.next().await {
///             Some(event) => println!("{:?}", event?),
///             None => break,
///         };
///     }
///
///     Ok(())
/// }
///
/// # #[tokio::main]
/// # async fn main() {
/// #     print_events(42).await.unwrap();
/// # }
/// ```
pub struct AsyncLineEventHandle {
    evented: PollEvented<PollWrapper>,
}

impl AsyncLineEventHandle {
    /// Wraps the specified `LineEventHandle`.
    ///
    /// # Arguments
    ///
    /// * `handle` - handle to be wrapped.
    pub fn new(handle: LineEventHandle) -> Result<AsyncLineEventHandle> {
        // The file descriptor needs to be configured for non-blocking I/O for PollEvented to work.
        let fd = handle.file.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL, 0);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        Ok(AsyncLineEventHandle {
            evented: PollEvented::new(PollWrapper { handle })?,
        })
    }
}

impl Stream for AsyncLineEventHandle {
    type Item = Result<LineEvent>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        let ready = Ready::readable();
        if let Err(e) = ready!(self.evented.poll_read_ready(cx, ready)) {
            return Poll::Ready(Some(Err(e.into())));
        }

        match self.evented.get_ref().handle.read_event() {
            Ok(Some(event)) => Poll::Ready(Some(Ok(event))),
            Ok(None) => Poll::Ready(Some(Err(event_err(nix::Error::Sys(
                nix::errno::Errno::EIO,
            ))))),
            Err(nix::Error::Sys(nix::errno::Errno::EAGAIN)) => {
                self.evented.clear_read_ready(cx, ready)?;
                Poll::Pending
            }
            Err(e) => Poll::Ready(Some(Err(event_err(e)))),
        }
    }
}

impl AsRef<LineEventHandle> for AsyncLineEventHandle {
    fn as_ref(&self) -> &LineEventHandle {
        &self.evented.get_ref().handle
    }
}
