import 'dart:math' as math;

/// Central configuration for the acoustic BFSK modem.
///
/// All tunable DSP and protocol parameters live here so that they can be
/// adjusted from a single place. The defaults are chosen to be robust in a
/// quiet room using the built-in speaker and microphone of typical Android
/// phones.
class ModemConfig {
  const ModemConfig({
    // 48 kHz is the native capture/playback rate on virtually all Android and
    // iOS devices, so requesting it avoids silent resampling that would desync
    // the demodulator. The physical BFSK frequencies are independent of it.
    this.sampleRate = 48000,
    this.freq0 = 1500,
    this.freq1 = 2500,
    this.symbolDurationMs = 40,
    this.amplitude = 0.7,
    this.preambleBits = 64,
    this.repetitionFactor = 3,
    this.frequencyTransitionMs = 3.0,
    this.bandpassLow = 1100,
    this.bandpassHigh = 2900,
    this.leadingSilenceMs = 120,
    this.trailingSilenceMs = 120,
    this.rampSamples = 24,
    this.maxPayloadLength = 128,
    this.confidenceThreshold = 0.12,
    this.minSymbolEnergy = 1e6,
    this.snrThreshold = 3.0,
  });

  /// Sampling frequency in Hz. Must match on both playback and capture.
  final int sampleRate;

  /// Frequency (Hz) that encodes a logical `0` bit.
  final int freq0;

  /// Frequency (Hz) that encodes a logical `1` bit.
  final int freq1;

  /// Duration of a single BFSK symbol (one bit) in milliseconds.
  final int symbolDurationMs;

  /// Duration (ms) of the smooth raised-cosine frequency transition applied at
  /// each symbol boundary. This keeps the instantaneous frequency continuous
  /// (GFSK-style) so there are no broadband clicks between bits.
  final double frequencyTransitionMs;

  /// Output amplitude in range 0.0 .. 1.0. Kept below 1.0 to avoid clipping.
  final double amplitude;

  /// Number of alternating (1010..) preamble bits used for detection and
  /// symbol-timing synchronization. NOT repetition coded.
  final int preambleBits;

  /// Repetition code factor. Each payload bit is transmitted this many times
  /// and recovered on the receiver with majority voting.
  final int repetitionFactor;

  /// Lower cutoff (Hz) of the receive band-pass filter.
  final double bandpassLow;

  /// Upper cutoff (Hz) of the receive band-pass filter.
  final double bandpassHigh;

  /// Silence padding before the preamble in milliseconds.
  final int leadingSilenceMs;

  /// Silence padding after the packet in milliseconds.
  final int trailingSilenceMs;

  /// Number of samples used for the raised-cosine ramp at symbol edges to
  /// avoid audible clicks and spectral splatter.
  final int rampSamples;

  /// Maximum payload length (bytes) for a single packet.
  final int maxPayloadLength;

  /// Minimum per-symbol confidence below which a symbol is considered weak.
  final double confidenceThreshold;

  /// Minimum Goertzel energy for a symbol to be considered "signal present".
  final double minSymbolEnergy;

  /// Minimum signal-to-noise ratio (linear) required to start decoding.
  final double snrThreshold;

  /// Number of PCM samples that make up one symbol.
  int get samplesPerSymbol =>
      (sampleRate * symbolDurationMs / 1000).round();

  /// Total width (samples) of the frequency transition centered on each symbol
  /// boundary. Always even so it splits cleanly across the boundary.
  int get transitionSamples {
    final raw = (sampleRate * frequencyTransitionMs / 1000).round();
    return raw.isOdd ? raw + 1 : raw;
  }

  /// Samples skipped at each symbol edge during demodulation so the Goertzel
  /// integrates only the clean, steady-frequency core of the symbol.
  int get symbolGuardSamples => transitionSamples ~/ 2;

  /// Number of steady-state samples in the middle of a symbol used for
  /// Goertzel energy estimation.
  int get coreSymbolSamples {
    final core = samplesPerSymbol - 2 * symbolGuardSamples;
    return core < 1 ? samplesPerSymbol : core;
  }

  /// Sync word marking the start of a packet (after the preamble).
  static const int syncWord = 0xDDAA;

  /// Protocol version byte.
  static const int protocolVersion = 1;

  /// Total bit rate (payload independent) in bits per second before coding.
  double get rawBitRate => 1000.0 / symbolDurationMs;

  /// Effective payload bit rate after repetition coding.
  double get effectiveBitRate => rawBitRate / repetitionFactor;

  /// Angular frequency step per sample for [freq0].
  double get omega0 => 2 * math.pi * freq0 / sampleRate;

  /// Angular frequency step per sample for [freq1].
  double get omega1 => 2 * math.pi * freq1 / sampleRate;

  ModemConfig copyWith({
    int? sampleRate,
    int? freq0,
    int? freq1,
    int? symbolDurationMs,
    double? amplitude,
    int? preambleBits,
    int? repetitionFactor,
  }) {
    return ModemConfig(
      sampleRate: sampleRate ?? this.sampleRate,
      freq0: freq0 ?? this.freq0,
      freq1: freq1 ?? this.freq1,
      symbolDurationMs: symbolDurationMs ?? this.symbolDurationMs,
      amplitude: amplitude ?? this.amplitude,
      preambleBits: preambleBits ?? this.preambleBits,
      repetitionFactor: repetitionFactor ?? this.repetitionFactor,
    );
  }
}

/// Packet type identifiers used in the protocol header.
class PacketType {
  const PacketType._();

  /// A UTF-8 text payload (possibly a fragment of a larger message).
  static const int text = 0x01;
}
