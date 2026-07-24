import 'dart:typed_data';

/// CRC-16/CCITT-FALSE implementation.
///
/// Polynomial: 0x1021, initial value: 0xFFFF, no reflection, no final XOR.
/// This is a widely used and easily verifiable CRC variant. The check value
/// for the ASCII string "123456789" is 0x29B1.
class Crc16 {
  const Crc16._();

  static const int _polynomial = 0x1021;
  static const int _initial = 0xFFFF;

  /// Computes the CRC-16/CCITT-FALSE over [data].
  static int compute(List<int> data) {
    int crc = _initial;
    for (final byte in data) {
      crc ^= (byte & 0xFF) << 8;
      for (int i = 0; i < 8; i++) {
        if ((crc & 0x8000) != 0) {
          crc = (crc << 1) ^ _polynomial;
        } else {
          crc <<= 1;
        }
        crc &= 0xFFFF;
      }
    }
    return crc & 0xFFFF;
  }

  /// Computes the CRC over a [Uint8List] view.
  static int computeBytes(Uint8List data) => compute(data);
}
