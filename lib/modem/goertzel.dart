import 'dart:math' as math;
import 'dart:typed_data';

/// Goertzel algorithm for measuring the energy of a single target frequency
/// within a block of samples.
///
/// This is far cheaper than a full FFT when only a couple of frequencies need
/// to be evaluated, which is exactly the BFSK demodulation case.
class Goertzel {
  Goertzel({
    required this.targetFrequency,
    required this.sampleRate,
    required this.blockSize,
  }) {
    final k = (0.5 + blockSize * targetFrequency / sampleRate).floor();
    final omega = 2 * math.pi * k / blockSize;
    _coeff = 2 * math.cos(omega);
  }

  final double targetFrequency;
  final int sampleRate;
  final int blockSize;

  late final double _coeff;

  /// Computes the squared magnitude (energy) of [targetFrequency] over
  /// [samples] in the range `[start, start + blockSize)`.
  double energy(Float64List samples, {int start = 0}) {
    double s0 = 0;
    double s1 = 0;
    double s2 = 0;
    final end = start + blockSize;
    for (int i = start; i < end; i++) {
      s0 = samples[i] + _coeff * s1 - s2;
      s2 = s1;
      s1 = s0;
    }
    // Squared magnitude of the DFT bin.
    return s1 * s1 + s2 * s2 - _coeff * s1 * s2;
  }
}
