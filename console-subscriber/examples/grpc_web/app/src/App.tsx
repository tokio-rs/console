import "./App.css";
import { createGrpcWebTransport } from "@connectrpc/connect-web";
import { createPromiseClient } from "@connectrpc/connect";
import { Instrument } from "./gen/instrument_connect";
import { InstrumentRequest } from "./gen/instrument_pb";

function App() {
  const transport = createGrpcWebTransport({
    baseUrl: "http://localhost:9999",
  });

  const client = createPromiseClient(Instrument, transport);

  (async () => {
    try {
      const updateStream = client.watchUpdates(new InstrumentRequest());

      for await (const value of updateStream) {
        console.log(value);
      }
    } catch (err) {
      console.error(err);
    }
  })();

  return (
    <>
      <h1>gRPC-Web Example</h1>
      <p>Open the console to see the updates</p>
    </>
  );
}

export default App;
