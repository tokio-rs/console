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
[subscriber]: https://crates.io/crates/console-subscriber
## Getting Started

To use the console CLI to debug an asynchronous application, the application
must first be instrumented to record `tokio-console` telemetry. The easiest way
to do this is [using the `console-subscriber` crate][subscriber].

Once the application is instrumented, install the console CLI using

```shell
cargo install tokio-console
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

When the console CLI is launched, it displays a list of all [asynchronous tasks]
in the program:

![tasks list](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tasks_list.png)

Using the <kbd>&#8593;</kbd> and <kbd>&#8595;</kbd> arrow keys, an individual task can be highlighted.
Pressing<kbd>enter</kbd> while a task is highlighted displays details about that
task:

![task details](https://raw.githubusercontent.com/tokio-rs/console/main/assets/details2.png)

Pressing the <kbd>escape</kbd> key returns to the task list.

The <kbd>r</kbd> key switches from the list of tasks to a list of [resources],
such as synchronization primitives, I/O resources, et cetera:

![resource list](https://raw.githubusercontent.com/tokio-rs/console/main/assets/resources.png)


Pressing the <kbd>t</kbd> key switches the view back to the task list.

Like the task list view, the resource list view can be navigated using the
<kbd>&#8593;</kbd> and <kbd>&#8595;</kbd> arrow keys. Pressing <kbd>enter</kbd>
while a resource is highlighted displays details about that resource:

![resource details --- oneshot](https://raw.githubusercontent.com/tokio-rs/console/main/assets/resource_details1.png)

The resource details view lists the tasks currently waiting on that resource.
This may be a single task, as in the [`tokio::sync::oneshot`] channel above, or
a large number of tasks, such as this [`tokio::sync::Semaphore`]:

![resource details --- semaphore](https://raw.githubusercontent.com/tokio-rs/console/main/assets/resource_details2.png)

Like the task details view, pressing the <kbd>escape</kbd> key while viewing a resource's details
returns to the resource list.

[`tokio-console`]: https://github.com/tokio-rs/console
[Tokio]: https://tokio.rs
[asynchronous tasks]: https://tokio.rs/tokio/tutorial/spawning#tasks
[resources]: https://tokio.rs/tokio/tutorial/async#async-fn-as-a-future
[`tokio::sync::oneshot`]: https://docs.rs/tokio/latest/tokio/sync/oneshot/index.html
[`tokio::sync::Semaphore`]: https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html

### Command-Line Arguments

Running `tokio-console --help` displays a list of all available command-line
arguments:
```shell
$ tokio-console --help

tokio-console 0.1.0

USAGE:
    tokio-console [OPTIONS] [TARGET_ADDR]

ARGS:
    <TARGET_ADDR>
            The address of a console-enabled process to connect to.

            This may be an IP address and port, or a DNS name.

            [default: http://127.0.0.1:6669]

OPTIONS:
        --ascii-only
            Explicitly use only ASCII characters

        --colorterm <truecolor>
            Overrides the value of the `COLORTERM` environment variable.

            If this is set to `24bit` or `truecolor`, 24-bit RGB color support will be enabled.

            [env: COLORTERM=truecolor]
            [possible values: 24bit, truecolor]

    -h, --help
            Print help information

        --lang <LANG>
            Overrides the terminal's default language

            [env: LANG=en_US.UTF-8]
            [default: en_us.UTF-8]

        --log <ENV_FILTER>
            Log level filter for the console's internal diagnostics.

            The console will log to stderr if a log level filter is provided. Since the console
            application runs interactively, stderr should generally be redirected to a file to avoid
            interfering with the console's text output.

            [env: RUST_LOG=]
            [default: off]

        --no-colors
            Disable ANSI colors entirely

        --no-duration-colors
            Disable color-coding for duration units

        --no-terminated-colors
            Disable color-coding for terminated tasks

        --palette <PALETTE>
            Explicitly set which color palette to use

            [possible values: 8, 16, 256, all, off]

        --retain-for <RETAIN_FOR>
            How long to continue displaying completed tasks and dropped resources after they have
            been closed.

            This accepts either a duration, parsed as a combination of time spans (such as `5days
            2min 2s`), or `none` to disable removing completed tasks and dropped resources.

            Each time span is an integer number followed by a suffix. Supported suffixes are:

            * `nsec`, `ns` -- nanoseconds

            * `usec`, `us` -- microseconds

            * `msec`, `ms` -- milliseconds

            * `seconds`, `second`, `sec`, `s`

            * `minutes`, `minute`, `min`, `m`

            * `hours`, `hour`, `hr`, `h`

            * `days`, `day`, `d`

            * `weeks`, `week`, `w`

            * `months`, `month`, `M` -- defined as 30.44 days

            * `years`, `year`, `y` -- defined as 365.25 days

            [default: 6s]

    -V, --version
            Print version information
```

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
