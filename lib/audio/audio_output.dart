import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import 'package:audioplayers/audioplayers.dart';
import 'package:path_provider/path_provider.dart';

import '../shared/utils/wav_utils.dart';

class AudioOutput {
  final AudioPlayer _player = AudioPlayer();
  File? _tempFile;
  StreamSubscription? _stateSub;
  bool _playing = false;

  bool get isPlaying => _playing;

  Future<void> play(Int16List pcm, {required int sampleRate}) async {
    await stop();

    final wav = WavUtils.encode(pcm, sampleRate: sampleRate);
    final dir = await getTemporaryDirectory();
    // The temporary directory (e.g. the sandboxed Caches subfolder on macOS)
    // is not guaranteed to exist yet — create it before writing.
    await dir.create(recursive: true);
    final file = File('${dir.path}/audio_${DateTime.now().microsecondsSinceEpoch}.wav');
    await file.writeAsBytes(wav, flush: true);
    _tempFile = file;

    final completer = Completer<void>();
    _stateSub = _player.onPlayerStateChanged.listen((state) {
      if (state == PlayerState.completed) {
        if (!completer.isCompleted) completer.complete();
      }
    });

    await _player.play(DeviceFileSource(file.path));
    _playing = true;
    return completer.future;
  }

  Future<void> stop() async {
    _playing = false;
    await _stateSub?.cancel();
    _stateSub = null;
    await _player.stop();
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
