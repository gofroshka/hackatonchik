import 'dart:math' as math;
import 'dart:typed_data';

/// Tracks an adaptive noise floor and reports whether a meaningful signal is
/// currently present.
///
/// The noise floor is estimated with an exponential moving average of the
/// signal RMS that only adapts (slowly) while no strong signal is detected,
/// so a loud transmission does not inflate the floor.
class SignalDetector {
  SignalDetector({
    this.snrThreshold = 3.0,
    this.floorAdaptRate = 0.05,
    double initialFloor = 1e-4,
  }) : _noiseFloor = initialFloor;

  final double snrThreshold;
  final double floorAdaptRate;

  double _noiseFloor;
  double _lastRms = 0;
  double _lastSnr = 0;

  double get noiseFloor => _noiseFloor;
  double get lastRms => _lastRms;

  /// Linear signal-to-noise ratio of the most recent [update].
  double get lastSnr => _lastSnr;

  /// Computes the RMS of [samples] in range `[start, start+count)`,
  /// updates the adaptive noise floor and returns whether a signal is present.
  bool update(Float64List samples, {int start = 0, int? count}) {
    final n = count ?? samples.length;
    if (n <= 0) return false;

    double sumSq = 0;
    final end = start + n;
    for (int i = start; i < end; i++) {
      sumSq += samples[i] * samples[i];
    }
    final rms = math.sqrt(sumSq / n);
    _lastRms = rms;

    final snr = rms / (_noiseFloor + 1e-12);
    _lastSnr = snr;

    final present = snr >= snrThreshold;
    if (!present) {
      // Only adapt the floor towards the current (quiet) level.
      _noiseFloor =
          (1 - floorAdaptRate) * _noiseFloor + floorAdaptRate * rms;
      if (_noiseFloor < 1e-6) _noiseFloor = 1e-6;
    }
    return present;
  }

  void reset({double initialFloor = 1e-4}) {
    _noiseFloor = initialFloor;
    _lastRms = 0;
    _lastSnr = 0;
  }
}
