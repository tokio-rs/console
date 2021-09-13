# tokio-console prototypes

[![API Documentation (`main`)](https://img.shields.io/netlify/0e5ffd50-e1fa-416e-b147-a04dab28cfb1?label=docs%20%28main%29)][main-docs]

⚠️ **extremely serious warning:** this is _pre-alpha_, proof-of-concept
software! currently, the wire format has _no stability guarantees_ &mdash;
the crates in this repository are not guaranteed to be interoperable except
within the same Git revision. when these crates are published to crates.io, the
wire format will follow semver, but currently, anything could happen!

[API Documentation (`main` branch)][main-docs]

[main-docs]: https://tokio-console.netlify.app

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
[the protobuf definitions]: https://github.com/tokio-rs/console/tree/main/console-api/proto
[`tonic`]: https://lib.rs/crates/tonic
[Tokio]: https://tokio.rs

## extremely cool and amazing screenshots

wow! whoa! it's like `top(1)` for tasks!

![task list view](https://user-images.githubusercontent.com/2796466/129774465-7bd2ad2f-f1a3-4830-a8fa-f72667028fa1.png)

viewing details for a single task:

![task details view](https://user-images.githubusercontent.com/2796466/129774524-288c967b-6066-4f98-973d-099b3e6a2c55.png)

## on the shoulders of giants...

the console is **part of a much larger effort** to improve debugging tooling for
async Rust. **a [2019 Google Summer of Code project][gsoc] by Matthias Prechtl**
([**@matprec**]) implemented an initial prototype, with a focus on interactive log
viewing. more recently, both **the [Tokio team][tokio-blog] and the [async
foundations working group][shiny-future]** have made diagnostics and debugging
tools a priority for async Rust in 2021 and beyond. in particular, a
[series][tw-1] of [blog][tw-2] [posts][tw-3] by [**@pnkfelix**] lay out much of
the vision that this project seeks to eventually implement.

furthermore, we're indebted to our antecedents in other programming languages
and environments for inspiration. this includes tools and systems such as
[`pprof`], Unix [`top(1)`] and [`htop(1)`], XCode's [Instruments], and many
others.

[gsoc]: https://github.com/tokio-rs/console-gsoc
[tokio-blog]: https://tokio.rs/blog/2020-12-tokio-1-0#tracing
[shiny-future]: https://rust-lang.github.io/wg-async-foundations/vision/shiny_future/barbara_makes_a_wish.html
[tw-1]: http://blog.pnkfx.org/blog/2021/04/26/road-to-turbowish-part-1-goals/
[tw-2]: http://blog.pnkfx.org/blog/2021/04/27/road-to-turbowish-part-2-stories/
[tw-3]: http://blog.pnkfx.org/blog/2021/05/03/road-to-turbowish-part-3-design/
[`pprof`]: https://github.com/google/pprof
[`top(1)`]: https://man7.org/linux/man-pages/man1/top.1.html
[`htop(1)`]: https://htop.dev/
[Instruments]: https://developer.apple.com/library/archive/documentation/ToolsLanguages/Conceptual/Xcode_Overview/MeasuringPerformance.html
[**@matprec**]: https://github.com/matprec
[**@pnkfelix**]: https://github.com/pnkfelix
## using it

to **instrument an application using Tokio**, add a dependency on the
[`console-subscriber`] crate, and **add this one-liner** to the top of your
`main` function:

```rust
console_subscriber::init();
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

Examples can be executed with:

```shell
cargo run --example $name
```

[`tracing`]: https://lib.rs/crates/tracing
[`tracing-subscriber`]: https://lib.rs/crates/tracing-subscriber
[`console-api`]: ./console-api
[`console-subscriber`]: ./console-subscriber
[`console`]: ./console
[`Layer`]: https://docs.rs/tracing-subscriber/0.2.18/tracing_subscriber/layer/trait.Layer.html
[`tracing` target]: https://docs.rs/tracing/0.1.26/tracing/struct.Metadata.html
