import 'dart:typed_data';

import '../modem/modem_config.dart';

/// A single protocol packet.
///
/// On-wire layout (after the acoustic preamble, before repetition coding):
///
/// ```
/// | sync (2) | version (1) | type (1) | sequence (1) | length (2) |
/// | payload (length) | CRC16 (2) |
/// ```
///
/// The CRC is computed over every byte from `version` up to and including the
/// last payload byte (the sync word is excluded because it is only used for
/// framing/detection).
class Packet {
  Packet({
    required this.type,
    required this.sequence,
    required this.payload,
    this.version = ModemConfig.protocolVersion,
  }) : assert(
          payload.length <= 0xFFFF,
          'Payload too large for 16-bit length field',
        );

  final int version;
  final int type;
  final int sequence;
  final Uint8List payload;

  /// The bytes that participate in the CRC calculation.
  Uint8List headerAndPayload() {
    final builder = BytesBuilder();
    builder.addByte(version & 0xFF);
    builder.addByte(type & 0xFF);
    builder.addByte(sequence & 0xFF);
    builder.addByte((payload.length >> 8) & 0xFF);
    builder.addByte(payload.length & 0xFF);
    builder.add(payload);
    return builder.toBytes();
  }

  @override
  String toString() =>
      'Packet(v$version, type:$type, seq:$sequence, len:${payload.length})';
}
