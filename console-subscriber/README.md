# tokio-console subscriber

&#x1F4E1;&#xFE0F;  A [`tracing-subscriber`] [`Layer`] for collecting
[`tokio-console`] instrumentation.

[![crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (`main` branch)][docs-main-badge]][docs-main-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[Website](https://tokio.rs) | [Chat][discord-url] | [API Documentation][docs-url]

[crates-badge]: https://img.shields.io/crates/v/console-subscriber.svg
[crates-url]: https://crates.io/crates/console-subscriber
[docs-badge]: https://docs.rs/console-subscriber/badge.svg
[docs-url]: https://docs.rs/console-subscriber
[docs-main-badge]: https://img.shields.io/netlify/0e5ffd50-e1fa-416e-b147-a04dab28cfb1?label=docs%20%28main%20branch%29
[docs-main-url]: https://tokio-console.netlify.app/console_subscriber/
[mit-badge]: https://img.shields.io/badge/license-MIT-blue.svg
[mit-url]: ../LICENSE
[actions-badge]: https://github.com/tokio-rs/console/workflows/CI/badge.svg
[actions-url]:https://github.com/tokio-rs/console/actions?query=workflow%3ACI
[discord-badge]: https://img.shields.io/discord/500028886025895936?logo=discord&label=discord&logoColor=white

## Overview

[`tokio-console`] is a debugging and profiling tool for asynchronous Rust
applications, which collects and displays in-depth diagnostic data on the
asynchronous tasks, resources, and operations in an application. The console
system consists of two primary components:

* _instrumentation_, embedded in the application, which collects data from the
  async runtime and exposes it over the console's [wire format]
* _consumers_, such as the [`tokio-console`] command-line application, which
  connect to the instrumented application, recieve telemetry data, and display
  it to the user

This crate implements the instrumentation-side interface using data
emitted by the async runtime using the [`tracing`]. It provides a type
implementing the [`Layer`] trait from [`tracing-subscriber`], for collecting and
aggregating the runtime's [`tracing`] data, and a gRPC server that exports
telemetry to clients.

[wire format]: https://crates.io/crates/console-api

## Getting Started

To instrument your asynchronous application, you must be using an async runtime
that supports the [`tracing`] instrumentation required by the console.
Currently, the only runtime that implements this instrumentation is [Tokio]
version 1.7.0 and newer.

### Enabling Tokio Instrumentation

&#x26A0;&#xFE0F; Currently, the [`tracing`] support in the [`tokio`
runtime][Tokio] is considered *experimental*. In order to use
`console-subscriber` with Tokio, the following is required:

* Tokio's optional `tracing` dependency must be enabled. For example:
  ```toml
  [dependencies]
  # ...
  tokio = { version = "1.15", features = ["full", "tracing"] }
  ```

* The `tokio_unstable` cfg flag, which enables experimental APIs in Tokio, must
  be enabled. It can be enabled by setting the `RUSTFLAGS` environment variable
  at build-time:
  ```shell
  $ RUSTFLAGS="--cfg tokio_unstable" cargo build
  ```
  or, by adding the following to the `.cargo/config` file in a Cargo workspace:
  ```toml
  [build]
  rustflags = ["--cfg", "tokio_unstable"]
  ```
### Adding the Console Subscriber

If the runtime emits compatible `tracing` events, enabling the console is as
simple as adding the following line to your `main` function:

```rust
console_subscriber::init();
```

This sets the [default `tracing` subscriber][default] to serve console telemetry
(as well as logging to stdout based on the `RUST_LOG` environment variable). The
console subscriber's behavior can be configured via a set of
[environment variables][env].

For programmatic configuration, a [builder interface][builder] is also provided:

```rust
use std::time::Duration;

console_subscriber::TasksLayer::builder()
    // set how long the console will retain data from completed tasks
    .retention(Duration::from_secs(60))
    // set the address the server is bound to
    .server_addr(([127, 0, 0, 1], 5555))
    // ... other configurations ...
    .init();
```

The layer provided by this crate can also be combined with other [`Layer`]s from
other crates:

```rust
use tracing_subscriber::prelude::*;

// spawn the console server in the background,
// returning a `Layer`:
let console_layer = console_subscriber::spawn();

// build a `Subscriber` by combining layers with a
// `tracing_subscriber::Registry`:
tracing_subscriber::registry()
    // add the console layer to the subscriber
    .with(tasks_layer)
    // add other layers...
    .with(tracing_subscriber::fmt::layer())
 // .with(...)
    .init();
```

[`tracing`]: https://crates.io/crates/tracing
[`tracing-subscriber`]: https://crates.io/crates/tracing-subscriber
[`Layer`]:https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/layer/index.html
[default]: https://docs.rs/tracing/latest/0.1/dispatcher/index.html#setting-the-default-subscriber
[env]: https://docs.rs/console-subscriber/0.1/console-subscriber/struct.builder#method.with_default_env
[builder]: https://docs.rs/console-subscriber/0.1/console-subscriber/struct.builder
[`tokio-console`]: https://github.com/tokio-rs/console
[Tokio]: https://tokio.rs

### Crate Feature Flags

This crate provides the following feature flags and optional dependencies:

* [`parking_lot`]: Use the [`parking_lot`] crate's locks, rather than `std::sync`.
  Using [`parking_lot`] may result in improved performance, especially in highly
  concurrent applications. Disabled by default.

[`parking_lot`]: https://crates.io/crates/parking_lot

## Getting Help

First, see if the answer to your question can be found in the
[API documentation]. If the answer is not there, there is an active community in
the [Tokio Discord server][discord-url]. We would be happy to try to answer your
question. You can also ask your question on [the discussions page][discussions].

[API documentation]: https://docs.rs/console-subscriber
[discussions]: https://github.com/tokio-rs/console/discussions
[discord-url]: https://discord.gg/tokio

## Contributing

&#x1f388; Thanks for your help improving the project! We are so happy to have
you! We have a [contributing guide][guide] to help you get involved in the Tokio
console project.

[guide]: https://github.com/tokio-rs/console/blob/main/CONTRIBUTING.md

## Supported Rust Versions

The Tokio console is built against the latest stable release. The minimum
supported version is 1.56. The current Tokio console version is not guaranteed
to build on Rust versions earlier than the minimum supported version.

## License

This project is licensed under the [MIT license].

[MIT license]: https://github.com/tokio-rs/console/blob/main/LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tokio by you, shall be licensed as MIT, without any additional
terms or conditions.
