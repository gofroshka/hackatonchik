import 'dart:convert';
import 'dart:typed_data';

import '../modem/modem_config.dart';
import 'packet.dart';

/// Splits a UTF-8 message into one or more [Packet]s.
///
/// Each packet payload carries a small fragment header so the receiver can
/// reassemble multi-packet messages:
///
/// ```
/// | messageId (1) | packetIndex (1) | packetCount (1) | chunk (...) |
/// ```
class PacketFragmenter {
  PacketFragmenter(this.config);

  final ModemConfig config;

  static const int fragmentHeaderSize = 3;

  /// Maximum number of UTF-8 bytes that fit in a single packet chunk.
  int get maxChunkSize => config.maxPayloadLength - fragmentHeaderSize;

  /// Fragments [message] into packets. [messageId] should be unique per message
  /// (0..255) so the receiver can distinguish concurrent transfers.
  List<Packet> fragment(String message, {required int messageId}) {
    final bytes = Uint8List.fromList(utf8.encode(message));
    final chunkSize = maxChunkSize;

    final packetCount = bytes.isEmpty ? 1 : (bytes.length + chunkSize - 1) ~/ chunkSize;
    if (packetCount > 255) {
      throw ArgumentError(
        'Message too large: needs $packetCount packets (max 255).',
      );
    }

    final packets = <Packet>[];
    for (int index = 0; index < packetCount; index++) {
      final startByte = index * chunkSize;
      final endByte = (startByte + chunkSize).clamp(0, bytes.length);
      final chunk = bytes.sublist(startByte, endByte);

      final payload = BytesBuilder();
      payload.addByte(messageId & 0xFF);
      payload.addByte(index & 0xFF);
      payload.addByte(packetCount & 0xFF);
      payload.add(chunk);

      packets.add(
        Packet(
          type: PacketType.text,
          sequence: index & 0xFF,
          payload: payload.toBytes(),
        ),
      );
    }
    return packets;
  }
}

/// A parsed fragment header + chunk extracted from a packet payload.
class Fragment {
  const Fragment({
    required this.messageId,
    required this.packetIndex,
    required this.packetCount,
    required this.data,
  });

  final int messageId;
  final int packetIndex;
  final int packetCount;
  final Uint8List data;

  /// Parses a fragment out of a raw packet [payload]. Returns null if the
  /// payload is too short to contain a valid fragment header.
  static Fragment? parse(Uint8List payload) {
    if (payload.length < PacketFragmenter.fragmentHeaderSize) return null;
    return Fragment(
      messageId: payload[0],
      packetIndex: payload[1],
      packetCount: payload[2],
      data: Uint8List.fromList(
        payload.sublist(PacketFragmenter.fragmentHeaderSize),
      ),
    );
  }
}
