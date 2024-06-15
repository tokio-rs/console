# gRPC-web example

this app provides an example of using the gRPC-web library to facilitate communication between a web browser and a gRPC server.

## prerequisites

ensure you have the following installed on your system:

- [Node.js](https://nodejs.org/en/download/) (version 20.10.0 or higher)
- [npm](https://www.npmjs.com/get-npm) (version 10.2.3 or higher)

## getting started

follow these steps to get the application up and running:

1. **install dependencies:** navigate to the `console-subscriber/examples/grpc_web/app` directory and install all necessary dependencies:

    ```sh
    npm install
    ```

2. **start the gRPC-web server:** in the console-subscriber directory, start the server:

    ```sh
    cargo run --example grpc_web --features grpc-web
    ```

3. **start the web application:** in the `console-subscriber/examples/grpc_web/app` directory, start the web application:

    ```sh
    npm run dev
    ```

4. **view the application:** open a web browser and navigate to `http://localhost:5173`. you can view the output in the developer console.

## understanding the code

this example leverages the [connect-es] library to enable communication with the gRPC server from a web browser. the client code can be found in the `console-subscriber/examples/grpc_web/app/src/app.tsx` file.

the [buf] tool is used to generate the gRPC code. you can generate the code using the following command:

```sh
npm run gen
```

for more information about the connect-es library, refer to the [connect-es documentation].

[connect-es]: https://github.com/connectrpc/connect-es
[buf]: https://buf.build/
[connect-es documentation]: https://connectrpc.com/docs/web/getting-started
