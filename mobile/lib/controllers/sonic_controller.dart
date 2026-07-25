import 'dart:async';
import 'dart:convert';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:path_provider/path_provider.dart';
import 'package:sonic_share/models/transfer_models.dart';
import 'package:sonic_share/services/acoustic_audio_service.dart';
import 'package:sonic_share/src/rust/api/rx.dart';
import 'package:sonic_share/src/rust/api/tx.dart';
import 'package:sonic_share/src/rust/api/types.dart';

class SonicController extends ChangeNotifier {
  SonicController(this._audio);

  final AcousticAudioService _audio;
  SonicMode mode = SonicMode.send;
  TransferPhase phase = TransferPhase.idle;
  PlatformFile? selectedFile;
  TxInfo? txInfo;
  double progress = 0;
  double microphoneLevel = 0;
  int packetIndex = 0;
  int packetCount = 0;
  int completedGroups = 0;
  int totalGroups = 0;
  String status = 'Готов к передаче';
  String? error;
  ReceivedTransfer? received;
  final List<ChatMessage> _chatMessages = [];
  TxSession? _tx;
  RxSession? _rx;
  bool _receivingData = false;
  bool _awaitingData = false;

  List<ChatMessage> get chatMessages => List.unmodifiable(_chatMessages);

  bool get isBusy =>
      phase == TransferPhase.sending ||
      phase == TransferPhase.listening ||
      phase == TransferPhase.handshaking ||
      phase == TransferPhase.handshakeWaitAck ||
      phase == TransferPhase.endWaitAck;

  Future<void> setMode(SonicMode value) async {
    if (mode == value) return;
    if (isBusy || phase == TransferPhase.preparing) {
      await stop();
    }
    mode = value;
    phase = TransferPhase.idle;
    status = switch (value) {
      SonicMode.send => 'Выберите файл для отправки',
      SonicMode.receive => 'Нажмите «Начать приём»',
      SonicMode.chat => 'Нажмите «Слушать чат»',
    };
    notifyListeners();
  }

  Future<void> pickFile() async {
    final result = await FilePicker.platform.pickFiles(
      allowMultiple: false,
      withData: false,
    );
    if (result == null) return;
    final file = result.files.single;
    if (file.path == null) {
      _setError('Платформа не предоставила локальный путь к файлу');
      return;
    }
    selectedFile = file;
    txInfo = null;
    progress = 0;
    phase = TransferPhase.idle;
    status = 'Файл выбран';
    error = null;
    notifyListeners();
  }

  Future<void> startSending() async {
    final file = selectedFile;
    if (file?.path == null || isBusy) return;
    _cancelled = false;
    phase = TransferPhase.preparing;
    status = 'Подготовка FEC и метаданных…';
    error = null;
    notifyListeners();
    try {
      _tx = await TxSession.fromFile(path: file!.path!);
      txInfo = await _tx!.info();
      packetCount = txInfo!.packetCount;

      await _doHandshake();
      if (_cancelled) return;

      phase = TransferPhase.sending;
      status = 'Передача через динамик';
      notifyListeners();
      await _audio.send(
        session: _tx!,
        onProgress: (chunk) {
          progress = chunk.progress;
          packetIndex = chunk.packetIndex;
          packetCount = chunk.packetCount;
          notifyListeners();
        },
      );
      if (_cancelled) return;

      await _doEndHandshake();
      if (_cancelled) return;

      phase = TransferPhase.completed;
      progress = 1;
      status = 'Передача завершена';
      notifyListeners();
    } catch (exception) {
      if (!_cancelled) {
        _setError(exception.toString());
      }
    }
  }

  Future<void> _doHandshake() async {
    final requestPcm = await handshakeRequestPcm(id: BigInt.zero);

    for (var attempt = 0; attempt < 8; attempt++) {
      if (_cancelled) return;

      phase = TransferPhase.handshaking;
      status = 'Рукопожатие: попытка ${attempt + 1}';
      notifyListeners();

      await _audio.prepareForPlayback();
      if (_cancelled) return;
      await _audio.playPcmTight(requestPcm);
      if (_cancelled) return;

      phase = TransferPhase.handshakeWaitAck;
      status = 'Ожидание ответа…';
      notifyListeners();

      final recorded = await _audio.recordShort(const Duration(milliseconds: 1200));
      if (_cancelled) return;
      if (recorded != null && recorded.isNotEmpty) {
        final events = await checkPcmForHandshake(
          pcm16Le: recorded,
          sampleRate: acousticSampleRate,
        );
        if (_cancelled) return;
        for (final event in events) {
          if (event.kind == 'handshake_ack') {
            await _audio.prepareForPlayback();
            status = 'Связь установлена!';
            notifyListeners();
            return;
          }
        }
      }
    }
    if (!_cancelled) {
      throw 'Не удалось выполнить рукопожатие. Убедитесь, что приёмник включён и находится рядом.';
    }
  }

