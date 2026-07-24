import 'dart:typed_data';

/// Utilities for converting between bytes and bit lists (MSB first).
class BitUtils {
  const BitUtils._();

  /// Converts [bytes] into a list of bits, most-significant bit first.
  static List<int> bytesToBits(List<int> bytes) {
    final bits = <int>[];
    for (final byte in bytes) {
      for (int i = 7; i >= 0; i--) {
        bits.add((byte >> i) & 1);
      }
    }
    return bits;
  }

  /// Converts a list of [bits] (MSB first) back into bytes. Trailing bits that
  /// do not form a full byte are ignored.
  static Uint8List bitsToBytes(List<int> bits) {
    final byteCount = bits.length ~/ 8;
    final out = Uint8List(byteCount);
    for (int b = 0; b < byteCount; b++) {
      int value = 0;
      for (int i = 0; i < 8; i++) {
        value = (value << 1) | (bits[b * 8 + i] & 1);
      }
      out[b] = value;
    }
    return out;
  }

  /// Converts a 16-bit value into a big-endian bit list of length 16.
  static List<int> word16ToBits(int value) {
    final bits = <int>[];
    for (int i = 15; i >= 0; i--) {
      bits.add((value >> i) & 1);
    }
    return bits;
  }
}
