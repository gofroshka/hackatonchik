import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:hackatonchik/modem/goertzel.dart';

void main() {
  group('Goertzel', () {
    const sampleRate = 24000;
    const blockSize = 480; // 20 ms

    Float64List tone(double freq) {
      final out = Float64List(blockSize);
      for (int i = 0; i < blockSize; i++) {
        out[i] = math.sin(2 * math.pi * freq * i / sampleRate);
      }
      return out;
    }

    test('responds strongly at the target frequency', () {
      final g = Goertzel(
        targetFrequency: 1500,
        sampleRate: sampleRate,
        blockSize: blockSize,
      );
      final atTarget = g.energy(tone(1500));
      final offTarget = g.energy(tone(2500));
      expect(atTarget, greaterThan(offTarget * 10));
    });

    test('discriminates BFSK frequencies', () {
      final g0 = Goertzel(
        targetFrequency: 1500,
        sampleRate: sampleRate,
        blockSize: blockSize,
      );
      final g1 = Goertzel(
        targetFrequency: 2500,
        sampleRate: sampleRate,
        blockSize: blockSize,
      );

      final signal1 = tone(2500);
      expect(g1.energy(signal1), greaterThan(g0.energy(signal1)));

      final signal0 = tone(1500);
      expect(g0.energy(signal0), greaterThan(g1.energy(signal0)));
    });
  });
}
