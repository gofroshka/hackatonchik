import 'dart:math';
import 'dart:math' as math;
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:hackatonchik/audio/pcm_converter.dart';
import 'package:hackatonchik/modem/acoustic_modem.dart';
import 'package:hackatonchik/modem/streaming_decoder.dart';

/// Simulates a realistic microphone capture: quiet ambient noise, then the
/// (attenuated) transmitted signal, then ambient noise again — fed to the
/// [StreamingDecoder] in small chunks like a live mic stream.
void main() {
  test('StreamingDecoder decodes a live-style noisy capture', () {
    final modem = AcousticModem();
    const message = 'HELLO FROM AUDIO';
    final signal = PcmConverter.int16ToFloat(modem.encode(message).pcm);

    final rng = Random(7);
    const ambient = 0.008; // low room noise
    const gain = 0.3; // signal attenuated by distance/mic

    double noise() => (rng.nextDouble() * 2 - 1) * ambient;

    final builder = <double>[];
    // ~0.5 s ambient before the signal.
    for (int i = 0; i < 24000; i++) {
      builder.add(noise());
    }
    // Signal + ambient.
    for (int i = 0; i < signal.length; i++) {
      builder.add((signal[i] * gain + noise()).clamp(-1.0, 1.0));
    }
    // ~0.5 s ambient after the signal (so the decoder sees the burst end).
    for (int i = 0; i < 24000; i++) {
      builder.add(noise());
    }

    final input = Float64List.fromList(builder);
    final decoder = StreamingDecoder();

    final decoded = <String>[];
    const chunkSize = 2048;
    for (int off = 0; off < input.length; off += chunkSize) {
      final end = (off + chunkSize).clamp(0, input.length);
      final chunk = Float64List.sublistView(input, off, end);
      final events = decoder.addSamples(chunk);
      for (final e in events) {
        if (e.state == DecoderState.messageReceived && e.message != null) {
          decoded.add(e.message!.text);
        }
      }
    }

    expect(decoded, contains(message));
  });

  test('StreamingDecoder decodes a QUIET source under low-freq room rumble',
      () {
    final modem = AcousticModem();
    const message = 'HELLO';
    final signal = PcmConverter.int16ToFloat(modem.encode(message).pcm);
    final rate = modem.config.sampleRate;

    final rng = Random(11);
    const signalGain = 0.14; // very quiet transmission
    const rumbleAmp = 0.09; // dominant low-frequency room noise

    double rumble(int i) {
      final t = i / rate;
      return rumbleAmp * math.sin(2 * math.pi * 90 * t) +
          0.5 * rumbleAmp * math.sin(2 * math.pi * 140 * t) +
          (rng.nextDouble() * 2 - 1) * 0.01;
    }

    final builder = <double>[];
    var idx = 0;
    for (int i = 0; i < 24000; i++) {
      builder.add(rumble(idx++));
    }
    for (int i = 0; i < signal.length; i++) {
      builder.add((signal[i] * signalGain + rumble(idx++)).clamp(-1.0, 1.0));
    }
    for (int i = 0; i < 24000; i++) {
      builder.add(rumble(idx++));
    }

    final input = Float64List.fromList(builder);
    final decoder = StreamingDecoder();
    final decoded = <String>[];
    const chunkSize = 4096;
    for (int off = 0; off < input.length; off += chunkSize) {
      final end = (off + chunkSize).clamp(0, input.length);
      final chunk = Float64List.sublistView(input, off, end);
      for (final e in decoder.addSamples(chunk)) {
        if (e.state == DecoderState.messageReceived && e.message != null) {
          decoded.add(e.message!.text);
        }
      }
    }
    expect(decoded, contains(message));
  });

  test('StreamingDecoder stays idle on pure noise (no false positives)', () {
    final decoder = StreamingDecoder();
    final rng = Random(3);
    bool gotMessage = false;
    for (int c = 0; c < 40; c++) {
      final chunk = Float64List(2048);
      for (int i = 0; i < chunk.length; i++) {
        chunk[i] = (rng.nextDouble() * 2 - 1) * 0.01;
      }
      for (final e in decoder.addSamples(chunk)) {
        if (e.state == DecoderState.messageReceived) gotMessage = true;
      }
    }
    expect(gotMessage, isFalse);
  });
}
