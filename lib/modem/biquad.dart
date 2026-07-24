import 'dart:math' as math;
import 'dart:typed_data';

/// A single biquad (second-order IIR) filter section, Direct Form I.
///
/// Coefficients follow the well-known Audio EQ Cookbook (RBJ) formulas.
class Biquad {
  Biquad(this._b0, this._b1, this._b2, this._a1, this._a2);

  final double _b0, _b1, _b2, _a1, _a2;
  double _x1 = 0, _x2 = 0, _y1 = 0, _y2 = 0;

  /// Low-pass section at [cutoff] Hz for the given [sampleRate].
  factory Biquad.lowPass(double cutoff, int sampleRate, {double q = 0.7071}) {
    final w0 = 2 * math.pi * cutoff / sampleRate;
    final cosw = math.cos(w0);
    final alpha = math.sin(w0) / (2 * q);
    final a0 = 1 + alpha;
    final b0 = ((1 - cosw) / 2) / a0;
    final b1 = (1 - cosw) / a0;
    final b2 = ((1 - cosw) / 2) / a0;
    final a1 = (-2 * cosw) / a0;
    final a2 = (1 - alpha) / a0;
    return Biquad(b0, b1, b2, a1, a2);
  }

  /// High-pass section at [cutoff] Hz for the given [sampleRate].
  factory Biquad.highPass(double cutoff, int sampleRate, {double q = 0.7071}) {
    final w0 = 2 * math.pi * cutoff / sampleRate;
    final cosw = math.cos(w0);
    final alpha = math.sin(w0) / (2 * q);
    final a0 = 1 + alpha;
    final b0 = ((1 + cosw) / 2) / a0;
    final b1 = (-(1 + cosw)) / a0;
    final b2 = ((1 + cosw) / 2) / a0;
    final a1 = (-2 * cosw) / a0;
    final a2 = (1 - alpha) / a0;
    return Biquad(b0, b1, b2, a1, a2);
  }

  double processSample(double x) {
    final y =
        _b0 * x + _b1 * _x1 + _b2 * _x2 - _a1 * _y1 - _a2 * _y2;
    _x2 = _x1;
    _x1 = x;
    _y2 = _y1;
    _y1 = y;
    return y;
  }

  void reset() {
    _x1 = _x2 = _y1 = _y2 = 0;
  }
}

/// A band-pass filter built from a cascade of high-pass then low-pass biquads.
///
/// Applied to the microphone input, it removes low-frequency room rumble and
/// high-frequency hiss so the BFSK tones dominate the signal — dramatically
/// improving detection and decoding at low volume in real rooms.
class BandpassFilter {
  BandpassFilter({
    required double lowCutoff,
    required double highCutoff,
    required int sampleRate,
  }) : _sections = [
          Biquad.highPass(lowCutoff, sampleRate),
          Biquad.highPass(lowCutoff, sampleRate),
          Biquad.lowPass(highCutoff, sampleRate),
          Biquad.lowPass(highCutoff, sampleRate),
        ];

  final List<Biquad> _sections;

  /// Filters [input] in place-safe fashion, returning a new buffer.
  Float64List process(Float64List input) {
    final out = Float64List(input.length);
    for (int i = 0; i < input.length; i++) {
      double s = input[i];
      for (final section in _sections) {
        s = section.processSample(s);
      }
      out[i] = s;
    }
    return out;
  }

  void reset() {
    for (final section in _sections) {
      section.reset();
    }
  }
}
