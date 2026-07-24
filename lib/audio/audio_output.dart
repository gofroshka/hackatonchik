import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:audioplayers/audioplayers.dart';
import 'package:path_provider/path_provider.dart';

import '../shared/utils/app_logger.dart';
import '../shared/utils/wav_utils.dart';

class AudioOutput {
  final AudioPlayer _player = AudioPlayer();
  File? _tempFile;
  StreamSubscription? _stateSub;
  bool _playing = false;
  bool _contextSet = false;

  bool get isPlaying => _playing;

  /// Configures the audio session for plain playback. Without an explicit
  /// context iOS can keep the session in a record-oriented category (set by the
  /// `record` plugin on the receive screen), which conflicts with playback.
  Future<void> _ensureContext() async {
    if (_contextSet) return;
    await AudioPlayer.global.setAudioContext(
      AudioContext(
        // playAndRecord (not playback) is compatible with the `record` plugin's
        // active capture session on the receive screen. Switching to a
        // playback-only category while the mic session is active crashes the
        // native audio unit on iOS. defaultToSpeaker routes to the loud speaker
        // instead of the earpiece.
        iOS: AudioContextIOS(
          category: AVAudioSessionCategory.playAndRecord,
          options: const {
            AVAudioSessionOptions.defaultToSpeaker,
            AVAudioSessionOptions.mixWithOthers,
            AVAudioSessionOptions.allowBluetooth,
          },
        ),
        android: AudioContextAndroid(
          isSpeakerphoneOn: true,
          contentType: AndroidContentType.music,
          usageType: AndroidUsageType.media,
          audioFocus: AndroidAudioFocus.gain,
        ),
      ),
    );
    _contextSet = true;
  }

  Future<void> play(Int16List pcm, {required int sampleRate}) async {
    await stop();

    try {
      AppLogger.info('play: encoding WAV (${pcm.length} samples @ $sampleRate)');
      final wav = WavUtils.encode(pcm, sampleRate: sampleRate);

      AppLogger.info('play: setting audio context');
      await _ensureContext();

      final dir = await getTemporaryDirectory();
      await dir.create(recursive: true);
      final file = File(
          '${dir.path}/audio_${DateTime.now().microsecondsSinceEpoch}.wav');
      await file.writeAsBytes(wav, flush: true);
      _tempFile = file;
      AppLogger.info('play: wrote ${wav.length} bytes to ${file.path}');

      final completer = Completer<void>();
      _stateSub = _player.onPlayerStateChanged.listen((state) {
        if (state == PlayerState.completed && !completer.isCompleted) {
          completer.complete();
        }
      });

      AppLogger.info('play: starting playback');
      await _player.play(DeviceFileSource(file.path));
      _playing = true;
      AppLogger.info('play: playback started');
      return completer.future;
    } catch (e, st) {
      AppLogger.error('play: failed', e, st);
      rethrow;
    }
  }

  Future<void> stop() async {
    _playing = false;
    await _stateSub?.cancel();
    _stateSub = null;
    try {
      await _player.stop();
    } catch (e, st) {
      AppLogger.error('stop: failed', e, st);
    }
    await _deleteTempFile();
  }

  Future<void> _deleteTempFile() async {
    final file = _tempFile;
    if (file != null && file.existsSync()) {
      try {
        await file.delete();
      } catch (_) {}
    }
    _tempFile = null;
  }

  Future<void> dispose() async {
    await stop();
    _player.dispose();
  }
}
