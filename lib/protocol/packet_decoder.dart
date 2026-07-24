import 'dart:typed_data';

import '../modem/modem_config.dart';
import 'crc16.dart';
import 'packet.dart';

/// Status of a packet decode attempt.
enum PacketDecodeStatus {
  /// No sync word was found in the provided bytes.
  noSync,

  /// The sync word was found but there are not enough bytes yet to hold the
  /// full packet described by the length field.
  needMoreData,

  /// The packet was fully parsed but its CRC did not match.
  crcError,

  /// The packet was parsed and its CRC is valid.
  ok,
}

/// Result of attempting to decode a packet out of a byte buffer.
class PacketDecodeResult {
  const PacketDecodeResult({
    required this.status,
    this.packet,
    this.syncOffset = -1,
    this.consumed = 0,
    this.computedCrc,
    this.receivedCrc,
  });

  final PacketDecodeStatus status;
  final Packet? packet;

  /// Index into the input where the sync word was located (-1 if none).
  final int syncOffset;

  /// Number of bytes consumed from [syncOffset] for a full packet (header +
  /// payload + CRC). Used by callers to advance their buffer.
  final int consumed;

  final int? computedCrc;
  final int? receivedCrc;

  bool get isOk => status == PacketDecodeStatus.ok;
}

/// Parses raw bytes (already demodulated + error-corrected) into [Packet]s.
class PacketDecoder {
  const PacketDecoder();

  static const int _minPacketBytes = 2 + 5 + 2; // sync + header + crc

  /// Scans [bytes] starting at [start] for the sync word and attempts to
  /// decode a single packet.
  PacketDecodeResult decode(Uint8List bytes, {int start = 0}) {
    final syncOffset = _findSync(bytes, start);
    if (syncOffset < 0) {
      return const PacketDecodeResult(status: PacketDecodeStatus.noSync);
    }

    // Need at least the fixed header to read the length field.
    const headerStart = 2; // relative to sync
    if (syncOffset + headerStart + 5 > bytes.length) {
      return PacketDecodeResult(
        status: PacketDecodeStatus.needMoreData,
        syncOffset: syncOffset,
      );
    }

    final version = bytes[syncOffset + 2];
    final type = bytes[syncOffset + 3];
    final sequence = bytes[syncOffset + 4];
    final length = (bytes[syncOffset + 5] << 8) | bytes[syncOffset + 6];

    if (length > ModemConfig().maxPayloadLength) {
      // Length is implausible; treat the sync as spurious and keep searching
      // after it.
      final next = decode(bytes, start: syncOffset + 2);
      if (next.status != PacketDecodeStatus.noSync) return next;
      return PacketDecodeResult(
        status: PacketDecodeStatus.crcError,
        syncOffset: syncOffset,
      );
    }

    final totalBytes = 2 + 5 + length + 2;
    if (syncOffset + totalBytes > bytes.length) {
      return PacketDecodeResult(
        status: PacketDecodeStatus.needMoreData,
        syncOffset: syncOffset,
      );
    }

    final payload = Uint8List.fromList(
      bytes.sublist(syncOffset + 7, syncOffset + 7 + length),
    );
    final receivedCrc = (bytes[syncOffset + 7 + length] << 8) |
        bytes[syncOffset + 7 + length + 1];

    final packet = Packet(
      version: version,
      type: type,
      sequence: sequence,
      payload: payload,
    );
    final computedCrc = Crc16.compute(packet.headerAndPayload());

    if (computedCrc != receivedCrc) {
      return PacketDecodeResult(
        status: PacketDecodeStatus.crcError,
        syncOffset: syncOffset,
        consumed: totalBytes,
        computedCrc: computedCrc,
        receivedCrc: receivedCrc,
      );
    }

    return PacketDecodeResult(
      status: PacketDecodeStatus.ok,
      packet: packet,
      syncOffset: syncOffset,
      consumed: totalBytes,
      computedCrc: computedCrc,
      receivedCrc: receivedCrc,
    );
  }

  int _findSync(Uint8List bytes, int start) {
    final hi = (ModemConfig.syncWord >> 8) & 0xFF;
    final lo = ModemConfig.syncWord & 0xFF;
    for (int i = start; i + 1 < bytes.length; i++) {
      if (bytes[i] == hi && bytes[i + 1] == lo) {
        if (i + _minPacketBytes <= bytes.length ||
            i + 2 + 5 <= bytes.length) {
          return i;
        }
      }
    }
    return -1;
  }
}
