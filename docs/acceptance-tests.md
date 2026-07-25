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
- a differential, rep-4, time-redundant header (no fragile coherent header), so
  frames are detected reliably;
- a 384-sample cyclic prefix with a centred FFT window to absorb speaker/room
  delay spread;
- carrier-major interleaving with 4x frequency diversity, so a notch in the
  handset response cannot hit every copy of a bit.

## Speed vs reliability sweep (why rep-4)

A hardware sweep (`ofdm-probe`, `OFDM_VARIANT`, 200-byte frames, 5 per run at
65% volume) measured how many frames fully decode per variant:

| variant | rel. speed | recovered / 5 |
| --- | ---: | ---: |
| DQPSK rep-2 (`dqpsk2`) | 4x | 0 |
| DQPSK rep-3 (`dqpsk3`) | 2.6x | 0 |
| DQPSK rep-4 (`dqpsk4`) | 2x | 2 |
| DBPSK rep-2 (`dbpsk2`) | 2x | 1 |
| DBPSK rep-3 (`dbpsk3`) | 1.3x | 3 |
| DBPSK rep-4 (default) | 1x | 3-5 |

The `ofdm-probe profile` command dumps the per-carrier channel magnitude. On the
MacBook Air speaker the response has **scattered deep notches — about 30% of the
carriers are effectively dead**, at frequencies that change if the band is
shifted (`OFDM_BAND`). Because the notches are wide and move, only 4x frequency
diversity keeps enough good copies of every bit; anything faster (less diversity
or the tighter QPSK decision regions) drops below the outer-FEC threshold and
transfers fail. Shrinking the cyclic prefix (192/128) or shifting the band did
not give a robust, device-agnostic speedup.

Conclusion: **rep-4 differential DBPSK is the fastest variant that transfers
reliably over the built-in laptop speaker.** Going faster needs a device-specific
carrier mask (a fixed notch profile per model) or an acoustic ACK/ARQ channel,
neither of which is device-agnostic.

Reproduce the ground-truth probe and channel profile:

```bash
# terminal 1
cargo run -p sonic-share-cli --bin record -- capture.wav 24
# terminal 2 (within the window)
cargo run -p sonic-share-cli --bin ofdm-probe -- send
# then
cargo run -p sonic-share-cli --bin ofdm-probe -- check capture.wav
cargo run -p sonic-share-cli --bin ofdm-probe -- profile capture.wav
```
