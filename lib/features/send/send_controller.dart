import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';

import '../../audio/audio_output.dart';
import '../../modem/acoustic_modem.dart';
import '../../modem/modem_config.dart';
import '../../protocol/packet.dart';
import '../../shared/utils/app_logger.dart';

/// Drives text -> audio transmission through the device speaker.
class SendController extends ChangeNotifier {
  SendController({ModemConfig? config})
      : config = config ?? const ModemConfig() {
    _modem = AcousticModem(config: this.config);
  }

  final ModemConfig config;
  late final AcousticModem _modem;
  final AudioOutput _output = AudioOutput();

  bool _isSending = false;
  bool get isSending => _isSending;

  double _progress = 0;
  double get progress => _progress;

  int _packetCount = 0;
  int get packetCount => _packetCount;

  double _estimatedSeconds = 0;
  double get estimatedSeconds => _estimatedSeconds;

  String _status = 'Готов к передаче';
  String get status => _status;

  int _payloadBytes = 0;
  int get payloadBytes => _payloadBytes;

  Timer? _progressTimer;

  /// Number of UTF-8 bytes the given [text] will occupy.
  int utf8ByteCount(String text) => utf8.encode(text).length;

  /// Encodes and plays [text]. Guards against overlapping transmissions.
  Future<void> send(String text) async {
    if (_isSending) return;

    final encoded = _modem.encode(text);
    _packetCount = encoded.packets.length;
    _payloadBytes = utf8ByteCount(text);
    final totalSamples = encoded.pcm.length;
    _estimatedSeconds = totalSamples / config.sampleRate;

    _isSending = true;
    _progress = 0;
    _status = 'Передача: $_packetCount пакет(ов)…';
    notifyListeners();

    // Drive a smooth progress indicator based on elapsed playback time.
    final startedAt = DateTime.now();
    _progressTimer =
        Timer.periodic(const Duration(milliseconds: 50), (timer) {
      final elapsed =
          DateTime.now().difference(startedAt).inMilliseconds / 1000.0;
      _progress =
          _estimatedSeconds <= 0 ? 1 : (elapsed / _estimatedSeconds).clamp(0, 1);
      notifyListeners();
    });

    AppLogger.info(
      'Передача начата: $_packetCount пакет(ов), $_payloadBytes байт, '
      '~${_estimatedSeconds.toStringAsFixed(1)}с | ${config.summary}',
    );
    try {
      await _output.play(encoded.pcm, sampleRate: config.sampleRate);
      _progress = 1;
      _status = 'Передача завершена';
      AppLogger.info('Передача завершена');
    } catch (e, st) {
      _status = 'Ошибка передачи: $e';
      AppLogger.error('Ошибка передачи', e, st);
    } finally {
      _progressTimer?.cancel();
      _progressTimer = null;
      _isSending = false;
      notifyListeners();
    }
  }

  /// Stops an in-progress transmission.
  Future<void> stop() async {
    if (!_isSending) return;
    await _output.stop();
    _progressTimer?.cancel();
    _progressTimer = null;
    _isSending = false;
    _status = 'Передача остановлена';
    notifyListeners();
  }

  /// Encodes [text] to PCM without playing (used for WAV export / demo).
  EncodeResult encodeOnly(String text) => _modem.encode(text);

  /// Modulates a single packet (used by diagnostics/demo).
  Int16List modulatePacket(Packet packet) => _modem.modulatePacket(packet);

  @override
  void dispose() {
    _progressTimer?.cancel();
    _output.dispose();
    super.dispose();
  }
}
