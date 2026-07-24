import 'dart:math';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:hackatonchik/audio/pcm_converter.dart';
import 'package:hackatonchik/modem/acoustic_modem.dart';
import 'package:hackatonchik/modem/modem_config.dart';

/// Adds Gaussian white noise scaled by [amount] (fraction of full scale).
Float64List addNoise(Float64List input, double amount, {int seed = 42}) {
  final rng = Random(seed);
  final out = Float64List(input.length);
  for (int i = 0; i < input.length; i++) {
    final noise = (rng.nextDouble() * 2 - 1) * amount;
    out[i] = (input[i] + noise).clamp(-1.0, 1.0);
  }
  return out;
}

/// Scales amplitude by [gain].
Float64List scaleVolume(Float64List input, double gain) {
  final out = Float64List(input.length);
  for (int i = 0; i < input.length; i++) {
    out[i] = (input[i] * gain).clamp(-1.0, 1.0);
  }
  return out;
}

/// Prepends [count] samples of silence.
Float64List prependSilence(Float64List input, int count) {
  final out = Float64List(input.length + count);
  out.setRange(count, count + input.length, input);
  return out;
}

/// Appends [count] samples of silence.
Float64List appendSilence(Float64List input, int count) {
  final out = Float64List(input.length + count);
  out.setRange(0, input.length, input);
  return out;
}

void main() {
  final modem = AcousticModem();

  Float64List encodeToFloat(String message) {
    final result = modem.encode(message);
    return PcmConverter.int16ToFloat(result.pcm);
  }

  String? decodeFloat(Float64List samples) {
    final report = modem.decodeAll(samples);
    return report.firstMessage;
  }

  group('Loopback text -> PCM -> text', () {
    test('HELLO FROM AUDIO round trips cleanly', () {
      const message = 'HELLO FROM AUDIO';
      expect(decodeFloat(encodeToFloat(message)), message);
    });

    test('HELLO', () {
      expect(decodeFloat(encodeToFloat('HELLO')), 'HELLO');
    });

    test('Russian text (Привет)', () {
      expect(decodeFloat(encodeToFloat('Привет')), 'Привет');
    });

    test('emoji string', () {
      const message = 'Hi 👋🚀';
      expect(decodeFloat(encodeToFloat(message)), message);
    });

    test('empty string', () {
      expect(decodeFloat(encodeToFloat('')), '');
    });

    test('max single-packet payload', () {
      final maxChars =
          const ModemConfig().maxPayloadLength - 3; // fragment header
      final message = 'A' * maxChars;
      expect(decodeFloat(encodeToFloat(message)), message);
    });

    test('long multi-packet message is split and reassembled', () {
      // Two packets is enough to exercise fragmentation/reassembly; at 40 ms
      // per symbol a much longer message would make the loopback test very slow.
      final message =
          List.generate(180, (i) => String.fromCharCode(65 + (i % 26))).join();
      expect(decodeFloat(encodeToFloat(message)), message);
    });
  });

  group('Robustness', () {
    test('survives moderate white noise', () {
      const message = 'HELLO FROM AUDIO';
      final noisy = addNoise(encodeToFloat(message), 0.03);
      expect(decodeFloat(noisy), message);
    });

    test('survives reduced volume', () {
      const message = 'HELLO FROM AUDIO';
      final quiet = scaleVolume(encodeToFloat(message), 0.35);
      expect(decodeFloat(quiet), message);
    });

    test('survives leading and trailing silence', () {
      const message = 'HELLO FROM AUDIO';
      var samples = encodeToFloat(message);
      samples = prependSilence(samples, 5000);
      samples = appendSilence(samples, 5000);
      expect(decodeFloat(samples), message);
    });

    test('survives sub-symbol timing misalignment', () {
      const message = 'HELLO FROM AUDIO';
      // Prepend an arbitrary offset that is not a multiple of the symbol size.
      final samples = prependSilence(encodeToFloat(message), 137);
      expect(decodeFloat(samples), message);
    });
  });

  group('Corruption', () {
    test('a wrecked payload region is rejected by CRC', () {
      const message = 'HELLO FROM AUDIO';
      final samples = encodeToFloat(message);
      final rng = Random(1);
      // Wreck a large contiguous region of the packet — far more than the
      // repetition code can repair — so the CRC must reject it.
      final start = (samples.length * 0.4).round();
      final end = (samples.length * 0.7).round();
      for (int i = start; i < end; i++) {
        samples[i] = rng.nextDouble() * 2 - 1;
      }
      expect(decodeFloat(samples), isNot(message));
    });
  });
}
