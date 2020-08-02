//! This module is deprecated and types are exported from the top-level of the crate
//!
//! In futures versions of the crate, this module will no longer be included in the crate.

use crate::IoctlKind;
use std::error::Error as StdError;
use std::fmt;
use std::io::Error as IOError;

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    Event(nix::Error),
    Io(IOError),
    Ioctl { kind: IoctlKind, cause: nix::Error },
    InvalidRequest(usize, usize),
    Offset(u32),
}

pub(crate) fn ioctl_err(kind: IoctlKind, cause: nix::Error) -> Error {
    Error {
        kind: ErrorKind::Ioctl { kind, cause },
    }
}

pub(crate) fn invalid_err(n_lines: usize, n_values: usize) -> Error {
    Error {
        kind: ErrorKind::InvalidRequest(n_lines, n_values),
    }
}

pub(crate) fn offset_err(offset: u32) -> Error {
    Error {
        kind: ErrorKind::Offset(offset),
    }
}

pub(crate) fn event_err(err: nix::Error) -> Error {
    Error {
        kind: ErrorKind::Event(err),
    }
}

impl fmt::Display for IoctlKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            IoctlKind::ChipInfo => write!(f, "get chip info"),
            IoctlKind::LineInfo => write!(f, "get line info"),
            IoctlKind::LineHandle => write!(f, "get line handle"),
            IoctlKind::LineEvent => write!(f, "get line event "),
            IoctlKind::GetLine => write!(f, "get line value"),
            IoctlKind::SetLine => write!(f, "set line value"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            ErrorKind::Event(err) => write!(f, "Failed to read event: {}", err),
            ErrorKind::Io(err) => err.fmt(f),
            ErrorKind::Ioctl { cause, kind } => write!(f, "Ioctl to {} failed: {}", kind, cause),
            ErrorKind::InvalidRequest(n_lines, n_values) => write!(
                f,
                "Invalid request: {} values requested to be set but only {} lines are open",
                n_values, n_lines
            ),
            ErrorKind::Offset(offset) => write!(f, "Offset {} is out of range", offset),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.kind {
            ErrorKind::Event(err) => Some(err),
            ErrorKind::Io(err) => Some(err),
            ErrorKind::Ioctl { kind: _, cause } => Some(cause),
            _ => None,
        }
    }
}

impl From<IOError> for Error {
    fn from(err: IOError) -> Error {
        Error {
            kind: ErrorKind::Io(err),
        }
    }
}