  Future<void> _doEndHandshake() async {
    if (_cancelled) return;
    await Future.delayed(const Duration(milliseconds: 300));
    if (_cancelled) return;

    for (var attempt = 0; attempt < 5; attempt++) {
      if (_cancelled) return;

      phase = TransferPhase.endWaitAck;
      status = 'Подтверждение получения…';
      notifyListeners();

      final recorded = await _audio.recordShort(const Duration(milliseconds: 600));
      if (_cancelled) return;
      if (recorded != null && recorded.isNotEmpty) {
        final events = await checkPcmForHandshake(
          pcm16Le: recorded,
          sampleRate: acousticSampleRate,
        );
        if (_cancelled) return;
        for (final event in events) {
          if (event.kind == 'end_ack') {
            status = 'Подтверждено!';
            notifyListeners();
            return;
          }
        }
      }
    }
    if (!_cancelled) {
      status = 'Файл отправлен (без подтверждения)';
      notifyListeners();
    }
  }

  Future<void> startReceiving() async {
    if (isBusy) return;
    await _startListening();
  }

  Future<void> startChatListening() async {
    if (mode != SonicMode.chat || isBusy) return;
    await _startListening();
  }

  Future<void> _startListening({bool? checkHandshake}) async {
    try {
      final documents = await getApplicationDocumentsDirectory();
      final output = '${documents.path}/Sonic Share';
      _rx = await RxSession.newInstance(
        sampleRate: acousticSampleRate,
        outputDir: output,
      );
      _cancelled = false;
      _receivingData = false;
      _awaitingData = false;
      phase = TransferPhase.listening;
      status = mode == SonicMode.chat
          ? 'Слушаю чат через микрофон…'
          : 'Слушаю акустический канал…';
      error = null;
      progress = 0;
      microphoneLevel = 0;
      received = null;
      notifyListeners();
      final doCheckHandshake = checkHandshake ?? (mode == SonicMode.receive);
      await _audio.receive(
        session: _rx!,
        onEvents: _handleReceiveEvents,
        onLevel: (level) {
          microphoneLevel = level;
          notifyListeners();
        },
        checkHandshake: doCheckHandshake,
      );
    } catch (exception) {
      _setError(exception.toString());
    }
  }

  Future<void> sendChatMessage(String value) async {
    final text = value.trim();
    if (mode != SonicMode.chat || text.isEmpty) return;
    if (phase == TransferPhase.sending || phase == TransferPhase.preparing) {
      return;
    }

    if (phase == TransferPhase.listening) {
      await _audio.stop();
      _rx = null;
    }

    final now = DateTime.now();
    final message = ChatMessage(
      id: 'local-${now.microsecondsSinceEpoch}',
      text: text,
      outgoing: true,
      createdAt: now,
      state: ChatMessageState.sending,
    );
    _chatMessages.add(message);
    phase = TransferPhase.preparing;
    status = 'Кодирую сообщение…';
    progress = 0;
    error = null;
    notifyListeners();

    try {
      _tx = await TxSession.fromData(
        name: 'chat-${now.millisecondsSinceEpoch}.txt',
        contentType: 'text/x-sonic-chat; charset=utf-8',
        data: utf8.encode(text),
      );
      txInfo = await _tx!.info();
      packetCount = txInfo!.packetCount;
      phase = TransferPhase.sending;
      status = 'Сообщение звучит в эфире';
      notifyListeners();
      await _audio.send(
        session: _tx!,
        onProgress: (chunk) {
          progress = chunk.progress;
          packetIndex = chunk.packetIndex;
          packetCount = chunk.packetCount;
          notifyListeners();
        },
      );
      if (phase != TransferPhase.sending) return;
      _setChatMessageState(message.id, ChatMessageState.sent);
      _tx = null;
      phase = TransferPhase.idle;
      progress = 0;
      status = 'Сообщение передано в эфир';
      notifyListeners();
      await startChatListening();
    } catch (exception) {
      _setChatMessageState(message.id, ChatMessageState.failed);
      _setError(exception.toString());
    }
  }

