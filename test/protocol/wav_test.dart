import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:hackatonchik/audio/pcm_converter.dart';
import 'package:hackatonchik/modem/acoustic_modem.dart';
import 'package:hackatonchik/shared/utils/wav_utils.dart';

void main() {
  group('WavUtils', () {
    test('encode produces a valid 44-byte RIFF header', () {
      final samples = Int16List.fromList([0, 1000, -1000, 32767, -32768]);
      final wav = WavUtils.encode(samples, sampleRate: 24000);
      expect(String.fromCharCodes(wav.sublist(0, 4)), 'RIFF');
      expect(String.fromCharCodes(wav.sublist(8, 12)), 'WAVE');
      expect(wav.length, 44 + samples.length * 2);
    });

    test('encode/decode round trips samples', () {
      final samples = Int16List.fromList([0, 500, -500, 12345, -12345, 32000]);
      final wav = WavUtils.encode(samples, sampleRate: 24000);
      final decoded = WavUtils.decode(wav);
      expect(decoded.sampleRate, 24000);
      expect(decoded.channels, 1);
      expect(decoded.samples, samples);
    });

    test('generated modem signal survives WAV round trip and decodes', () {
      final modem = AcousticModem();
      const message = 'HELLO FROM AUDIO';
      final encoded = modem.encode(message);

      final wav = WavUtils.encode(encoded.pcm, sampleRate: 24000);
      final decoded = WavUtils.decode(wav);
      final samples = PcmConverter.int16ToFloat(decoded.samples);

      final report = modem.decodeAll(samples);
      expect(report.firstMessage, message);
    });

    test('rejects non-WAV data', () {
      expect(
        () => WavUtils.decode(Uint8List.fromList(List.filled(50, 0))),
        throwsA(isA<FormatException>()),
      );
    });
  });
}
