# tokio-console CLI

&#x1f39b;&#xfe0f; The [`tokio-console`] command-line application.

[![crates.io][crates-badge]][crates-url]
[![Documentation][docs-badge]][docs-url]
[![Documentation (`main` branch)][docs-main-badge]][docs-main-url]
[![MIT licensed][mit-badge]][mit-url]
[![Build Status][actions-badge]][actions-url]
[![Discord chat][discord-badge]][discord-url]

[Website](https://tokio.rs) | [Chat][discord-url] | [API Documentation][docs-url]

[crates-badge]: https://img.shields.io/crates/v/tokio-console.svg
[crates-url]: https://crates.io/crates/tokio-console
[docs-badge]: https://docs.rs/tokio-console/badge.svg
[docs-url]: https://docs.rs/tokio-console
[docs-main-badge]: https://img.shields.io/netlify/0e5ffd50-e1fa-416e-b147-a04dab28cfb1?label=docs%20%28main%20branch%29
[docs-main-url]: https://tokio-console.netlify.app/tokio_console/
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
* _consumers_, which connect to the instrumented application, recieve telemetry
  data, and display it to the user

This crate is the primary consumer of `tokio-console` telemetry, a command-line
application that provides an interactive debugging interface.

[wire format]: https://crates.io/crates/console-api

## Getting Started

To use the console CLI to debug an asynchronous application, the application
must first be instrumented to record `tokio-console` telemetry. The easiest way
to do this is [using the `console-subscriber` crate][subscriber].

Once the application is instrumented, install the console CLI using

```shell
cargo install tokio-console
```

[`tracing`]: https://crates.io/crates/tracing
[`tracing-subscriber`]: https://crates.io/crates/tracing-subscriber
[`Layer`]:https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/layer/index.html
[default]: https://docs.rs/tracing/latest/0.1/dispatcher/index.html#setting-the-default-subscriber
[env]: https://docs.rs/tokio-console/0.1/tokio-console/struct.builder#method.with_default_env
[builder]: https://docs.rs/tokio-console/0.1/tokio-console/struct.builder
[`tokio-console`]: https://github.com/tokio-rs/console
[Tokio]: https://tokio.rs


## Getting Help

First, see if the answer to your question can be found in the
[API documentation]. If the answer is not there, there is an active community in
the [Tokio Discord server][discord-url]. We would be happy to try to answer your
question. You can also ask your question on [the discussions page][discussions].

[API documentation]: https://docs.rs/tokio-console
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
