# Socket protocol fixtures

One message per line, exactly as it travels on the socket
(`docs/socket-api.md`, protocol version 1). Every client checks itself
against these files: the Rust types in `keyvo_core::socket`
(`crates/keyvo-core/tests/socket.rs`) parse every line and write it back to
the same JSON, and the TypeScript client (`app/src/lib/protocol.test.ts`)
runs the same check. `keyvo-client` (`crates/keyvo-client/tests/`) uses the
response and event lines as the daemon's side of its exchanges, and the
app's Home tests (`app/src/lib/home.test.ts`, `app/src/Home.test.ts`) feed
the event lines to a fake daemon.

| File | Content |
|---|---|
| `requests.ndjson` | one request per command, every field written out |
| `responses.ndjson` | the answers to them: `status` with and without a Keypad, `subscribe`, a dry-run and a real `press`, `reload` and its failure, `notify_context`, and the three request errors |
| `events.ndjson` | every event class; the `trace` lines are the records of `crates/keyvo-core/tests/fixtures/trace/records.ndjson` |

The lines are written by hand from the protocol, not captured. Key order in
an object carries no meaning; a client compares parsed values, not text.

A client ignores unknown fields in responses and events at any depth, so a
later daemon may add fields within version 1; the Rust check adds a field
to every object of every response and event line and expects the same
message. The daemon rejects unknown fields in requests.
