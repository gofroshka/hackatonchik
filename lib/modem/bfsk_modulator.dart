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
  Int16List modulateBits(List<int> bits) {
    final samplesPerSymbol = config.samplesPerSymbol;
    final leadingSilence =
        (config.sampleRate * config.leadingSilenceMs / 1000).round();
    final trailingSilence =
        (config.sampleRate * config.trailingSilenceMs / 1000).round();

    final toneSamples = bits.length * samplesPerSymbol;
    final total = leadingSilence + toneSamples + trailingSilence;
    final out = Int16List(total);

    double phase = 0;
    int writeIndex = leadingSilence;
    final scale = config.amplitude * 32767.0;

    for (int b = 0; b < bits.length; b++) {
      final freq = bits[b] == 1 ? config.freq1 : config.freq0;
      final phaseStep = 2 * math.pi * freq / config.sampleRate;
      for (int i = 0; i < samplesPerSymbol; i++) {
        double envelope = 1.0;
        // Apply a raised-cosine ramp only at the very beginning and end of the
        // whole tone burst to avoid clicks while keeping symbols phase-locked.
        final globalIndex = b * samplesPerSymbol + i;
        if (globalIndex < config.rampSamples) {
          envelope = 0.5 *
              (1 - math.cos(math.pi * globalIndex / config.rampSamples));
        } else if (globalIndex >= toneSamples - config.rampSamples) {
          final tail = toneSamples - globalIndex;
          envelope =
              0.5 * (1 - math.cos(math.pi * tail / config.rampSamples));
        }

        double sample = math.sin(phase) * envelope * scale;
        // Guard against any accidental clipping.
        if (sample > 32767) sample = 32767;
        if (sample < -32768) sample = -32768;
        out[writeIndex++] = sample.round();

        phase += phaseStep;
        if (phase > 2 * math.pi) phase -= 2 * math.pi;
      }
    }

    return out;
  }
}
