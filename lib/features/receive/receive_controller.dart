import 'dart:async';

import 'package:flutter/foundation.dart';

import '../../audio/audio_input.dart';
import '../../modem/bfsk_demodulator.dart';
import '../../modem/modem_config.dart';
import '../../modem/streaming_decoder.dart';
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
      _worker = DecoderWorker();
      await _worker!.start(config);
      _eventSub = _worker!.events.listen(_onEvent);

      final stream = await _input.start(sampleRate: config.sampleRate);
      _audioSub = stream.listen(
        (chunk) => _worker?.addSamples(chunk),
        onError: (Object e) {
          _errorText = e is AudioInputException
              ? e.message
              : 'Ошибка аудио: $e';
          _micStatus = 'Ошибка микрофона';
          notifyListeners();
        },
      );

      _isReceiving = true;
      _micStatus = 'Слушаю (${config.sampleRate} Гц)';
      _lastMessageStart = DateTime.now();
      notifyListeners();
    } on AudioInputException catch (e) {
      _errorText = e.message;
      _micStatus = 'Ошибка микрофона';
      await _cleanup();
      notifyListeners();
    } catch (e) {
      _errorText = 'Не удалось запустить приём: $e';
      _micStatus = 'Ошибка';
      await _cleanup();
      notifyListeners();
    }
  }

  void _onEvent(DecoderEvent event) {
    _decoderState = event.state;
    _inputLevel = event.inputLevel;
    _diagnostics = event.diagnostics;

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
    }

    if (event.state == DecoderState.error) {
      _crcErrors++;
      _errorText = event.errorText;
    }

    notifyListeners();
  }

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
