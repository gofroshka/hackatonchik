import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:hackatonchik/modem/modem_config.dart';
import 'package:hackatonchik/protocol/packet.dart';
import 'package:hackatonchik/protocol/packet_decoder.dart';
import 'package:hackatonchik/protocol/packet_encoder.dart';

void main() {
  const encoder = PacketEncoder();
  const decoder = PacketDecoder();

  Packet makePacket(List<int> payload) => Packet(
        type: PacketType.text,
        sequence: 7,
        payload: Uint8List.fromList(payload),
      );

  group('Packet encode/decode', () {
    test('valid packet round trips and passes CRC', () {
      final packet = makePacket([1, 2, 3, 4, 5]);
      final bytes = encoder.encode(packet);
      final result = decoder.decode(bytes);

      expect(result.status, PacketDecodeStatus.ok);
      expect(result.packet!.type, PacketType.text);
      expect(result.packet!.sequence, 7);
      expect(result.packet!.payload, [1, 2, 3, 4, 5]);
    });

    test('sync word can be found with a leading offset', () {
      final packet = makePacket([9, 9, 9]);
      final bytes = encoder.encode(packet);
      final noisy = Uint8List.fromList([0x00, 0xFF, 0x12, ...bytes]);
      final result = decoder.decode(noisy);
      expect(result.status, PacketDecodeStatus.ok);
      expect(result.packet!.payload, [9, 9, 9]);
    });

    test('flipped payload bit is rejected by CRC', () {
      final packet = makePacket([10, 20, 30]);
      final bytes = encoder.encode(packet);
      // Flip a bit inside the payload region.
      bytes[8] ^= 0x01;
      final result = decoder.decode(bytes);
      expect(result.status, PacketDecodeStatus.crcError);
    });

    test('truncated packet reports needMoreData', () {
      final packet = makePacket([1, 2, 3, 4]);
      final bytes = encoder.encode(packet);
      final truncated = bytes.sublist(0, bytes.length - 3);
      final result = decoder.decode(truncated);
      expect(result.status, PacketDecodeStatus.needMoreData);
    });

    test('implausible length is not accepted as a packet', () {
      // sync + a bogus huge length, nothing else valid.
      final bogus = Uint8List.fromList([
        (ModemConfig.syncWord >> 8) & 0xFF,
        ModemConfig.syncWord & 0xFF,
        1, // version
        1, // type
        0, // seq
        0xFF, 0xFF, // length = 65535
      ]);
      final result = decoder.decode(bogus);
      expect(result.status, isNot(PacketDecodeStatus.ok));
    });
  });
}
