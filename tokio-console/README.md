# tokio-console CLI

&#x1f39b;&#xfe0f; The [Tokio console][`tokio-console`]: a debugger for
asynchronous Rust programs.

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

* &#x1f4e1;&#xfe0f; _instrumentation_, embedded in the application, which
  collects data from the async runtime and exposes it over the console's wire
  format
* &#x1f6f0;&#xfe0f; _consumers_, which connect to the instrumented application,
  receive telemetry data, and display it to the user

This crate is the primary consumer of `tokio-console` telemetry, a command-line
application that provides an interactive debugging interface.

[wire format]: https://crates.io/crates/console-api
[subscriber]: https://crates.io/crates/console-subscriber
## Getting Started

To use the console to monitor and debug a program, it must be instrumented to
emit the data the console consumes. Then, the `tokio-console` CLI application
can be used to connect to the application and monitor its operation.
### Instrumenting the Application

Before the console can connect to an application, it must first be instrumented
to record `tokio-console` telemetry. The easiest way  to do this is [using the
`console-subscriber` crate][subscriber].

`console-subscriber` requires that the application's async runtime (or runtimes)
emit [`tracing`] data in a format that the console can record. For programs that
use the [Tokio] runtime, this means that:

- Tokio's [unstable features][unstable] must be enabled. See [the `console-subscriber`
  documentation][unstable] for details.
- A [compatible Tokio version][versions] must be used. Tokio v1.0 or greater is required
  to use the console, and some features are only available in later versions.
  See [the `console-subscriber` documentation][versions] for details.
 
[`tracing`]: https://crates.io/crates/tracing
[unstable]: https://docs.rs/console-subscriber/0.1/console_subscriber/#enabling-tokio-instrumentation
[versions]: https://docs.rs/console-subscriber/0.1/console_subscriber/#required-tokio-versions

### Using the Console

Once the application is instrumented, install the console CLI using

```shell
cargo install --locked tokio-console
```

Running `tokio-console` without any arguments will connect to an application on
localhost listening on the default port, port 6669:

```shell
tokio-console
```

If the application is not running locally, or was configured to listen on a
different port, the console will also accept a target address as a command-like
argument:

```shell
tokio-console http://192.168.0.42:9090
```

A DNS name can also be provided as the target address:
```shell
tokio-console http://my.instrumented.application.local:6669
```

See [here][cli-ref] for a complete list of all command-line arguments.

When the console CLI is launched, it displays a list of all [asynchronous tasks]
in the program:

![tasks list](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tokio-console-0.1.8/tasks_list.png)

Using the <kbd>&#8593;</kbd> and <kbd>&#8595;</kbd> arrow keys, an individual task can be highlighted.
Pressing<kbd>enter</kbd> while a task is highlighted displays details about that
task:

![task details](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tokio-console-0.1.8/task_details.png)

Pressing the <kbd>escape</kbd> key returns to the task list.

The <kbd>r</kbd> key switches from the list of tasks to a list of [resources],
such as synchronization primitives, I/O resources, et cetera:

![resource list](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tokio-console-0.1.8/resources_list.png)


Pressing the <kbd>t</kbd> key switches the view back to the task list.

Like the task list view, the resource list view can be navigated using the
<kbd>&#8593;</kbd> and <kbd>&#8595;</kbd> arrow keys. Pressing <kbd>enter</kbd>
while a resource is highlighted displays details about that resource:

![resource details --- sleep](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tokio-console-0.1.8/resource_details_sleep.png)

The resource details view lists the tasks currently waiting on that resource.
This may be a single task, as in the [`tokio::time::Sleep`] above, or
a large number of tasks, such as this private `tokio::sync::batch_semaphore::Semaphore`:

![resource details --- semaphore](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tokio-console-0.1.8/resource_details_semaphore.png)

Like the task details view, pressing the <kbd>escape</kbd> key while viewing a resource's details
returns to the resource list.

A configuration file (`console.toml`) can be used to configure the console's
behavior. See [the documentation][cfg-ref] for details.

[`tokio-console`]: https://github.com/tokio-rs/console
[Tokio]: https://tokio.rs
[asynchronous tasks]: https://tokio.rs/tokio/tutorial/spawning#tasks
[resources]: https://tokio.rs/tokio/tutorial/async#async-fn-as-a-future
[`tokio::sync::oneshot`]: https://docs.rs/tokio/latest/tokio/sync/oneshot/index.html
[`tokio::sync::Semaphore`]: https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html
[cli-ref]: https://docs.rs/tokio-console/latest/tokio_console/config_reference/index.html#command-line-arguments
[cfg-ref]: https://docs.rs/tokio-console/latest/tokio_console/config_reference/index.html#configuration-file

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
supported version is 1.58. The current Tokio console version is not guaranteed
to build on Rust versions earlier than the minimum supported version.

## License

This project is licensed under the [MIT license].

[MIT license]: https://github.com/tokio-rs/console/blob/main/LICENSE

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tokio by you, shall be licensed as MIT, without any additional
terms or conditions.
