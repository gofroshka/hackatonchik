# Acoustic acceptance tests

## Quick regression

Run from the repository root:

```bash
cargo run -p sonic-share-cli --bin bench-assets -- assets
```

This verifies complete transfer/FEC/SHA reconstruction for every file in
`assets/`, sends representative packets through Fast FSK, and sends every packet
through hybrid OFDM QPSK without waiting for real-time playback. It also checks
the 20 KB OFDM airtime targets.

Current reference result:

| Asset | Packets | FSK Fast | FSK Robust | OFDM safe | OFDM fast |
| --- | ---: | ---: | ---: | ---: | ---: |
| `1kb.txt` | 10 | 48.3 s | 190.5 s | 18.7 s | 8.1 s |
| `2kb.txt` | 10 | 48.3 s | 190.5 s | 18.7 s | 8.1 s |
| `images.jpeg` | 172 | 968.7 s | 3830.8 s | 337.2 s | 108.7 s |

Text assets compress heavily and therefore have equal packet counts. JPEG is
already compressed and represents the worst supplied asset.

`OFDM safe` is the default (differential DBPSK with rep-2 frequency diversity),
which is the mode verified to survive a real acoustic path. `OFDM fast`
(`--qam16`, differential QPSK) is roughly 3x faster but needs a cleaner channel.
The thresholds in `bench-assets` are regression guards, not the airtime target.

## Test 4: distance and high noise

Use Robust on both devices:

```bash
# receiver
cargo run -p sonic-share-cli --bin listen -- --robust --lane 0

# sender
cargo run -p sonic-share-cli --bin send -- file assets/1kb.txt --robust --lane 0
```

Record the following in the demo video:

- measured speaker-to-microphone distance;
- ambient noise in dBA measured near the receiver;
- device models and volume percentage;
- successful SHA-256 verification shown by the receiver.

The software simulation includes attenuation, additive noise, clipping,
multipath echo and 1500 ppm clock drift. It cannot truthfully convert gain into
metres, so the maximum supported distance must be established and stated from
this physical test.

## Test 5: simultaneous pairs within 1 metre

Assign a different lane to each pair:

```bash
# pair A
cargo run -p sonic-share-cli --bin listen -- --lane 0
cargo run -p sonic-share-cli --bin send -- file assets/1kb.txt --lane 0

# pair B
cargo run -p sonic-share-cli --bin listen -- --lane 1
cargo run -p sonic-share-cli --bin send -- file assets/2kb.txt --lane 1
```

Start both senders together. Place every sender and its receiver no farther
than 1 metre apart. Both receivers must finish with SHA-256 verification.
Transmitters sharing the same lane are expected to collide; lane assignment is
required for simultaneous independent pairs.

## MacBook Air loopback result

Physical loopback was run on the built-in MacBook Air speakers and microphone at
48 kHz. The default `OFDM safe` mode transfers files end-to-end with byte-exact
SHA-256 verification:

- `assets/1kb.txt` (1 group, 10 packets): received and SHA-256 verified.
- 2500-byte incompressible file (2 groups, 27 packets): both groups
  reconstructed and SHA-256 verified.

What made OFDM work over the real acoustic path:

- differential DBPSK across time, which cancels the static channel phase, a
  fixed sample-timing offset and constant carrier rotation without a coherent
  channel estimate;
- a 384-sample cyclic prefix with a centred FFT window to absorb speaker/room
  delay spread;
- carrier-major interleaving with rep-2 frequency diversity, so a notch in the
  handset response (a handful of dead subcarriers) cannot corrupt the payload.

Ground-truth measurement (`ofdm-probe`): full-rate DBPSK saw ~2-5% raw bit error
concentrated on a few notch subcarriers; rep-2 diversity dropped this below 0.5%
and every recorded frame decoded. The `--qam16` fast mode has no diversity and
still needs a clean channel; the safe default is the accepted physical result.

Reproduce the ground-truth BER probe:

```bash
# terminal 1
cargo run -p sonic-share-cli --bin record -- capture.wav 12
# terminal 2 (within the 12 s window)
cargo run -p sonic-share-cli --bin ofdm-probe -- send
# then
cargo run -p sonic-share-cli --bin ofdm-probe -- check capture.wav
```
