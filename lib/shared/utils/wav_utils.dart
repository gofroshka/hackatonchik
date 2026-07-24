import 'dart:typed_data';

import '../../audio/pcm_converter.dart';

/// Data parsed from a WAV file.
class WavData {
  const WavData({
    required this.samples,
    required this.sampleRate,
    required this.channels,
  });

  final Int16List samples;
  final int sampleRate;
  final int channels;
}

/// Minimal WAV (RIFF/PCM) reader and writer for 16-bit mono/stereo audio.
///
/// Only uncompressed PCM is supported by design — lossy codecs would destroy
/// the modem signal.
class WavUtils {
  const WavUtils._();

  /// Encodes 16-bit PCM [samples] into a canonical 44-byte-header WAV file.
  static Uint8List encode(
    Int16List samples, {
    required int sampleRate,
    int channels = 1,
  }) {
    final dataBytes = PcmConverter.int16ToBytes(samples);
    final byteRate = sampleRate * channels * 2;
    final blockAlign = channels * 2;
    final dataLength = dataBytes.length;
    final fileLength = 36 + dataLength;

    final out = BytesBuilder();
    void writeString(String s) => out.add(s.codeUnits);
    void writeUint32(int v) {
      final b = ByteData(4)..setUint32(0, v, Endian.little);
      out.add(b.buffer.asUint8List());
    }

    void writeUint16(int v) {
      final b = ByteData(2)..setUint16(0, v, Endian.little);
      out.add(b.buffer.asUint8List());
    }

    writeString('RIFF');
    writeUint32(fileLength);
    writeString('WAVE');
    writeString('fmt ');
    writeUint32(16); // PCM fmt chunk size
    writeUint16(1); // audio format = PCM
    writeUint16(channels);
    writeUint32(sampleRate);
    writeUint32(byteRate);
    writeUint16(blockAlign);
    writeUint16(16); // bits per sample
    writeString('data');
    writeUint32(dataLength);
    out.add(dataBytes);

    return out.toBytes();
  }

  /// Decodes a WAV file into PCM samples. If the file is stereo, only the first
  /// channel is returned (mono) so it can be fed directly to the demodulator.
  static WavData decode(Uint8List bytes) {
    if (bytes.length < 44) {
      throw const FormatException('File too small to be a valid WAV.');
    }
    final data = ByteData.sublistView(bytes);

    String tag(int offset) =>
        String.fromCharCodes(bytes.sublist(offset, offset + 4));

    if (tag(0) != 'RIFF' || tag(8) != 'WAVE') {
      throw const FormatException('Not a RIFF/WAVE file.');
    }

    int channels = 1;
    int sampleRate = 44100;
    int bitsPerSample = 16;
    int offset = 12;
    Uint8List? pcm;

    while (offset + 8 <= bytes.length) {
      final chunkId = tag(offset);
      final chunkSize = data.getUint32(offset + 4, Endian.little);
      final bodyStart = offset + 8;
      if (chunkId == 'fmt ') {
        channels = data.getUint16(bodyStart + 2, Endian.little);
        sampleRate = data.getUint32(bodyStart + 4, Endian.little);
        bitsPerSample = data.getUint16(bodyStart + 14, Endian.little);
      } else if (chunkId == 'data') {
        final end = (bodyStart + chunkSize).clamp(0, bytes.length);
        pcm = Uint8List.fromList(bytes.sublist(bodyStart, end));
      }
      // Chunks are word-aligned.
      offset = bodyStart + chunkSize + (chunkSize.isOdd ? 1 : 0);
    }

    if (pcm == null) {
      throw const FormatException('No data chunk found in WAV.');
    }
    if (bitsPerSample != 16) {
      throw FormatException(
        'Unsupported bit depth: $bitsPerSample (only 16-bit PCM supported).',
      );
    }

    final interleaved = PcmConverter.bytesToInt16(pcm);
    if (channels <= 1) {
      return WavData(
        samples: interleaved,
        sampleRate: sampleRate,
        channels: 1,
      );
    }

    // Downmix to first channel.
    final mono = Int16List(interleaved.length ~/ channels);
    for (int i = 0; i < mono.length; i++) {
      mono[i] = interleaved[i * channels];
    }
    return WavData(samples: mono, sampleRate: sampleRate, channels: 1);
  }
}
