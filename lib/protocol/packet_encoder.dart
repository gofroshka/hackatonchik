import 'dart:typed_data';

import '../modem/modem_config.dart';
import 'crc16.dart';
import 'packet.dart';

/// Serializes [Packet]s into their on-wire byte representation.
class PacketEncoder {
  const PacketEncoder();

  /// Encodes [packet] into bytes including the sync word and trailing CRC16.
  Uint8List encode(Packet packet) {
    final body = packet.headerAndPayload();
    final crc = Crc16.compute(body);

    final builder = BytesBuilder();
    builder.addByte((ModemConfig.syncWord >> 8) & 0xFF);
    builder.addByte(ModemConfig.syncWord & 0xFF);
    builder.add(body);
    builder.addByte((crc >> 8) & 0xFF);
    builder.addByte(crc & 0xFF);
    return builder.toBytes();
  }
}
