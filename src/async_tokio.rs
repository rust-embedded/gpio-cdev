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
use tokio::io::unix::{AsyncFd, TryIoError};

use std::os::unix::io::AsRawFd;
use std::pin::Pin;

use super::event_err;
use super::{LineEvent, LineEventHandle, Result};

/// Wrapper around a `LineEventHandle` which implements a `futures::stream::Stream` for interrupts.
///
/// # Example
///
/// The following example waits for state changes on an input line.
///
/// ```no_run
/// use futures::stream::StreamExt;
/// use gpio_cdev::{AsyncLineEventHandle, Chip, EventRequestFlags, LineRequestFlags};
///
/// async fn print_events(line: u32) -> Result<(), gpio_cdev::Error> {
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
    asyncfd: AsyncFd<LineEventHandle>,
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
            asyncfd: AsyncFd::new(handle)?,
        })
    }
}

impl Stream for AsyncLineEventHandle {
    type Item = Result<LineEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<Self::Item>> {
        loop {
            let mut guard = ready!(self.asyncfd.poll_read_ready_mut(cx))?;
            match guard.try_io(|inner| inner.get_mut().read_event()) {
                Err(TryIoError { .. }) => {
                    // Continue
                }
                Ok(Ok(Some(event))) => return Poll::Ready(Some(Ok(event))),
                Ok(Ok(None)) => return Poll::Ready(Some(Err(event_err(nix::errno::Errno::EIO)))),
                Ok(Err(err)) => return Poll::Ready(Some(Err(err.into()))),
            }
        }
    }
}

impl AsRef<LineEventHandle> for AsyncLineEventHandle {
    fn as_ref(&self) -> &LineEventHandle {
        &self.asyncfd.get_ref()
    }
}
