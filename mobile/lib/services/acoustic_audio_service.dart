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
  DateTime _lastHandshakeCheck = DateTime(2000);

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
    bool checkHandshake = false,
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
    _lastHandshakeCheck = DateTime(2000);
    await _recorder.openRecorder();
    _recorderOpen = true;
    _recordController = StreamController<Uint8List>();
    _recordSubscription = _recordController!.stream.listen((pcm) {
      onLevel(_pcmLevel(pcm));
      if (checkHandshake) {
        _decodeQueue = _decodeQueue.then((_) async {
          if (_cancelled) return;
          if (DateTime.now().difference(_lastHandshakeCheck) >
              const Duration(seconds: 2)) {
            final hsEvents = await checkPcmForHandshake(
              pcm16Le: pcm,
              sampleRate: acousticSampleRate,
            );
            for (final event in hsEvents) {
              if (event.kind == 'handshake_request') {
                _lastHandshakeCheck = DateTime.now();
                onEvents(hsEvents);
              }
            }
          }
        });
      }
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

  Future<void> playPcmTight(Uint8List pcm, {int sampleRate = 48000}) async {
    if (_player.isPlaying) await _player.stopPlayer();
    if (!_player.isOpen()) await _player.openPlayer();
    await _player.startPlayerFromStream(
      codec: Codec.pcm16,
      interleaved: true,
      numChannels: 1,
      sampleRate: sampleRate,
      bufferSize: 8192,
    );
    await _player.feedUint8FromStream(pcm);
    final duration = Duration(
      milliseconds: (pcm.length / 2 / sampleRate * 1000).ceil(),
    );
    await Future.delayed(duration);
    if (!_cancelled) {
      await _player.stopPlayer();
      await _player.closePlayer();
    }
  }

  Future<void> playShortPcm(Uint8List pcm, {int sampleRate = 48000}) async {
    _cancelled = false;
    if (_player.isPlaying) await _player.stopPlayer();
    if (!_player.isOpen()) await _player.openPlayer();
    await _player.startPlayerFromStream(
      codec: Codec.pcm16,
      interleaved: true,
      numChannels: 1,
      sampleRate: sampleRate,
      bufferSize: 8192,
    );
    await _player.feedUint8FromStream(pcm);
    final duration = Duration(
      milliseconds: (pcm.length / 2 / sampleRate * 1000).ceil() + 200,
    );
    await Future.delayed(duration);
    if (!_cancelled) {
      await _player.stopPlayer();
      await _player.closePlayer();
    }
  }

  Future<Uint8List?> recordShort(Duration timeout, {int sampleRate = 48000}) async {
    final permission = await Permission.microphone.request();
    if (!permission.isGranted) return null;
    _cancelled = false;
    if (_recorder.isRecording) await _recorder.stopRecorder();
    if (!_recorderOpen) {
      await _recorder.openRecorder();
      _recorderOpen = true;
    }
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
    final buffer = BytesBuilder();
    final controller = StreamController<Uint8List>();
    final sub = controller.stream.listen((data) {
      if (!_cancelled) buffer.add(data);
    });
    await _recorder.startRecorder(
      toStream: controller.sink,
      codec: Codec.pcm16,
      numChannels: 1,
      sampleRate: sampleRate,
      bufferSize: 4096,
    );
    await Future.delayed(timeout);
    if (!_cancelled) {
      await _recorder.stopRecorder();
    }
    await sub.cancel();
    await controller.close();
    return buffer.takeBytes();
  }

  Future<void> prepareForPlayback() async {
    if (_recorder.isRecording) await _recorder.stopRecorder();
    final audioSession = await AudioSession.instance;
    await audioSession.configure(const AudioSessionConfiguration.music());
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
