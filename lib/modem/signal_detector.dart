import 'dart:math' as math;
import 'dart:typed_data';

/// Tracks an adaptive noise floor and reports whether a meaningful signal is
/// currently present.
///
/// The floor is bootstrapped from the very first chunk (so it reflects the real
/// ambient level of the device instead of an arbitrary constant) and then
/// tracks the background: it drops quickly towards quieter input and rises only
/// very slowly, so a loud transmission does not inflate it. This lets the
/// detector correctly report the *end* of a burst (return to silence), which is
/// what triggers decoding.
class SignalDetector {
  SignalDetector({
    this.snrThreshold = 3.0,
    this.absoluteFloor = 2e-3,
  });

  /// Linear SNR above the noise floor required to declare "signal present".
  final double snrThreshold;

  /// Absolute minimum RMS to consider anything present at all (guards against
  /// declaring digital silence as signal when the floor is tiny).
  final double absoluteFloor;

  double _noiseFloor = 0;
  bool _initialized = false;
  double _lastRms = 0;
  double _lastSnr = 0;

  double get noiseFloor => _noiseFloor;
  double get lastRms => _lastRms;
  double get lastSnr => _lastSnr;

  /// Computes the RMS of [samples], updates the adaptive noise floor and returns
  /// whether a signal is present.
  bool update(Float64List samples, {int start = 0, int? count}) {
    final n = count ?? samples.length;
    if (n <= 0) return false;

    double sumSq = 0;
    final end = start + n;
    for (int i = start; i < end; i++) {
      final s = samples[i];
      sumSq += s * s;
    }
    final rms = math.sqrt(sumSq / n);
    _lastRms = rms;

    if (!_initialized) {
      _noiseFloor = math.max(rms, absoluteFloor);
      _initialized = true;
    }

    final snr = rms / (_noiseFloor + 1e-12);
    _lastSnr = snr;

    final present = rms > absoluteFloor && snr >= snrThreshold;

    // Update the floor: track downwards fairly quickly, upwards very slowly so
    // an ongoing tone burst never pulls the floor up to itself.
    if (rms < _noiseFloor) {
      _noiseFloor = 0.9 * _noiseFloor + 0.1 * rms;
    } else if (!present) {
      _noiseFloor = 0.995 * _noiseFloor + 0.005 * rms;
    }
    if (_noiseFloor < 1e-5) _noiseFloor = 1e-5;

    return present;
  }

  void reset() {
    _noiseFloor = 0;
    _initialized = false;
    _lastRms = 0;
    _lastSnr = 0;
  }
}
