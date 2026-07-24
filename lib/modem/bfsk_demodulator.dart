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
        _guard = config.symbolGuardSamples,
        _g0 = Goertzel(
          targetFrequency: config.freq0.toDouble(),
          sampleRate: config.sampleRate,
          blockSize: config.coreSymbolSamples,
        ),
        _g1 = Goertzel(
          targetFrequency: config.freq1.toDouble(),
          sampleRate: config.sampleRate,
          blockSize: config.coreSymbolSamples,
        );

  final ModemConfig config;
  final RepetitionCode _repetition;
  final Synchronizer _sync;
  final int _guard;
  final Goertzel _g0;
  final Goertzel _g1;

  static const PacketDecoder _packetDecoder = PacketDecoder();

  /// Demodulates a single symbol whose boundary starts at [start]. Only the
  /// steady-frequency core of the symbol (excluding the transition edges) is
  /// integrated so the smoothed boundaries do not blur the decision.
  SymbolResult demodulateSymbol(Float64List samples, int start) {
    final core = start + _guard;
    final e0 = _g0.energy(samples, start: core);
    final e1 = _g1.energy(samples, start: core);
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
    // Bound the preamble search to a window just large enough to contain a
    // whole preamble plus slack. Without this bound the synchronizer takes the
    // global maximum over the ENTIRE remaining buffer on every call, which for
    // a long multi-packet transmission is O(packets × bufferLength) and hangs
    // the decoder isolate. The next packet's preamble always begins near
    // [searchStart], so a two-preamble window is more than enough.
    final windowEnd = searchStart + config.preambleBits * s * 2;
    final searchEnd = windowEnd < samples.length ? windowEnd : samples.length;
    final preamble = _sync.search(
      samples,
      searchStart: searchStart,
      searchEnd: searchEnd,
      minScore: minPreambleScore,
    );

    if (!preamble.found) {
      return FrameDecodeResult(
        status: PacketDecodeStatus.noSync,
        consumedUntil: searchStart,
        diagnostics: DemodDiagnostics(preambleScore: preamble.score),
      );
    }

    final rep = config.repetitionFactor;
    final dataStart = preamble.startSample + config.preambleBits * s;
    final availableSymbols = (samples.length - dataStart) ~/ s;

    // Number of coded symbols the fixed header (sync+ver+type+seq+len = 7 bytes)
    // occupies. This tells us how much to read before we know the full length.
    const headerBytes = 2 + 5;
    final headerSymbols = headerBytes * 8 * rep;

    if (availableSymbols < headerSymbols) {
      return _incompleteResult(preamble, samples.length);
    }

    // Read just the header to discover the payload length.
    final head = _demod(samples, dataStart, headerSymbols);
    final headerDecoded = _repetition.decodeSoft(head.bits, head.confs);
    final headerBytesOut = BitUtils.bitsToBytes(headerDecoded.bits);
    final syncOk = headerBytesOut.length >= 2 &&
        ((headerBytesOut[0] << 8) | headerBytesOut[1]) == ModemConfig.syncWord;
    final length = headerBytesOut.length >= 7
        ? ((headerBytesOut[5] << 8) | headerBytesOut[6])
        : 0xFFFF;

    // Misaligned preamble or garbage header: skip past it and keep searching.
    if (!syncOk || length > config.maxPayloadLength) {
      return FrameDecodeResult(
        status: PacketDecodeStatus.crcError,
        diagnostics: DemodDiagnostics(
          offset: preamble.startSample,
          preambleScore: preamble.score,
          bitCount: headerDecoded.bits.length,
        ),
        consumedUntil: dataStart + headerSymbols * s,
      );
    }

    final totalBytes = 2 + 5 + length + 2;
    final totalSymbols = totalBytes * 8 * rep;

    // Length-aware: read exactly the whole frame, tolerating any momentary
    // energy dips in the middle (from clicks, AGC or noise suppression) instead
    // of stopping at the first quiet symbol.
    if (availableSymbols < totalSymbols) {
      return _incompleteResult(preamble, samples.length);
    }

    final frame = _demod(samples, dataStart, totalSymbols);
    final decoded = _repetition.decodeSoft(frame.bits, frame.confs);
    final bytes = BitUtils.bitsToBytes(decoded.bits);
    final packetResult = _packetDecoder.decode(bytes);

    final consumedUntil = dataStart + totalSymbols * s;
    final avgConfidence =
        frame.bits.isEmpty ? 0.0 : frame.confSum / frame.bits.length;
    final noiseFloor = _preambleNoiseFloor(samples, preamble.startSample);
    final signalEnergy = frame.lastE0 + frame.lastE1;
    final snr = noiseFloor > 0 ? signalEnergy / noiseFloor : 0.0;

    final diagnostics = DemodDiagnostics(
      energy0: frame.lastE0,
      energy1: frame.lastE1,
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

  /// Demodulates [count] symbols starting at [dataStart], returning the hard
  /// bits, per-symbol confidences and energy statistics.
  ({
    List<int> bits,
    List<double> confs,
    double lastE0,
    double lastE1,
    double confSum,
  }) _demod(Float64List samples, int dataStart, int count) {
    final s = config.samplesPerSymbol;
    final bits = <int>[];
    final confs = <double>[];
    double lastE0 = 0;
    double lastE1 = 0;
    double confSum = 0;
    for (int i = 0; i < count; i++) {
      final pos = dataStart + i * s;
      if (pos + s > samples.length) break;
      final sym = demodulateSymbol(samples, pos);
      bits.add(sym.bit);
      confs.add(sym.confidence);
      confSum += sym.confidence;
      lastE0 = sym.energy0;
      lastE1 = sym.energy1;
    }
    return (
      bits: bits,
      confs: confs,
      lastE0: lastE0,
      lastE1: lastE1,
      confSum: confSum,
    );
  }

  double _preambleNoiseFloor(Float64List samples, int start) {
    final s = config.samplesPerSymbol;
    double energy = 0;
    int n = 0;
    for (int k = 0; k < config.preambleBits; k++) {
      final pos = start + k * s;
      if (pos + s > samples.length) break;
      final sym = demodulateSymbol(samples, pos);
      energy += sym.energy0 + sym.energy1;
      n++;
    }
    return n > 0 ? (energy / n) * 0.12 : 0.0;
  }

  FrameDecodeResult _incompleteResult(
    PreambleSearchResult preamble,
    int samplesLength,
  ) {
    return FrameDecodeResult(
      status: PacketDecodeStatus.needMoreData,
      diagnostics: DemodDiagnostics(
        offset: preamble.startSample,
        preambleScore: preamble.score,
      ),
      consumedUntil: samplesLength,
    );
  }
}
