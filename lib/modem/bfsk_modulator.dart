import 'dart:math' as math;
import 'dart:typed_data';

import '../protocol/bit_stream_builder.dart';
import '../protocol/packet.dart';
import '../protocol/packet_encoder.dart';
import '../protocol/repetition_code.dart';
import '../shared/utils/bit_utils.dart';
import 'modem_config.dart';

/// Generates continuous-phase BFSK PCM audio for a packet.
///
/// The generated frame consists of:
///   leading silence -> preamble (raw alternating bits) ->
///   repetition-coded packet bits -> trailing silence.
///
/// Phase is preserved between symbols and short raised-cosine ramps are applied
/// at the very start and end of the tone burst to avoid audible clicks and
/// spectral splatter.
class BfskModulator {
  BfskModulator(this.config)
      : _repetition = RepetitionCode(config.repetitionFactor);

  final ModemConfig config;
  final RepetitionCode _repetition;

  /// Builds the full bit stream (preamble + coded packet) for [packet].
  List<int> buildFrameBits(Packet packet) {
    final builder = BitStreamBuilder();

    // Preamble: alternating 1,0,1,0 ... (not error-corrected).
    for (int i = 0; i < config.preambleBits; i++) {
      builder.addBit(i.isEven ? 1 : 0);
    }

    final packetBytes = const PacketEncoder().encode(packet);
    final packetBits = BitUtils.bytesToBits(packetBytes);
    builder.addBits(_repetition.encode(packetBits));

    return builder.bits;
  }

  /// Modulates [packet] into 16-bit signed little-endian PCM samples.
  Int16List modulate(Packet packet) {
    final bits = buildFrameBits(packet);
    return modulateBits(bits);
  }

  /// Modulates an arbitrary [bits] stream into PCM, including silence padding.
  ///
  /// Uses continuous-phase FSK with a raised-cosine frequency transition at each
  /// symbol boundary (GFSK-style). Because both the phase AND the instantaneous
  /// frequency are continuous, the waveform has no slope discontinuities and
  /// therefore produces no broadband clicks — which also prevents the receiving
  /// phone's noise suppressor from chewing up the signal.
  Int16List modulateBits(List<int> bits) {
    final s = config.samplesPerSymbol;
    final leadingSilence =
        (config.sampleRate * config.leadingSilenceMs / 1000).round();
    final trailingSilence =
        (config.sampleRate * config.trailingSilenceMs / 1000).round();

    final toneSamples = bits.length * s;
    final total = leadingSilence + toneSamples + trailingSilence;
    final out = Int16List(total);

    final scale = config.amplitude * 32767.0;
    final half = config.transitionSamples ~/ 2;

    double freqOfSymbol(int k) => bits[k] == 1
        ? config.freq1.toDouble()
        : config.freq0.toDouble();

    double phase = 0;
    int writeIndex = leadingSilence;

    for (int j = 0; j < toneSamples; j++) {
      final k = j ~/ s;
      final p = j % s;
      final fk = freqOfSymbol(k);

      // Determine the instantaneous frequency, smoothing across boundaries.
      double freq = fk;
      if (half > 0) {
        if (p < half && k > 0) {
          // Second half of the transition from the previous symbol.
          final fPrev = freqOfSymbol(k - 1);
          final u = (half + p) / (2 * half); // 0.5 -> 1.0
          final w = 0.5 * (1 - math.cos(math.pi * u));
          freq = fPrev + (fk - fPrev) * w;
        } else if (p >= s - half && k < bits.length - 1) {
          // First half of the transition towards the next symbol.
          final fNext = freqOfSymbol(k + 1);
          final u = (p - (s - half)) / (2 * half); // 0.0 -> ~0.5
          final w = 0.5 * (1 - math.cos(math.pi * u));
          freq = fk + (fNext - fk) * w;
        }
      }

      // Raised-cosine amplitude ramp only at the very start/end of the burst.
      double envelope = 1.0;
      if (j < config.rampSamples) {
        envelope = 0.5 * (1 - math.cos(math.pi * j / config.rampSamples));
      } else if (j >= toneSamples - config.rampSamples) {
        final tail = toneSamples - j;
        envelope = 0.5 * (1 - math.cos(math.pi * tail / config.rampSamples));
      }

      double sample = math.sin(phase) * envelope * scale;
      if (sample > 32767) sample = 32767;
      if (sample < -32768) sample = -32768;
      out[writeIndex++] = sample.round();

      phase += 2 * math.pi * freq / config.sampleRate;
      if (phase > 2 * math.pi) phase -= 2 * math.pi;
    }

    return out;
  }
}
