import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../audio/audio_input.dart';
import '../../modem/bfsk_demodulator.dart';
import '../../modem/modem_config.dart';
import '../../modem/streaming_decoder.dart';
import '../../shared/utils/app_logger.dart';
import '../../workers/decoder_worker.dart';

/// A message shown in the receive history.
class ReceivedMessage {
  ReceivedMessage(this.text) : timestamp = DateTime.now();
  final String text;
  final DateTime timestamp;
}

/// Drives live microphone reception and exposes decoder + diagnostic state to
/// both the Receive and Diagnostics screens.
class ReceiveController extends ChangeNotifier {
  ReceiveController({ModemConfig? config})
      : config = config ?? const ModemConfig();

  final ModemConfig config;

  final AudioInput _input = AudioInput();
  DecoderWorker? _worker;
  StreamSubscription<Float64List>? _audioSub;
  StreamSubscription<DecoderEvent>? _eventSub;

  int _activeRate = 48000;
  bool _rebuilding = false;

  bool _isReceiving = false;
  bool get isReceiving => _isReceiving;

  DecoderState _decoderState = DecoderState.idle;
  DecoderState get decoderState => _decoderState;

  double _inputLevel = 0;
  double get inputLevel => _inputLevel;

  String _micStatus = 'Микрофон не активен';
  String get micStatus => _micStatus;

  String _receivedText = '';
  String get receivedText => _receivedText;

  String? _errorText;
  String? get errorText => _errorText;

  DemodDiagnostics _diagnostics = const DemodDiagnostics();
  DemodDiagnostics get diagnostics => _diagnostics;

  int _totalPackets = 0;
  int get totalPackets => _totalPackets;

  int _totalBits = 0;
  int get totalBits => _totalBits;

  int _crcErrors = 0;
  int get crcErrors => _crcErrors;

  double _measuredBitRate = 0;
  double get measuredBitRate => _measuredBitRate;

  final List<ReceivedMessage> _history = [];
  List<ReceivedMessage> get history => List.unmodifiable(_history);

  static const int _energyHistoryLength = 120;
  final List<double> _energyHistory =
      List<double>.filled(_energyHistoryLength, 0, growable: true);
  List<double> get energyHistory => List.unmodifiable(_energyHistory);

  DateTime? _lastMessageStart;

  /// Starts microphone capture and decoding.
  Future<void> start() async {
    if (_isReceiving) return;
    _errorText = null;

    final allowed = await _input.hasPermission();
    if (!allowed) {
      _micStatus = 'Нет разрешения на микрофон';
      _errorText =
          'Приложению не выдано разрешение на микрофон. Разрешите доступ в настройках.';
      notifyListeners();
      return;
    }

    try {
      _activeRate = config.sampleRate;
      await _startWorker(_activeRate);

      final stream = await _input.start(
        sampleRate: config.sampleRate,
        onSampleRateChanged: _handleRateChange,
      );
      _audioSub = stream.listen(
        (chunk) => _worker?.addSamples(chunk),
        onError: (Object e) {
          _errorText = e is AudioInputException
              ? e.message
              : 'Ошибка аудио: $e';
          _micStatus = 'Ошибка микрофона';
          AppLogger.error('Ошибка аудиопотока', e);
          notifyListeners();
        },
      );

      _isReceiving = true;
      _micStatus = 'Слушаю ($_activeRate Гц)';
      _lastMessageStart = DateTime.now();
      AppLogger.info('Приём запущен ($_activeRate Гц) | ${config.summary}');
      notifyListeners();
    } on AudioInputException catch (e, st) {
      _errorText = e.message;
      _micStatus = 'Ошибка микрофона';
      AppLogger.error('Не удалось запустить приём (микрофон)', e, st);
      await _cleanup();
      notifyListeners();
    } catch (e, st) {
      _errorText = 'Не удалось запустить приём: $e';
      _micStatus = 'Ошибка';
      AppLogger.error('Не удалось запустить приём', e, st);
      await _cleanup();
      notifyListeners();
    }
  }

  Future<void> _startWorker(int rate) async {
    _worker = DecoderWorker();
    await _worker!.start(config.copyWith(sampleRate: rate));
    _eventSub = _worker!.events.listen(_onEvent);
  }

