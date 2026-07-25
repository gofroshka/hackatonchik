# Sonic Share CLI

Run the unified terminal interface:

```sh
cargo run -p sonic-share-cli --bin sonic-share
```

The TUI provides Send, Receive, and Chat tabs. Received files are written to
`received/`; chat messages use `text/x-sonic-chat; charset=utf-8` and remain only
in memory.

- `1`, `2`, `3` or `Tab`: switch modes.
- `Enter`: submit a path/message or start/stop receiving.
- `Esc`: leave input mode; `q` or `Esc` again exits.
- `r`: start/stop the listener from Chat when input mode is inactive.

The diagnostic binaries remain available:

```sh
cargo run -p sonic-share-cli --bin send -- file ./example.bin
cargo run -p sonic-share-cli --bin listen -- --output-dir received
cargo run -p sonic-share-cli --bin decode -- recording.wav
cargo run -p sonic-share-cli --bin record -- recording.wav 10
```
