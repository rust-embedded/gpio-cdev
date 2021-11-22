# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic
Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]


## [v0.5.1] - 2021-11-22

- Updated nix to version `0.23`.

## [v0.5.0] - 2021-09-21

- Update Tokio to 1.x. [#55](https://github.com/rust-embedded/gpio-cdev/pull/55).
- Fix lsgpio example to output gpio line flags.
- Add `is_empty()` function for `Lines` struct.
- Add common trait implementations for public structures.
- Breaking change of `LineEventHandle::get_event()` which now expects `&mut self`.
- MSRV is now 1.46.0.
- Updated `nix` to version `0.22`.
- Updated `quicli` to version `0.4`.
- Updated `bitflags` to version `1.3`.


## [v0.4.0] - 2020-08-01

- Removed pub "errors" module.  Error now exposed at top level.
- MSRV is now 1.39.0
- Add support behind a feature flag for reading events from a line as a Stream via tokio. [#35](https://github.com/rust-embedded/gpio-cdev/pull/35).

## [v0.3.0] - 2020-02-10

Refactored Errors:
- Removed the `error-chain` dependency.
- Errors are now implemented "manually" with `ErrorKind` and `IoctlKind` enums.
- The encompassing `Error` type implements the `std::error::Error` trait.

## v0.2.0 - 2018-12-12

Adds the ability to create a collection of lines from a single chip and read or write those lines simultaneously with a single stystem call.

- A new `Lines` object (plural) was added. It is a collection of individual `Line` objects on a single `Chip` which can be read or written simultaneously with a single system call.
- A `Line` now just contains the reference to the Chip and the offset number. No system call is incurred when one is created.
- Information about an individual line is now represented by a separate `LineInfo` struct which can be obtained from the function `Line::info()`. This incurs a system call to retrieve the information.
- Creating a `Line` can't fail unless the caller specifies an offset that is out of range of the chip.
- The `LineIterator` can not fail since it checks the offset range. So now its item is just a `Line`, and not `Result<Line>`.
- There was no longer a need for `Line::refresh()` so it was removed.
- Since a `Line` object is trivial to create, it is now OK to have `Lines` be a simple collection of `Line` structs.

## v0.1.0 - 2018-09-28

- Initial release of the library with basic operations centered around operating
  on a single line at a time.

[Unreleased]: https://github.com/rust-embedded/gpio-cdev/compare/0.5.1...HEAD
[v0.5.1]: https://github.com/rust-embedded/gpio-cdev/compare/0.5.0...0.5.1
[v0.5.0]: https://github.com/rust-embedded/gpio-cdev/compare/0.4.0...0.5.0
[v0.4.0]: https://github.com/rust-embedded/gpio-cdev/compare/0.3.0...0.4.0
