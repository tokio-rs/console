# tokio-console CLI

&#x1f39b;&#xfe0f; the [Tokio console][`tokio-console`]: a debugger for
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

## overview

[`tokio-console`] is a debugging and profiling tool for asynchronous Rust
applications, which collects and displays in-depth diagnostic data on the
asynchronous tasks, resources, and operations in an application. the console
system consists of two primary components:

* &#x1f4e1;&#xfe0f; _instrumentation_, embedded in the application, which
  collects data from the async runtime and exposes it over the console's wire
  format
* &#x1f6f0;&#xfe0f; _consumers_, which connect to the instrumented application,
  receive telemetry data, and display it to the user

this crate is the primary consumer of `tokio-console` telemetry, a command-line
application that provides an interactive debugging interface.

[wire format]: https://crates.io/crates/console-api
[subscriber]: https://crates.io/crates/console-subscriber

## getting started

to use the console to monitor and debug a program, it must be instrumented to
emit the data the console consumes. then, the `tokio-console` CLI application
can be used to connect to the application and monitor its operation.

### instrumenting the application

before the console can connect to an application, it must first be instrumented
to record `tokio-console` telemetry. the easiest way  to do this is [using the
`console-subscriber` crate][subscriber].

`console-subscriber` requires that the application's async runtime (or runtimes)
emit [`tracing`] data in a format that the console can record. for programs that
use the [Tokio] runtime, this means that:

- Tokio's [unstable features][unstable] must be enabled. see [the `console-subscriber`
  documentation][unstable] for details.
- a [compatible Tokio version][versions] must be used. Tokio v1.0 or greater is required
  to use the console, and some features are only available in later versions.
  see [the `console-subscriber` documentation][versions] for details.

[`tracing`]: https://crates.io/crates/tracing
[unstable]: https://docs.rs/console-subscriber/0.1/console_subscriber/#enabling-tokio-instrumentation
[versions]: https://docs.rs/console-subscriber/0.1/console_subscriber/#required-tokio-versions

### using the console

once the application is instrumented, install the console CLI using

```shell
cargo install --locked tokio-console
```

running `tokio-console` without any arguments will connect to an application on
localhost listening on the default port, port 6669:

```shell
tokio-console
```

if the application is not running locally, or was configured to listen on a
different port, the console will also accept a target address as a command-like
argument:

```shell
tokio-console http://192.168.0.42:9090
```

a DNS name can also be provided as the target address:

```shell
tokio-console http://my.instrumented.application.local:6669
```

see [here][cli-ref] for a complete list of all command-line arguments.

tokio console has a number of different views:

