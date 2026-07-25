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

Send/listen/decode use the reliable hybrid OFDM profile by default (differential
DBPSK with rep-2 frequency diversity), which is verified over a real MacBook
speaker/microphone path. Use `--qam16` for the faster clean-channel OFDM,
`--fsk` for legacy Fast FSK, or `--robust` for Robust FSK. Both peers must use
the same profile and `--lane` value.

`ofdm-probe send|check <wav>` measures the true OFDM bit error rate for tuning.
