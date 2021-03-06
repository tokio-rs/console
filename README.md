# console

Tokio provides an instrumentation API using `tracing` as well as a number of instrumentation points built into Tokio itself and the Tokio ecosystem. The goal of the project is to implement a library for aggregation, metrics of said instrumentation points and a console-based UI that connects to the process, allowing users to quickly visualize, browse and debug the data.

Because processes can encode structured and typed business logic with instrumentation points based on `tracing`, a domain-specific debugger built upon those can provide powerful, ad hoc tooling, e.g. filtering events by connection id, execution context etcetera. As instrumentation points of underlying libraries are collected as well, it is easy to observe their behaviour and interaction. This is an eminent advantage over traditional debuggers, where the user instead observes the implementation.

## GSoC
This project has been part of Google summer of code. For more information, see [gsoc.md](https://github.com/tokio-rs/console/blob/master/gsoc.md).