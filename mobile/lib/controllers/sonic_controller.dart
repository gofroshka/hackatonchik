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
  // false = fast OFDM link; true = slow but very robust Robust-FSK link.
  bool reliableMode = false;
  ReceivedTransfer? received;
  final List<ChatMessage> _chatMessages = [];
  TxSession? _tx;
  RxSession? _rx;

  List<ChatMessage> get chatMessages => List.unmodifiable(_chatMessages);

  bool get isBusy =>
      phase == TransferPhase.sending || phase == TransferPhase.listening;

  void setReliableMode(bool value) {
    if (isBusy || reliableMode == value) return;
    reliableMode = value;
    notifyListeners();
  }

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
    phase = TransferPhase.preparing;
    status = 'Подготовка FEC и метаданных…';
    error = null;
    notifyListeners();
    try {
      _tx = await TxSession.fromFile(
        path: file!.path!,
        reliable: reliableMode,
      );
      txInfo = await _tx!.info();
      packetCount = txInfo!.packetCount;
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
      if (phase != TransferPhase.error) {
        phase = TransferPhase.completed;
        progress = 1;
        status = 'Звук передачи завершён';
        notifyListeners();
      }
    } catch (exception) {
      _setError(exception.toString());
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

  Future<void> _startListening() async {
    try {
      final documents = await getApplicationDocumentsDirectory();
      final output = '${documents.path}/Sonic Share';
      _rx = await RxSession.newInstance(
        sampleRate: acousticSampleRate,
        outputDir: output,
        reliable: reliableMode,
      );
      phase = TransferPhase.listening;
      status = mode == SonicMode.chat
          ? 'Слушаю чат через микрофон…'
          : 'Слушаю акустический канал…';
      error = null;
      progress = 0;
      microphoneLevel = 0;
      received = null;
      notifyListeners();
      await _audio.receive(
        session: _rx!,
        onEvents: _handleReceiveEvents,
        onLevel: (level) {
          microphoneLevel = level;
          notifyListeners();
        },
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
        reliable: reliableMode,
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
        case 'started':
          packetIndex = 0;
          completedGroups = 0;
          totalGroups = event.totalGroups ?? 0;
          status = _isChatContentType(event.contentType)
              ? 'Принимаю сообщение…'
              : 'Найден файл: ${event.name ?? 'без имени'}';
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
          phase = TransferPhase.completed;
          progress = 1;
          status = 'Файл принят и SHA-256 проверен';
          unawaited(_audio.stop());
        case 'failed':
          _setError(event.message ?? 'Ошибка приёма');
      }
    }
    notifyListeners();
  }

  Future<void> stop() async {
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
