/// Simple forward error correction based on bit repetition with majority
/// voting on decode.
///
/// The architecture intentionally keeps encode/decode behind a small interface
/// so that this can later be replaced by Hamming or Reed–Solomon codes without
/// touching the modem or protocol layers.
class RepetitionCode {
  const RepetitionCode(this.factor)
      : assert(factor >= 1, 'Repetition factor must be >= 1');

  /// How many times each input bit is repeated. Should be odd for unambiguous
  /// majority voting.
  final int factor;

  /// Encodes [bits] by repeating every bit [factor] times.
  List<int> encode(List<int> bits) {
    final out = <int>[];
    for (final bit in bits) {
      final b = bit & 1;
      for (int i = 0; i < factor; i++) {
        out.add(b);
      }
    }
    return out;
  }

  /// Decodes hard [bits] using majority voting. Extra trailing bits that do
  /// not form a complete group are ignored.
  RepetitionDecodeResult decode(List<int> bits) {
    final out = <int>[];
    int corrected = 0;
    final groups = bits.length ~/ factor;
    for (int g = 0; g < groups; g++) {
      int ones = 0;
      for (int i = 0; i < factor; i++) {
        ones += bits[g * factor + i] & 1;
      }
      final decided = ones * 2 > factor ? 1 : 0;
      final zeros = factor - ones;
      // Number of bits that disagreed with the majority = number of errors
      // this code corrected for this group.
      corrected += decided == 1 ? zeros : ones;
      out.add(decided);
    }
    return RepetitionDecodeResult(bits: out, correctedBits: corrected);
  }

  /// Decodes using soft confidence weights (0..1) per input bit. Bits with a
  /// higher confidence contribute more to the majority decision.
  RepetitionDecodeResult decodeSoft(
    List<int> bits,
    List<double> confidences,
  ) {
    final out = <int>[];
    int corrected = 0;
    final groups = bits.length ~/ factor;
    for (int g = 0; g < groups; g++) {
      double weightedOnes = 0;
      double weightedZeros = 0;
      int hardOnes = 0;
      for (int i = 0; i < factor; i++) {
        final idx = g * factor + i;
        final bit = bits[idx] & 1;
        final w = idx < confidences.length ? confidences[idx] : 1.0;
        if (bit == 1) {
          weightedOnes += w;
          hardOnes++;
        } else {
          weightedZeros += w;
        }
      }
      final decided = weightedOnes >= weightedZeros ? 1 : 0;
      final hardZeros = factor - hardOnes;
      corrected += decided == 1 ? hardZeros : hardOnes;
      out.add(decided);
    }
    return RepetitionDecodeResult(bits: out, correctedBits: corrected);
  }
}

/// Result of a repetition-code decode operation.
class RepetitionDecodeResult {
  const RepetitionDecodeResult({
    required this.bits,
    required this.correctedBits,
  });

  /// The recovered bits.
  final List<int> bits;

  /// The number of individual repeated bits that disagreed with the majority
  /// and were therefore effectively corrected.
  final int correctedBits;
}
