import 'package:flutter_test/flutter_test.dart';
import 'package:hackatonchik/modem/modem_config.dart';
import 'package:hackatonchik/protocol/message_assembler.dart';
import 'package:hackatonchik/protocol/packet_fragmenter.dart';

void main() {
  final config = const ModemConfig();
  final fragmenter = PacketFragmenter(config);

  group('Fragmentation + reassembly', () {
    test('short message is a single packet', () {
      final packets = fragmenter.fragment('HELLO', messageId: 1);
      expect(packets.length, 1);
    });

    test('long message is split across packets and reassembles', () {
      final message = 'X' * 400;
      final packets = fragmenter.fragment(message, messageId: 5);
      expect(packets.length, greaterThan(1));

      final assembler = MessageAssembler();
      AssembledMessage? result;
      for (final packet in packets) {
        result = assembler.addPacket(packet) ?? result;
      }
      expect(result, isNotNull);
      expect(result!.text, message);
    });

    test('out-of-order fragments still reassemble', () {
      final message = 'ABCDEFG' * 40;
      final packets = fragmenter.fragment(message, messageId: 9);
      final assembler = MessageAssembler();
      AssembledMessage? result;
      for (final packet in packets.reversed) {
        result = assembler.addPacket(packet) ?? result;
      }
      expect(result!.text, message);
    });

    test('empty message produces one packet that reassembles to empty', () {
      final packets = fragmenter.fragment('', messageId: 0);
      expect(packets.length, 1);
      final assembler = MessageAssembler();
      final result = assembler.addPacket(packets.first);
      expect(result!.text, '');
    });
  });
}
