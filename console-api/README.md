# tokio-console API

&#x1f6f0; [Tonic] bindings for the [`tokio-console`] [protobuf] wire format.

[![crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (`main` branch)][docs-main-badge]][docs-main-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[Website](https://tokio.rs) | [Chat][discord-url] | [API Documentation][docs-url]

[crates-badge]: https://img.shields.io/crates/v/console-api.svg
[crates-url]: https://crates.io/crates/console-api
[docs-badge]: https://docs.rs/console-api/badge.svg
[docs-url]: https://docs.rs/console-api
[docs-main-badge]: https://img.shields.io/netlify/0e5ffd50-e1fa-416e-b147-a04dab28cfb1?label=docs%20%28main%20branch%29
[docs-main-url]: https://tokio-console.netlify.app/console_api/
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: ../LICENSE
[actions-badge]: https://github.com/tokio-rs/console/workflows/CI/badge.svg
[actions-url]:https://github.com/tokio-rs/console/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white

## Overview

This crate contains generated [protobuf] bindings for the [`tokio-console`] wire
format. The wire format is used to export diagnostic data from instrumented
applications to consumers that aggregate and display that data.

[`tokio-console`] is a debugging and profiling tool for asynchronous Rust
applications, which collects and displays in-depth diagnostic data on the
asynchronous tasks, resources, and operations in an application. The console
system consists of two primary components:

* _instrumentation_, embedded in the application, which collects data from the
  async runtime and exposes it over the console's wire format
* _consumers_, such as the [`tokio-console`] command-line application, which
  connect to the instrumented application, receive telemetry data, and display
  it to the user

The wire format [protobuf] bindings in this crate are used by both the
instrumentation in the [`console-subscriber`] crate, which emits telemetry in
this format, and by the clients that consume that telemetry.

In general, most [`tokio-console`] users will *not* depend on this crate
directly. Applications are typically instrumented using the
[`console-subscriber`] crate, which collects data and exports it using
this wire format; this data can be consumed using the [`tokio-console`]
command-line application. However, the wire format API definition in this crate
may be useful for anyone implementing other software that also consumes the
[`tokio-console`] diagnostic data.

[`tokio-console`]: https://github.com/tokio-rs/console
[`console-subscriber`]: https://crates.io/crates/console-subscriber
[protobuf]: https://developers.google.com/protocol-buffers

### Stability

&#x26A0;&#xfe0f; The protobuf wire format is not currently considered totally
stable. While we will try to avoid unnecessary protobuf-incompatible changes,
protobuf compatibility is only guaranteed within SemVer-compatible releases of
this crate. For example, the protobuf as of `console-api` v0.2.5 may not be
backwards-compatible with `console-api` v0.1.12.

### Crate Feature Flags

This crate provides the following feature flags:

* `transport`: Generate code that is compatible with [Tonic]'s [`transport`
  module] (disabled by default)

[Tonic]: https://crates.io/crates/tonic
[`transport` module]: https://docs.rs/tonic/latest/tonic/transport/index.html

## Getting Help

First, see if the answer to your question can be found in the
[API documentation]. If the answer is not there, there is an active community in
the [Tokio Discord server][discord-url]. We would be happy to try to answer your
question. You can also ask your question on [the discussions page][discussions].

[API documentation]: https://docs.rs/console-api
[discussions]: https://github.com/tokio-rs/console/discussions
[discord-url]: https://discord.gg/tokio

## Contributing

&#x1f388; Thanks for your help improving the project! We are so happy to have
you! We have a [contributing guide][guide] to help you get involved in the Tokio
console project.

[guide]: https://github.com/tokio-rs/console/blob/main/CONTRIBUTING.md

## Supported Rust Versions

The Tokio console is built against the latest stable release. The minimum
supported version is 1.58. The current Tokio console version is not guaranteed
to build on Rust versions earlier than the minimum supported version.

## License

This project is licensed under the [MIT license].

[MIT license]: https://github.com/tokio-rs/console/blob/main/LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tokio by you, shall be licensed as MIT, without any additional
terms or conditions.
