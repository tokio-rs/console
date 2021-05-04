# tokio-console prototypes

⚠️ **extremely serious warning:** this is _pre-alpha_, proof-of-concept
software! currently, the wire format has _no stability guarantees_ &mdash;
the crates in this repository are not guaranteed to be interoperable except
within the same Git revision. when these crates are published to crates.io, the
wire format will follow semver, but currently, anything could happen!

## what's all this, then?

this repository contains a prototype implementation of TurboWish/tokio-console,
a diagnostics and debugging tool for asynchronous Rust programs. the diagnostic
toolkit consists of multiple components:

* a **wire protocol for streaming diagnostic data** from instrumented applications
  to diagnostic tools. the wire format is defined using [gRPC] and [protocol
  buffers], for efficient transport on the wire and interoperability between
  different implementations of data producers and consumers.

  the [`console-api`] crate contains generated code for this wire format for
  projects using the [`tonic`] gRPC implementation. additionally, projects using
  other gRPC code generators (including those in other languages!) can depend on
  [the protobuf definitions] themselves.

* **instrumentation for collecting diagnostic data** from a process and exposing
  it over the wire format. the [`console-subscriber`] crate in this repository
  contains **an implementation of the instrumentation-side API as a
  [`tracing-subscriber`] [`Layer`]**, for projects using [Tokio] and
  [`tracing`].

* tools for **displaying and exploring diagnostic data**, implemented as gRPC
  clients using the console wire protocol. the [`console`] crate implements an
  **an interactive command-line tool** that consumes this data, but **other
  implementations**, such as graphical or web-based tools, are also possible.

[gRPC]: https://grpc.io/
[protocol buffers]: https://developers.google.com/protocol-buffers
[`tonic`]: https://lib.rs/crates/tonic
[Tokio]: https://tokio.rs

## using it

to **instrument an application using Tokio**, add a dependency on the
[`console-subscriber`] crate, and **add the `TasksLayer` type** to your
[`tracing`] subscriber. for example:
```rust
    use tracing_subscriber::{prelude::*, fmt, EnvFilter};
    // construct the `console_subscriber` layer and the console wire protocol server
    let (layer, server) = console_subscriber::TasksLayer::new();
    // ensure that Tokio's internal instrumentation is enabled
    let filter = EnvFilter::from_default_env().add_directive("tokio=trace".parse()?);

    tracing_subscriber::registry()
        // the `TasksLayer` can be used in combination with other `tracing` layers...
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .with(layer)
        .init();

    // spawn the server task
    tokio::spawn(server);
```

notes:

* in order to collect task data from Tokio, **the `tokio_unstable` cfg must be
  enabled**. for example, you could build your project with
  ```shell
  $ RUSTFLAGS="--cfg tokio_unstable" cargo build
  ```
  or add the following to your `.cargo/config` file:
  ```toml
  [build]
  rustflags = ["--cfg", "tokio_unstable"]
  ```
* the `tokio::task` [`tracing` target] must be enabled

to **run the console command line tool**, simply
```shell
$ cargo run
```
in this repository.

## for development:

the `console-subscriber/examples` directory contains **some potentially useful
tools**:

* `app.rs`: a very simple example program that spawns a bunch of tasks in a loop
  forever
* `dump.rs`: a simple CLI program that dumps the data stream from a `Tasks`
  server

[`tracing`]: https://lib.rs/crates/tracing
[`tracing-subscriber`]: https://lib.rs/crates/tracing-subscriber
[`console-api`]: ../console-api
[`console-subscriber`]: ../console-subscriber
[`console`]: ../console
[`Layer`]: https://docs.rs/tracing-subscriber/0.2.18/tracing_subscriber/layer/trait.Layer.html
[`tracing` target]: https://docs.rs/tracing/0.1.26/tracing/struct.Metadata.html