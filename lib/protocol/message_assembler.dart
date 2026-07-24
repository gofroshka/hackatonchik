import 'dart:convert';
import 'dart:typed_data';

import 'packet.dart';
import 'packet_fragmenter.dart';

/// A fully reassembled message.
class AssembledMessage {
  const AssembledMessage({
    required this.messageId,
    required this.text,
    required this.packetCount,
  });

  final int messageId;
  final String text;
  final int packetCount;
}

/// Reassembles fragmented messages from individually received packets.
///
/// Fragments belonging to the same [messageId] are buffered until every
/// fragment index has been seen, at which point the full message is decoded.
class MessageAssembler {
  final Map<int, _MessageBuffer> _buffers = {};

  /// Feeds a validated [packet] into the assembler.
  ///
  /// Returns an [AssembledMessage] once all fragments for that message have
  /// arrived, otherwise null.
  AssembledMessage? addPacket(Packet packet) {
    final fragment = Fragment.parse(packet.payload);
    if (fragment == null) return null;

    final buffer = _buffers.putIfAbsent(
      fragment.messageId,
      () => _MessageBuffer(fragment.packetCount),
    );
    buffer.fragments[fragment.packetIndex] = fragment.data;

    if (buffer.isComplete) {
      final message = buffer.assemble();
      _buffers.remove(fragment.messageId);
      return AssembledMessage(
        messageId: fragment.messageId,
        text: message,
        packetCount: fragment.packetCount,
      );
    }
    return null;
  }

  /// Progress (0..1) for a given message id, or 0 if unknown.
  double progressFor(int messageId) {
    final buffer = _buffers[messageId];
    if (buffer == null) return 0;
    return buffer.fragments.length / buffer.packetCount;
  }

  /// Clears all buffered partial messages.
  void reset() => _buffers.clear();
}

class _MessageBuffer {
  _MessageBuffer(this.packetCount);

  final int packetCount;
  final Map<int, Uint8List> fragments = {};

  bool get isComplete => fragments.length == packetCount;

  String assemble() {
    final builder = BytesBuilder();
    for (int i = 0; i < packetCount; i++) {
      final data = fragments[i];
      if (data != null) builder.add(data);
    }
    return utf8.decode(builder.toBytes(), allowMalformed: true);
  }
}