  /// Rebuilds the decoder if the platform reports a different actual capture
  /// rate than the one we requested (some devices force 44.1/48 kHz).
  Future<void> _handleRateChange(int rate) async {
    if (rate == _activeRate || _rebuilding || !_isReceiving) return;
    _rebuilding = true;
    _activeRate = rate;
    _micStatus = 'Слушаю ($rate Гц, адаптировано)';
    notifyListeners();
    try {
      await _eventSub?.cancel();
      _eventSub = null;
      await _worker?.dispose();
      _worker = null;
      await _startWorker(rate);
    } finally {
      _rebuilding = false;
    }
  }

  void _onEvent(DecoderEvent event) {
    _decoderState = event.state;
    _inputLevel = event.inputLevel;

    // Level "ticks" carry only noise-floor/SNR and zeroed decode fields. Keep
    // the last real decode values sticky so the diagnostics screen stays
    // meaningful instead of flashing back to zero between transmissions.
    final d = event.diagnostics;
    final isRealDecode =
        d.offset >= 0 || d.bitCount > 0 || d.energy0 > 0 || d.energy1 > 0;
    if (isRealDecode) {
      _diagnostics = d;
    } else {
      _diagnostics = DemodDiagnostics(
        energy0: _diagnostics.energy0,
        energy1: _diagnostics.energy1,
        noiseFloor: d.noiseFloor,
        snr: d.snr,
        confidence: _diagnostics.confidence,
        offset: _diagnostics.offset,
        bitCount: _diagnostics.bitCount,
        packetCount: _diagnostics.packetCount,
        correctedBits: _diagnostics.correctedBits,
        preambleScore: _diagnostics.preambleScore,
        crcOk: _diagnostics.crcOk,
      );
    }

    _pushEnergy(event.inputLevel);

    if (event.diagnostics.bitCount > 0) {
      _totalBits += event.diagnostics.bitCount;
    }

    if (event.state == DecoderState.messageReceived && event.message != null) {
      final text = event.message!.text;
      _receivedText = text;
      _history.insert(0, ReceivedMessage(text));
      if (_history.length > 50) _history.removeLast();
      _totalPackets += event.message!.packetCount;

      final elapsed = _lastMessageStart == null
          ? null
          : DateTime.now().difference(_lastMessageStart!).inMilliseconds;
      if (elapsed != null && elapsed > 0) {
        _measuredBitRate = event.message!.text.length * 8 * 1000 / elapsed;
      }
      _lastMessageStart = DateTime.now();
      AppLogger.info(
        'Сообщение принято: "${_truncate(text)}" '
        '(пакетов=${event.message!.packetCount}, ${_diagString(d)})',
      );
    }

    // Only act on *real* error events (which carry errorText). The decoder's
    // error state is sticky and re-emitted on every level tick, so gating on
    // errorText avoids spamming logs and over-counting CRC errors.
    if (event.state == DecoderState.error && event.errorText != null) {
      _crcErrors++;
      _errorText = event.errorText;
      AppLogger.error(
        'Ошибка приёма: ${event.errorText} '
        '(${_diagString(d)}, всего ошибок=$_crcErrors)',
      );
    }

    notifyListeners();
  }

  String _diagString(DemodDiagnostics d) =>
      'SNR=${d.snr.toStringAsFixed(1)} '
      'e0=${d.energy0.toStringAsFixed(0)} e1=${d.energy1.toStringAsFixed(0)} '
      'noise=${d.noiseFloor.toStringAsFixed(4)} '
      'preamble=${d.preambleScore.toStringAsFixed(2)} '
      'conf=${d.confidence.toStringAsFixed(2)} '
      'bits=${d.bitCount} corrected=${d.correctedBits} crcOk=${d.crcOk}';

  String _truncate(String s, [int max = 40]) =>
      s.length <= max ? s : '${s.substring(0, max)}…';

  void _pushEnergy(double value) {
    _energyHistory.add(value);
    while (_energyHistory.length > _energyHistoryLength) {
      _energyHistory.removeAt(0);
    }
  }

  /// Stops capture and decoding.
  Future<void> stop() async {
    if (!_isReceiving) return;
    await _cleanup();
    _isReceiving = false;
    _decoderState = DecoderState.idle;
    _micStatus = 'Микрофон не активен';
    _inputLevel = 0;
    notifyListeners();
  }

  /// Clears the received text and history.
  void clearHistory() {
    _history.clear();
    _receivedText = '';
    notifyListeners();
  }

  Future<void> _cleanup() async {
    await _audioSub?.cancel();
    _audioSub = null;
    await _eventSub?.cancel();
    _eventSub = null;
    await _input.stop();
    await _worker?.dispose();
    _worker = null;
  }

  @override
  void dispose() {
    _cleanup();
    _input.dispose();
    super.dispose();
  }
}
