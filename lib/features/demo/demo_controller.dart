import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';

import '../../audio/audio_output.dart';
import '../../audio/pcm_converter.dart';
import '../../modem/acoustic_modem.dart';
import '../../modem/modem_config.dart';
import '../../shared/utils/wav_utils.dart';

/// Demo mode: encodes a fixed message, visualizes the pipeline, exports and
/// re-imports WAV, and can run entirely on a single device.
class DemoController extends ChangeNotifier {
  DemoController({ModemConfig? config})
      : config = config ?? const ModemConfig() {
    _modem = AcousticModem(config: this.config);
    _rebuild();
  }

  static const String demoMessage = 'HELLO FROM AUDIO';

  final ModemConfig config;
  late final AcousticModem _modem;
  final AudioOutput _output = AudioOutput();

  EncodeResult? _encoded;
  int _bitCount = 0;
  int _byteCount = 0;
  double _durationSeconds = 0;

  int get packetCount => _encoded?.packets.length ?? 0;
  int get bitCount => _bitCount;
  int get byteCount => _byteCount;
  double get durationSeconds => _durationSeconds;

  String _status = 'Готово';
  String get status => _status;

  String? _decodedResult;
  String? get decodedResult => _decodedResult;

  bool _busy = false;
  bool get busy => _busy;

  void _rebuild() {
    final encoded = _modem.encode(demoMessage, messageId: 0);
    _encoded = encoded;
    _durationSeconds = encoded.pcm.length / config.sampleRate;

    int codedBits = 0;
    int bytes = 0;
    for (final packet in encoded.packets) {
      final headerAndPayload = packet.headerAndPayload();
      final onWireBytes = headerAndPayload.length + 4; // + sync(2) + crc(2)
      bytes += onWireBytes;
      codedBits += onWireBytes * 8 * config.repetitionFactor +
          config.preambleBits;
    }
    _byteCount = bytes;
    _bitCount = codedBits;
  }

  /// Plays the demo signal through the speaker.
  Future<void> play() async {
    final encoded = _encoded;
    if (encoded == null || _busy) return;
    _busy = true;
    _status = 'Воспроизведение…';
    notifyListeners();
    try {
      await _output.play(encoded.pcm, sampleRate: config.sampleRate);
      _status = 'Воспроизведение завершено';
    } catch (e) {
      _status = 'Ошибка воспроизведения: $e';
    } finally {
      _busy = false;
      notifyListeners();
    }
  }

  /// Saves the generated signal to a WAV file in the app documents directory.
  Future<String?> saveWav() async {
    final encoded = _encoded;
    if (encoded == null) return null;
    _busy = true;
    notifyListeners();
    try {
      final wav = WavUtils.encode(encoded.pcm, sampleRate: config.sampleRate);
      final dir = await getApplicationDocumentsDirectory();
      final path = '${dir.path}/demo_signal.wav';
      final file = File(path);
      await file.writeAsBytes(wav, flush: true);
      _status = 'WAV сохранён: $path';
      return path;
    } catch (e) {
      _status = 'Ошибка сохранения WAV: $e';
      return null;
    } finally {
      _busy = false;
      notifyListeners();
    }
  }

  /// Lets the user pick a WAV file and decodes it back to text.
  Future<void> importAndDecodeWav() async {
    _busy = true;
    _status = 'Выбор файла…';
    _decodedResult = null;
    notifyListeners();
    try {
      final result = await FilePicker.pickFiles(
        type: FileType.custom,
        allowedExtensions: ['wav'],
        withData: true,
      );
      if (result == null || result.files.isEmpty) {
        _status = 'Файл не выбран';
        return;
      }
      final file = result.files.first;
      final bytes = file.bytes ??
          (file.path != null
              ? await File(file.path!).readAsBytes()
              : null);
      if (bytes == null) {
        _status = 'Не удалось прочитать файл';
        return;
      }
      final decoded = _decodeWavBytes(Uint8List.fromList(bytes));
      _decodedResult = decoded ?? '(сообщение не распознано)';
      _status = decoded != null
          ? 'Декодировано из WAV'
          : 'CRC не прошёл / сигнал не распознан';
    } catch (e) {
      _status = 'Ошибка импорта WAV: $e';
    } finally {
      _busy = false;
      notifyListeners();
    }
  }

  /// Decodes the last saved demo WAV (round-trip test on a single device).
  Future<void> decodeGeneratedSignal() async {
    final encoded = _encoded;
    if (encoded == null) return;
    _busy = true;
    notifyListeners();
    final samples = PcmConverter.int16ToFloat(encoded.pcm);
    final report = _modem.decodeAll(samples);
    _decodedResult = report.firstMessage ?? '(не распознано)';
    _status = report.firstMessage != null
        ? 'Loopback успешен'
        : 'Loopback не удался';
    _busy = false;
    notifyListeners();
  }

  String? _decodeWavBytes(Uint8List bytes) {
    final wav = WavUtils.decode(bytes);
    final samples = PcmConverter.int16ToFloat(wav.samples);
    // Decode using the WAV's own sample rate if it matches the modem config.
    final modem = wav.sampleRate == config.sampleRate
        ? _modem
        : AcousticModem(config: config.copyWith(sampleRate: wav.sampleRate));
    return modem.decodeAll(samples).firstMessage;
  }

  @override
  void dispose() {
    _output.dispose();
    super.dispose();
  }
}
