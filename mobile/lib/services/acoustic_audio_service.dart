import 'dart:async';
import 'dart:typed_data';

import 'package:audio_session/audio_session.dart';
import 'package:flutter_sound/flutter_sound.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:sonic_share/src/rust/api/rx.dart';
import 'package:sonic_share/src/rust/api/tx.dart';
import 'package:sonic_share/src/rust/api/types.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

const acousticSampleRate = 48000;

class AcousticAudioService {
  final FlutterSoundPlayer _player = FlutterSoundPlayer();
  final FlutterSoundRecorder _recorder = FlutterSoundRecorder();

  StreamController<Uint8List>? _recordController;
  StreamSubscription<Uint8List>? _recordSubscription;
  Future<void> _decodeQueue = Future.value();
  bool _cancelled = false;
  bool _recorderOpen = false;

  Future<void> send({
    required TxSession session,
    required void Function(TxChunk chunk) onProgress,
  }) async {
    _cancelled = false;
    await WakelockPlus.enable();
    final audioSession = await AudioSession.instance;
    await audioSession.configure(const AudioSessionConfiguration.music());
    await _player.openPlayer();
    await _player.startPlayerFromStream(
      codec: Codec.pcm16,
      interleaved: true,
      numChannels: 1,
      sampleRate: acousticSampleRate,
      bufferSize: 8192,
    );
    try {
      while (!_cancelled) {
        final chunk = await session.nextPcmChunk(maxSamples: 8192);
        if (chunk.pcm16Le.isNotEmpty) {
          await _player.feedUint8FromStream(chunk.pcm16Le);
        }
        onProgress(chunk);
        if (chunk.done) break;
      }
      await Future<void>.delayed(const Duration(milliseconds: 400));
    } finally {
      if (_player.isOpen()) {
        await _player.stopPlayer();
        await _player.closePlayer();
      }
      await WakelockPlus.disable();
    }
  }

  Future<void> receive({
    required RxSession session,
    required void Function(List<MobileReceiveEvent> events) onEvents,
    required void Function(double level) onLevel,
  }) async {
    final permission = await Permission.microphone.request();
    if (!permission.isGranted) {
      throw StateError('Нет разрешения на использование микрофона');
    }
    _cancelled = false;
    await WakelockPlus.enable();
    final audioSession = await AudioSession.instance;
    await audioSession.configure(
      const AudioSessionConfiguration(
        avAudioSessionCategory: AVAudioSessionCategory.record,
        avAudioSessionMode: AVAudioSessionMode.measurement,
        androidAudioAttributes: AndroidAudioAttributes(
          contentType: AndroidAudioContentType.music,
          usage: AndroidAudioUsage.media,
        ),
        androidAudioFocusGainType: AndroidAudioFocusGainType.gain,
      ),
    );
    await _recorder.openRecorder();
    _recorderOpen = true;
    _recordController = StreamController<Uint8List>();
    _recordSubscription = _recordController!.stream.listen((pcm) {
      onLevel(_pcmLevel(pcm));
      _decodeQueue = _decodeQueue.then((_) async {
        if (_cancelled) return;
        final events = await session.pushPcm16(pcm16Le: pcm);
        if (events.isNotEmpty) onEvents(events);
      });
    });
    await _recorder.startRecorder(
      toStream: _recordController!.sink,
      codec: Codec.pcm16,
      numChannels: 1,
      sampleRate: acousticSampleRate,
      bufferSize: 8192,
      audioSource: AudioSource.defaultSource,
    );
  }

  Future<void> stop() async {
    _cancelled = true;
    if (_recorder.isRecording) await _recorder.stopRecorder();
    await _recordSubscription?.cancel();
    _recordSubscription = null;
    await _recordController?.close();
    _recordController = null;
    await _decodeQueue;
    if (_recorderOpen) {
      await _recorder.closeRecorder();
      _recorderOpen = false;
    }
    if (_player.isPlaying) await _player.stopPlayer();
    if (_player.isOpen()) await _player.closePlayer();
    await WakelockPlus.disable();
  }

  Future<void> dispose() => stop();

  double _pcmLevel(Uint8List bytes) {
    var peak = 0;
    final data = ByteData.sublistView(bytes);
    for (var offset = 0; offset + 1 < bytes.length; offset += 2) {
      final value = data.getInt16(offset, Endian.little).abs();
      if (value > peak) peak = value;
    }
    return (peak / 32767).clamp(0, 1).toDouble();
  }
}
