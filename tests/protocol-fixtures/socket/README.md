# Socket protocol fixtures

One message per line, exactly as it travels on the socket
(`docs/socket-api.md`, protocol version 1). Every client checks itself
against these files: the Rust types in `keyvo_core::socket`
(`crates/keyvo-core/tests/socket.rs`) parse every line and write it back to
the same JSON, and the TypeScript client (`app/`, pre-alpha slice 14) runs
the same check.

| File | Content |
|---|---|
| `requests.ndjson` | one request per command, every field written out |
| `responses.ndjson` | the answers to them: `status` with and without a Keypad, `subscribe`, a dry-run and a real `press`, `reload` and its failure, `notify_context`, and the three request errors |
| `events.ndjson` | every event class; the `trace` lines are the records of `crates/keyvo-core/tests/fixtures/trace/records.ndjson` |

The lines are written by hand from the protocol, not captured. Key order in
an object carries no meaning; a client compares parsed values, not text.
