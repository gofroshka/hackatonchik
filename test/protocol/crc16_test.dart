import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:hackatonchik/protocol/crc16.dart';

void main() {
  group('Crc16 (CCITT-FALSE)', () {
    test('known test vector "123456789" == 0x29B1', () {
      final crc = Crc16.compute(ascii.encode('123456789'));
      expect(crc, 0x29B1);
    });

    test('empty input == initial value 0xFFFF', () {
      expect(Crc16.compute(const []), 0xFFFF);
    });

    test('single flipped bit changes the CRC', () {
      final a = Crc16.compute([0x01, 0x02, 0x03, 0x04]);
      final b = Crc16.compute([0x01, 0x02, 0x03, 0x05]);
      expect(a, isNot(equals(b)));
    });

    test('is deterministic', () {
      final data = [10, 20, 30, 40, 50];
      expect(Crc16.compute(data), Crc16.compute(data));
    });
  });
}