* [tasks list](#tasks-list)
* [task details](#task-details)
* [resources list](#resources-list)
* [resource details](#resource-details)

#### running the console on windows

the console uses the UTF-8 character set to display graphs and other visual
features in the terminal. in order to display this rich terminal UI on Windows,
it's necessary to use a UTF-8-enabled terminal emulator, such as the new
[Windows Terminal](https://learn.microsoft.com/en-us/windows/terminal/install).

if you're using a terminal that supports UTF-8, make sure to explicitly call
tokio-console with the UTF-8 language flag set:

```shell
tokio-console --lang en_US.UTF-8
```

### tasks list

when the console CLI is launched, it displays a list of all [asynchronous tasks]
in the program:

![tasks list](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tokio-console-0.1.8/tasks_list.png)

tasks are displayed in a table.

* `Warn` - the number of warnings active for the task.
* `ID` - the ID of the task. this is the same as the value returned by the unstable [`tokio::task::Id`](https://docs.rs/tokio/latest/tokio/task/struct.Id.html) API (see documentation for details).
* `State` - the state of the task.
  * `RUNNING`/▶ - task is currently being polled.
  * `IDLE`/⏸ - task is waiting on some resource.
  * `SCHED`/⏫ - task is scheduled (it has been woken but not yet polled).
  * `DONE`/⏹ - task has completed.
* `Name` - the name of the task, which can be set when spawning a task using the unstable [`tokio::task::Builder::name()`](https://docs.rs/tokio/latest/tokio/task/struct.Builder.html#method.name) API.
* `Total` - duration the task has been alive (sum of Busy, Sched, and Idle).
* `Busy` - total duration for which the task has been actively executing.
* `Sched` - total duration for which the task has been scheduled to be polled by the runtime.
* `Idle` - total duration for which the task has been idle (waiting to be woken).
* `Polls` - number of times the task has been polled.
* `Target` - the target of the span used to record the task.
  * `tokio::task` - async task.
  * `tokio::task::blocking` - a blocking task (created with [tokio::task::spawn_blocking](https://docs.rs/tokio/latest/tokio/task/fn.spawn_blocking.html)).
* `Location` - the source code location where the task was spawned from.
* `Fields` - additional fields on the task span.
  * `kind` - may be `task` (for async tasks) or `blocking` (for blocking tasks).
  * `fn` - function signature for blocking tasks. Async tasks don't record this field, as it is generally very large when using `async`/`await`.

using the <kbd>&#8593;</kbd> and <kbd>&#8595;</kbd> arrow keys, an individual task can be highlighted.
pressing<kbd>enter</kbd> while a task is highlighted displays details about that
task.

### task details

this view shows details about a specific task:

![task details](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tokio-console-0.1.8/task_details.png)

the task details view includes percentiles and a visual histogram of the polling (busy) times
and scheduled times.

pressing the <kbd>escape</kbd> key returns to the task list.

### resources list

the <kbd>r</kbd> key switches from the list of tasks to a list of [resources],
such as synchronization primitives, I/O resources, et cetera:

![resource list](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tokio-console-0.1.8/resources_list.png)

resources are displayed in a table similar to the task list.

* `ID` - the ID of the resource. this is a display ID as there is no internal resource ID to reference.
* `Parent` - the ID of the parent resource if it exists.
* `Kind` - the resource kind, this is a high level grouping of resources.
  * `Sync` - synchronization resources from [`tokio::sync`](https://docs.rs/tokio/latest/tokio/sync/index.html) such as [`Mutex`](https://docs.rs/tokio/latest/tokio/sync/struct.Mutex.html).
  * `Timer` - timer resources from [`tokio::time`](https://docs.rs/tokio/latest/tokio/time/index.html) such as [`Sleep`](https://docs.rs/tokio/latest/tokio/time/struct.Sleep.html).
* `Total` - total duration that this resource has been alive.
* `Target` - the module path of the resource type.
* `Type` - the specific type of the resource, possible values depend on the resources instrumented in Tokio, which may vary between versions.
* `Vis` - the visibility of the resource.
  * `INT`/🔒 - internal, this resource is only used by other resources.
  * `PUB`/✅ - public, available in the public Tokio API.
* `Location` - the source code location where the resource was created.
* `Attributes` - additional resource-dependent attributes, for example a resource of type `Sleep` record the `duration` of the sleep.

pressing the <kbd>t</kbd> key switches the view back to the task list.

like the task list view, the resource list view can be navigated using the
<kbd>&#8593;</kbd> and <kbd>&#8595;</kbd> arrow keys. pressing <kbd>enter</kbd>
while a resource is highlighted displays details about that resource.

### resource details

![resource details --- sleep](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tokio-console-0.1.8/resource_details_sleep.png)

the resource details view lists the tasks currently waiting on that resource.
this may be a single task, as in the [`tokio::time::Sleep`] above, or
a large number of tasks, such as this private `tokio::sync::batch_semaphore::Semaphore`:

![resource details --- semaphore](https://raw.githubusercontent.com/tokio-rs/console/main/assets/tokio-console-0.1.8/resource_details_semaphore.png)

the resource details view includes a table of async ops belonging to the resource.

* `ID` - the ID of the async op. this is a display ID similar to those recorded for resources.
* `Parent` - the ID of the parent async op, if it exists.
* `Task` - the ID and name of the task which performed this async op.
* `Source` - the method where the async op is being called from.
* `Total` - total duration for which the async op has been alive (sum of Busy and Idle, as an async op has no scheduled state).
* `Busy` - total duration for which the async op has been busy (its future is actively being polled).
* `Idle` - total duration for which the async op has been idle (the future exists but is not being polled).
* `Polls` - number of times the async op has been polled.
* `Attributes` - additional attributes from the async op. these will vary based on the type of the async op.

like the task details view, pressing the <kbd>escape</kbd> key while viewing a resource's details
returns to the resource list.

a configuration file (`console.toml`) can be used to configure the console's
behavior. see [the documentation][cfg-ref] for details.

[`tokio-console`]: https://github.com/tokio-rs/console
[Tokio]: https://tokio.rs
[asynchronous tasks]: https://tokio.rs/tokio/tutorial/spawning#tasks
[resources]: https://tokio.rs/tokio/tutorial/async#async-fn-as-a-future
[`tokio::sync::oneshot`]: https://docs.rs/tokio/latest/tokio/sync/oneshot/index.html
[`tokio::sync::Semaphore`]: https://docs.rs/tokio/latest/tokio/sync/struct.Semaphore.html
[cli-ref]: https://docs.rs/tokio-console/latest/tokio_console/config_reference/index.html#command-line-arguments
[cfg-ref]: https://docs.rs/tokio-console/latest/tokio_console/config_reference/index.html#configuration-file

## getting help

first, see if the answer to your question can be found in the
[API documentation]. if the answer is not there, there is an active community in
the [Tokio Discord server][discord-url]. we would be happy to try to answer your
question. you can also ask your question on [the discussions page][discussions].

[API documentation]: https://docs.rs/tokio-console
[discussions]: https://github.com/tokio-rs/console/discussions
[discord-url]: https://discord.gg/tokio

## contributing

&#x1f388; thanks for your help improving the project! we are so happy to have
you! we have a [contributing guide][guide] to help you get involved in the Tokio
console project.

[guide]: https://github.com/tokio-rs/console/blob/main/CONTRIBUTING.md

## supported Rust versions

the Tokio console is built against the latest stable release. the minimum
supported version is 1.74. the current Tokio console version is not guaranteed
to build on Rust versions earlier than the minimum supported version.

## license

this project is licensed under the [MIT license].

[MIT license]: https://github.com/tokio-rs/console/blob/main/LICENSE

### contribution

unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Tokio by you, shall be licensed as MIT, without any additional
terms or conditions.
