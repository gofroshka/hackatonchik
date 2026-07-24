import 'dart:typed_data';

import 'goertzel.dart';
import 'modem_config.dart';

/// Result of searching for the preamble within a sample buffer.
class PreambleSearchResult {
  const PreambleSearchResult({
    required this.found,
    required this.startSample,
    required this.score,
  });

  final bool found;

  /// Sample index where the first preamble symbol begins.
  final int startSample;

  /// Normalized correlation score (higher is a better match).
  final double score;

  static const PreambleSearchResult notFound = PreambleSearchResult(
    found: false,
    startSample: -1,
    score: 0,
  );
}

/// Locates the alternating preamble and recovers precise symbol timing.
///
/// The receiver must not assume that recording started exactly on a symbol
/// boundary, so several candidate offsets are evaluated and the one whose
/// energy best matches the expected alternating tone pattern is selected.
class Synchronizer {
  Synchronizer(this.config)
      : _guard = config.symbolGuardSamples,
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
  final int _guard;
  final Goertzel _g0;
  final Goertzel _g1;

  /// Searches [samples] in `[searchStart, searchEnd)` for the preamble.
  ///
  /// A coarse scan followed by a fine (per-sample) refinement finds the exact
  /// symbol boundary offset.
  PreambleSearchResult search(
    Float64List samples, {
    int searchStart = 0,
    int? searchEnd,
    double minScore = 0.35,
  }) {
    final s = config.samplesPerSymbol;
    final end = searchEnd ?? samples.length;
    final lastStart = end - config.preambleBits * s;
    if (lastStart <= searchStart) return PreambleSearchResult.notFound;

    final coarseStep = (s ~/ 4).clamp(1, s);
    int bestStart = -1;
    double bestScore = -1;

    // The alternating preamble is periodic and therefore correlates highly at
    // offsets shifted by an even number of symbols. We must take the GLOBAL
    // maximum (a full scan) — the true alignment always scores highest because
    // every preamble symbol lines up — rather than exiting early on the first
    // "good enough" (but shifted) peak.
    for (int start = searchStart; start <= lastStart; start += coarseStep) {
      final score = _preambleScore(samples, start);
      if (score > bestScore) {
        bestScore = score;
        bestStart = start;
      }
    }

    if (bestStart < 0) return PreambleSearchResult.notFound;

    // Fine refinement ±coarseStep around the coarse maximum.
    final refineLow = (bestStart - coarseStep).clamp(searchStart, lastStart);
    final refineHigh = (bestStart + coarseStep).clamp(searchStart, lastStart);
    for (int start = refineLow; start <= refineHigh; start++) {
      final score = _preambleScore(samples, start);
      if (score > bestScore) {
        bestScore = score;
        bestStart = start;
      }
    }

    if (bestScore < minScore) {
      return PreambleSearchResult(
        found: false,
        startSample: bestStart,
        score: bestScore,
      );
    }

    return PreambleSearchResult(
      found: true,
      startSample: bestStart,
      score: bestScore,
    );
  }

  /// Scores how strongly the region starting at [start] matches an alternating
  /// preamble. Range is roughly -1..1.
  double _preambleScore(Float64List samples, int start) {
    final s = config.samplesPerSymbol;
    double score = 0;
    int counted = 0;
    for (int k = 0; k < config.preambleBits; k++) {
      final pos = start + k * s;
      if (pos + s > samples.length) break;
      final core = pos + _guard;
      final e0 = _g0.energy(samples, start: core);
      final e1 = _g1.energy(samples, start: core);
      final total = e0 + e1;
      if (total <= 0) {
        counted++;
        continue;
      }
      // Preamble bit pattern: index 0 -> 1, index 1 -> 0, ...
      final expectedBit = k.isEven ? 1 : 0;
      final winner = e1 > e0 ? 1 : 0;
      final confidence = (e1 - e0).abs() / total;
      score += winner == expectedBit ? confidence : -confidence;
      counted++;
    }
    if (counted == 0) return -1;
    return score / counted;
  }
}