  void _handleReceiveEvents(List<MobileReceiveEvent> events) {
    for (final event in events) {
      switch (event.kind) {
        case 'handshake_request':
          if (!_cancelled && mode == SonicMode.receive && !_receivingData && !_awaitingData) {
            _awaitingData = true;
            status = 'Обнаружен запрос связи, отвечаю…';
            notifyListeners();
            _respondHandshake(event.id, isEndAck: false);
          }
        case 'handshake_ack':
          status = 'Рукопожатие подтверждено';
        case 'end_ack':
          status = 'Передача подтверждена получателем';
        case 'started':
          if (_isChatContentType(event.contentType)) {
            status = 'Принимаю сообщение…';
            notifyListeners();
            continue;
          }
          _awaitingData = false;
          _receivingData = true;
          packetIndex = 0;
          completedGroups = 0;
          totalGroups = event.totalGroups ?? 0;
          status = 'Найден файл: ${event.name ?? 'без имени'}';
        case 'progress':
          completedGroups = event.completedGroups ?? completedGroups;
          totalGroups = event.totalGroups ?? totalGroups;
          progress = totalGroups == 0 ? 0 : completedGroups / totalGroups;
          status = 'Восстановление FEC-групп';
        case 'completed':
          if (_isChatContentType(event.contentType)) {
            final text = event.text;
            if (text != null && text.isNotEmpty) {
              _chatMessages.add(
                ChatMessage(
                  id: event.id,
                  text: text,
                  outgoing: false,
                  createdAt: DateTime.now(),
                  state: ChatMessageState.sent,
                ),
              );
            }
            progress = 0;
            if (mode == SonicMode.chat) {
              phase = TransferPhase.listening;
              status = 'Сообщение принято · слушаю дальше';
            } else {
              phase = TransferPhase.completed;
              status = 'Получено сообщение · откройте режим «Чат»';
              unawaited(_audio.stop());
            }
            continue;
          }
          received = ReceivedTransfer(
            name: event.name ?? 'received.bin',
            contentType: event.contentType ?? 'application/octet-stream',
            path: event.path ?? '',
            size: event.originalSize?.toInt() ?? 0,
            text: event.text,
          );
          _receivingData = false;
          _awaitingData = false;
          phase = TransferPhase.completed;
          progress = 1;
          status = 'Файл принят и SHA-256 проверен';
          _respondHandshake(event.id, isEndAck: true);
          unawaited(_audio.stop());
        case 'failed':
          _receivingData = false;
          _awaitingData = false;
          _setError(event.message ?? 'Ошибка приёма');
      }
    }
    notifyListeners();
  }

  void _respondHandshake(String idHex, {required bool isEndAck}) {
    unawaited(_doRespondHandshake(idHex, isEndAck: isEndAck));
  }

  Future<void> _doRespondHandshake(String idHex, {required bool isEndAck}) async {
    try {
      if (_cancelled || phase == TransferPhase.error) return;
      await _audio.stop();
      if (!isEndAck) {
        if (_cancelled) return;
        await Future.delayed(const Duration(milliseconds: 500));
      }
      if (_cancelled) return;
      final id = BigInt.parse(idHex, radix: 16);
      final pcm = isEndAck
          ? await endAckPcm(id: id)
          : await handshakeAckPcm(id: id);
      await _audio.prepareForPlayback();
      await _audio.playShortPcm(pcm);
      if (_cancelled) return;
      if (!isEndAck || mode == SonicMode.chat) {
        await _startListening(checkHandshake: false);
      } else if (!_cancelled) {
        notifyListeners();
      }
    } catch (e) {
      _setError('Handshake error: $e');
    }
  }

  bool _cancelled = false;

  Future<void> stop() async {
    _cancelled = true;
    _awaitingData = false;
    if (mode == SonicMode.chat &&
        (phase == TransferPhase.sending || phase == TransferPhase.preparing)) {
      final sending = _chatMessages.lastWhere(
        (message) => message.state == ChatMessageState.sending,
      );
      _setChatMessageState(sending.id, ChatMessageState.failed);
    }
    if (_tx != null) await _tx!.cancel();
    await _audio.stop();
    _tx = null;
    _rx = null;
    if (isBusy || phase == TransferPhase.preparing) {
      phase = TransferPhase.idle;
      status = switch (mode) {
        SonicMode.send => 'Передача остановлена',
        SonicMode.receive => 'Приём остановлен',
        SonicMode.chat => 'Чат приостановлен',
      };
      notifyListeners();
    }
  }

  void reset() {
    _cancelled = false;
    phase = TransferPhase.idle;
    progress = 0;
    packetIndex = 0;
    completedGroups = 0;
    totalGroups = 0;
    error = null;
    received = null;
    status = switch (mode) {
      SonicMode.send => 'Выберите файл для отправки',
      SonicMode.receive => 'Нажмите «Начать приём»',
      SonicMode.chat => 'Нажмите «Слушать чат»',
    };
    notifyListeners();
  }

  void _setChatMessageState(String id, ChatMessageState state) {
    final index = _chatMessages.indexWhere((message) => message.id == id);
    if (index >= 0) {
      _chatMessages[index] = _chatMessages[index].withState(state);
    }
  }

  void _setError(String message) {
    error = message.replaceFirst('Exception: ', '');
    phase = TransferPhase.error;
    status = 'Передача прервана';
    notifyListeners();
  }

  @override
  void dispose() {
    unawaited(_audio.dispose());
    super.dispose();
  }
}

bool _isChatContentType(String? contentType) =>
    contentType?.split(';').first.trim().toLowerCase() == 'text/x-sonic-chat';
