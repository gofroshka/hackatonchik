import 'dart:typed_data';

/// Conversions between 16-bit signed PCM and normalized floating point samples.
class PcmConverter {
  const PcmConverter._();

  /// Converts signed 16-bit little-endian PCM [bytes] into normalized doubles
  /// in the range roughly -1.0 .. 1.0.
  static Float64List bytesToFloat(Uint8List bytes) {
    final sampleCount = bytes.length ~/ 2;
    final out = Float64List(sampleCount);
    final data = ByteData.sublistView(bytes);
    for (int i = 0; i < sampleCount; i++) {
      out[i] = data.getInt16(i * 2, Endian.little) / 32768.0;
    }
    return out;
  }

  /// Converts an [Int16List] of PCM samples into normalized doubles.
  static Float64List int16ToFloat(Int16List samples) {
    final out = Float64List(samples.length);
    for (int i = 0; i < samples.length; i++) {
      out[i] = samples[i] / 32768.0;
    }
    return out;
  }

  /// Converts an [Int16List] into signed 16-bit little-endian bytes.
  static Uint8List int16ToBytes(Int16List samples) {
    final out = Uint8List(samples.length * 2);
    final data = ByteData.sublistView(out);
    for (int i = 0; i < samples.length; i++) {
      data.setInt16(i * 2, samples[i], Endian.little);
    }
    return out;
  }

  /// Converts signed 16-bit little-endian PCM [bytes] into an [Int16List].
  static Int16List bytesToInt16(Uint8List bytes) {
    final sampleCount = bytes.length ~/ 2;
    final out = Int16List(sampleCount);
    final data = ByteData.sublistView(bytes);
    for (int i = 0; i < sampleCount; i++) {
      out[i] = data.getInt16(i * 2, Endian.little);
    }
    return out;
  }
}
