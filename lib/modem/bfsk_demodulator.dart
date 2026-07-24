import 'dart:typed_data';

import '../protocol/packet_decoder.dart';
import '../protocol/repetition_code.dart';
import '../shared/utils/bit_utils.dart';
import 'goertzel.dart';
import 'modem_config.dart';
import 'synchronizer.dart';

/// Per-symbol demodulation output.
class SymbolResult {
  const SymbolResult({
    required this.bit,
    required this.confidence,
    required this.energy0,
    required this.energy1,
  });

  final int bit;
  final double confidence;
  final double energy0;
  final double energy1;
}

/// Diagnostic snapshot produced by a decode attempt, surfaced to the UI.
class DemodDiagnostics {
  const DemodDiagnostics({
    this.energy0 = 0,
    this.energy1 = 0,
    this.noiseFloor = 0,
    this.snr = 0,
    this.confidence = 0,
    this.offset = -1,
    this.bitCount = 0,
    this.packetCount = 0,
    this.correctedBits = 0,
    this.preambleScore = 0,
    this.crcOk = false,
  });

  final double energy0;
  final double energy1;
  final double noiseFloor;
  final double snr;
  final double confidence;
  final int offset;
  final int bitCount;
  final int packetCount;
  final int correctedBits;
  final double preambleScore;
  final bool crcOk;
}

/// Result of decoding a single frame (one preamble + its data burst).
class FrameDecodeResult {
  const FrameDecodeResult({
    required this.status,
    this.packetResult,
    this.diagnostics = const DemodDiagnostics(),
    this.consumedUntil = 0,
  });

  final PacketDecodeStatus status;
  final PacketDecodeResult? packetResult;
  final DemodDiagnostics diagnostics;

  /// Sample index up to which the input has been consumed.
  final int consumedUntil;

  static const FrameDecodeResult noPreamble = FrameDecodeResult(
    status: PacketDecodeStatus.noSync,
  );
}

/// Demodulates BFSK PCM audio back into bits and packets using the Goertzel
/// algorithm for energy estimation at the two carrier frequencies.
class BfskDemodulator {
  BfskDemodulator(this.config)
      : _repetition = RepetitionCode(config.repetitionFactor),
        _sync = Synchronizer(config),
        _g0 = Goertzel(
          targetFrequency: config.freq0.toDouble(),
          sampleRate: config.sampleRate,
          blockSize: config.samplesPerSymbol,
        ),
        _g1 = Goertzel(
          targetFrequency: config.freq1.toDouble(),
          sampleRate: config.sampleRate,
          blockSize: config.samplesPerSymbol,
        );

  final ModemConfig config;
  final RepetitionCode _repetition;
  final Synchronizer _sync;
  final Goertzel _g0;
  final Goertzel _g1;

  static const PacketDecoder _packetDecoder = PacketDecoder();

  /// Demodulates a single symbol located at [start].
  SymbolResult demodulateSymbol(Float64List samples, int start) {
    final e0 = _g0.energy(samples, start: start);
    final e1 = _g1.energy(samples, start: start);
    final total = e0 + e1;
    final bit = e1 > e0 ? 1 : 0;
    final confidence = total > 0 ? (e1 - e0).abs() / total : 0.0;
    return SymbolResult(
      bit: bit,
      confidence: confidence,
      energy0: e0,
      energy1: e1,
    );
  }

  /// Attempts to decode a single frame from [samples], starting the preamble
  /// search at [searchStart].
  FrameDecodeResult decodeFrame(
    Float64List samples, {
    int searchStart = 0,
    double minPreambleScore = 0.35,
  }) {
    final s = config.samplesPerSymbol;
    final preamble = _sync.search(
      samples,
      searchStart: searchStart,
      minScore: minPreambleScore,
    );

    if (!preamble.found) {
      return FrameDecodeResult(
        status: PacketDecodeStatus.noSync,
        consumedUntil: searchStart,
        diagnostics: DemodDiagnostics(preambleScore: preamble.score),
      );
    }

    // Establish a presence threshold from the preamble's own energy so the
    // decoder adapts to the current playback volume.
    double preambleEnergy = 0;
    int preambleSymbols = 0;
    for (int k = 0; k < config.preambleBits; k++) {
      final pos = preamble.startSample + k * s;
      if (pos + s > samples.length) break;
      final e0 = _g0.energy(samples, start: pos);
      final e1 = _g1.energy(samples, start: pos);
      preambleEnergy += e0 + e1;
      preambleSymbols++;
    }
    final avgPreambleEnergy =
        preambleSymbols > 0 ? preambleEnergy / preambleSymbols : 0.0;
    final presenceThreshold = avgPreambleEnergy * 0.12;

    final dataStart = preamble.startSample + config.preambleBits * s;

    final bits = <int>[];
    final confidences = <double>[];
    double lastE0 = 0;
    double lastE1 = 0;
    double confidenceSum = 0;
    int silenceRun = 0;
    int pos = dataStart;
    while (pos + s <= samples.length) {
      final sym = demodulateSymbol(samples, pos);
      final total = sym.energy0 + sym.energy1;
      if (total < presenceThreshold) {
        silenceRun++;
        // Two consecutive silent symbols mark the end of the burst.
        if (silenceRun >= 2) break;
      } else {
        silenceRun = 0;
      }
      bits.add(sym.bit);
      confidences.add(sym.confidence);
      confidenceSum += sym.confidence;
      lastE0 = sym.energy0;
      lastE1 = sym.energy1;
      pos += s;
    }

    final consumedUntil = pos;

    final decoded = _repetition.decodeSoft(bits, confidences);
    final bytes = BitUtils.bitsToBytes(decoded.bits);
    final packetResult = _packetDecoder.decode(bytes);

    final avgConfidence = bits.isEmpty ? 0.0 : confidenceSum / bits.length;
    final noiseFloor = presenceThreshold;
    final snr = noiseFloor > 0 ? (lastE0 + lastE1) / noiseFloor : 0.0;

    final diagnostics = DemodDiagnostics(
      energy0: lastE0,
      energy1: lastE1,
      noiseFloor: noiseFloor,
      snr: snr,
      confidence: avgConfidence,
      offset: preamble.startSample,
      bitCount: decoded.bits.length,
      packetCount: packetResult.isOk ? 1 : 0,
      correctedBits: decoded.correctedBits,
      preambleScore: preamble.score,
      crcOk: packetResult.isOk,
    );

    return FrameDecodeResult(
      status: packetResult.status,
      packetResult: packetResult,
      diagnostics: diagnostics,
      consumedUntil: consumedUntil,
    );
  }
}
