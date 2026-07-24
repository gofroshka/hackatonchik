import 'package:flutter_test/flutter_test.dart';
import 'package:hackatonchik/audio/pcm_converter.dart';
import 'package:hackatonchik/modem/acoustic_modem.dart';

void main() {
  final modem = AcousticModem();

  group('Click-free modulation', () {
    test('waveform has no sharp discontinuities (no clicks)', () {
      final pcm = modem.encode('HELLO FROM AUDIO', messageId: 0).pcm;

      // The second difference approximates curvature; a hard frequency switch
      // (a click) produces a large spike. A smooth GFSK signal stays bounded
      // near the single-tone curvature limit.
      int maxSecondDiff = 0;
      for (int i = 2; i < pcm.length; i++) {
        final d2 = (pcm[i] - 2 * pcm[i - 1] + pcm[i - 2]).abs();
        if (d2 > maxSecondDiff) maxSecondDiff = d2;
      }

      // A hard-switched BFSK signal spikes well above ~5000 here; smooth GFSK
      // stays close to the ~2500 single-tone bound.
      expect(maxSecondDiff, lessThan(3500));
    });

    test('no full-scale sample-to-sample jumps', () {
      final pcm = modem.encode('HELLO', messageId: 0).pcm;
      int maxJump = 0;
      for (int i = 1; i < pcm.length; i++) {
        final d = (pcm[i] - pcm[i - 1]).abs();
        if (d > maxJump) maxJump = d;
      }
      // The steepest slope belongs to the 2.5 kHz tone; nowhere near a click.
      expect(maxJump, lessThan(9000));
    });
  });

  group('Length-aware decoding tolerates mid-burst dropouts', () {
    test('a strongly attenuated middle region still decodes', () {
      const message = 'HELLO FROM AUDIO';
      final samples = PcmConverter.int16ToFloat(modem.encode(message).pcm);

      // Simulate the phone briefly ducking the signal (AGC / noise suppressor)
      // by attenuating a middle region. Frequency ratio is preserved, so bits
      // are still correct — the length-aware decoder must not stop early here.
      final start = (samples.length * 0.45).round();
      final end = (samples.length * 0.52).round();
      for (int i = start; i < end; i++) {
        samples[i] *= 0.03;
      }

      expect(modem.decodeAll(samples).firstMessage, message);
    });
  });
}
