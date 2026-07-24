import 'package:flutter_test/flutter_test.dart';
import 'package:hackatonchik/protocol/repetition_code.dart';

void main() {
  const code = RepetitionCode(3);

  group('RepetitionCode', () {
    test('encode repeats each bit factor times', () {
      expect(code.encode([1, 0, 1]), [1, 1, 1, 0, 0, 0, 1, 1, 1]);
    });

    test('decode recovers clean bits', () {
      final result = code.decode([1, 1, 1, 0, 0, 0]);
      expect(result.bits, [1, 0]);
      expect(result.correctedBits, 0);
    });

    test('majority voting corrects a single error per group', () {
      final result = code.decode([1, 0, 1, 0, 1, 0]);
      expect(result.bits, [1, 0]);
      expect(result.correctedBits, 2);
    });

    test('soft decode weights confidence', () {
      // Group 1: bits 0,1,1 but the two 1s have tiny confidence and the 0 is
      // very confident -> decides 0.
      final result = code.decodeSoft(
        [0, 1, 1],
        [0.9, 0.05, 0.05],
      );
      expect(result.bits, [0]);
    });

    test('round trip through encode/decode', () {
      final bits = [1, 0, 0, 1, 1, 0, 1, 0];
      final decoded = code.decode(code.encode(bits));
      expect(decoded.bits, bits);
    });
  });
}
