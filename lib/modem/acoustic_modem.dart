import 'dart:typed_data';

import '../protocol/message_assembler.dart';
import '../protocol/packet.dart';
import '../protocol/packet_decoder.dart';
import '../protocol/packet_fragmenter.dart';
import 'bfsk_demodulator.dart';
import 'bfsk_modulator.dart';
import 'modem_config.dart';

/// Report produced by an offline decode over a complete PCM buffer.
class ModemDecodeReport {
  ModemDecodeReport({
    required this.messages,
    required this.packetsOk,
    required this.packetsCrcError,
    required this.lastDiagnostics,
  });

  final List<AssembledMessage> messages;
  final int packetsOk;
  final int packetsCrcError;
  final DemodDiagnostics lastDiagnostics;

  /// Convenience: the text of the first fully assembled message, if any.
  String? get firstMessage => messages.isEmpty ? null : messages.first.text;
}

/// High-level facade combining fragmentation, modulation and demodulation.
///
/// This class is pure Dart (no Flutter dependency) so it can be unit tested and
/// run inside an isolate.
class AcousticModem {
  AcousticModem({ModemConfig? config})
      : config = config ?? const ModemConfig() {
    _modulator = BfskModulator(this.config);
    _demodulator = BfskDemodulator(this.config);
    _fragmenter = PacketFragmenter(this.config);
  }

  final ModemConfig config;
  late final BfskModulator _modulator;
  late final BfskDemodulator _demodulator;
  late final PacketFragmenter _fragmenter;

  int _messageCounter = 0;

  /// Encodes [message] into one or more packets and returns the full PCM audio
  /// (16-bit signed) that transmits every packet back-to-back.
  EncodeResult encode(String message, {int? messageId}) {
    final id = messageId ?? (_messageCounter++ & 0xFF);
    final packets = _fragmenter.fragment(message, messageId: id);
    final builder = BytesBuilder();
    final chunks = <Int16List>[];
    int totalSamples = 0;
    for (final packet in packets) {
      final pcm = _modulator.modulate(packet);
      chunks.add(pcm);
      totalSamples += pcm.length;
    }
    // Concatenate.
    final combined = Int16List(totalSamples);
    int offset = 0;
    for (final chunk in chunks) {
      combined.setRange(offset, offset + chunk.length, chunk);
      offset += chunk.length;
    }
    builder.clear();
    return EncodeResult(
      pcm: combined,
      packets: packets,
      messageId: id,
    );
  }

  /// Decodes a complete PCM buffer (offline), returning every assembled message
  /// and packet statistics. Used for loopback tests, WAV import and demo mode.
  ModemDecodeReport decodeAll(Float64List samples) {
    final assembler = MessageAssembler();
    final messages = <AssembledMessage>[];
    int packetsOk = 0;
    int packetsCrcError = 0;
    DemodDiagnostics lastDiag = const DemodDiagnostics();

    int searchStart = 0;
    final s = config.samplesPerSymbol;
    int guard = 0;
    while (searchStart + config.preambleBits * s < samples.length) {
      final frame = _demodulator.decodeFrame(samples, searchStart: searchStart);
      lastDiag = frame.diagnostics;

      if (frame.status == PacketDecodeStatus.noSync) {
        // No preamble found from here on.
        break;
      }

      if (frame.status == PacketDecodeStatus.ok) {
        packetsOk++;
        final packet = frame.packetResult!.packet!;
        final assembled = assembler.addPacket(packet);
        if (assembled != null) messages.add(assembled);
      } else if (frame.status == PacketDecodeStatus.crcError) {
        packetsCrcError++;
      }

      // Advance past this frame to look for the next one.
      final next = frame.consumedUntil;
      if (next <= searchStart) {
        searchStart += s; // Ensure forward progress.
      } else {
        searchStart = next;
      }

      if (++guard > 10000) break; // Safety against pathological inputs.
    }

    return ModemDecodeReport(
      messages: messages,
      packetsOk: packetsOk,
      packetsCrcError: packetsCrcError,
      lastDiagnostics: lastDiag,
    );
  }

  /// Directly modulate a single [packet] (used by demo/diagnostics).
  Int16List modulatePacket(Packet packet) => _modulator.modulate(packet);
}

/// Result of encoding a message to PCM.
class EncodeResult {
  const EncodeResult({
    required this.pcm,
    required this.packets,
    required this.messageId,
  });

  final Int16List pcm;
  final List<Packet> packets;
  final int messageId;
}
