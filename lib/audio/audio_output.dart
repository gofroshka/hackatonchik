import 'dart:async';
import 'dart:typed_data';

import 'package:flutter_pcm_sound/flutter_pcm_sound.dart';

/// Plays 16-bit signed PCM through the device speaker using a feed-callback
/// driven queue so large buffers stream smoothly without gaps.
class AudioOutput {
  AudioOutput({this.feedThreshold = 8000});

  /// When the plugin's internal queue falls below this many frames the feed
  /// callback fires so we can push more data.
  final int feedThreshold;

  bool _initialized = false;
  Int16List _pending = Int16List(0);
  int _position = 0;
  Completer<void>? _completer;
  bool _stopped = false;
  int _configuredSampleRate = 0;

  bool get isPlaying => _completer != null && !_completer!.isCompleted;

  Future<void> _ensureSetup(int sampleRate) async {
    if (_initialized && _configuredSampleRate == sampleRate) return;
    FlutterPcmSound.setLogLevel(LogLevel.none);
    await FlutterPcmSound.setup(sampleRate: sampleRate, channelCount: 1);
    FlutterPcmSound.setFeedThreshold(feedThreshold);
    FlutterPcmSound.setFeedCallback(_onFeed);
    _initialized = true;
    _configuredSampleRate = sampleRate;
  }

  /// Plays [pcm]. The returned future completes when playback finishes or is
  /// stopped.
  Future<void> play(Int16List pcm, {required int sampleRate}) async {
    await stop();
    await _ensureSetup(sampleRate);

    _pending = pcm;
    _position = 0;
    _stopped = false;
    _completer = Completer<void>();

    // Kick off playback by feeding the first chunk.
    _feedNext(0);
    FlutterPcmSound.start();

    return _completer!.future;
  }

  void _onFeed(int remainingFrames) {
    if (_stopped) return;
    _feedNext(remainingFrames);
  }

  void _feedNext(int remainingFrames) {
    if (_stopped) return;
    if (_position >= _pending.length) {
      // Nothing left to feed; when the queue drains, playback is done.
      if (remainingFrames == 0) {
        _complete();
      }
      return;
    }
    const chunkFrames = 8000;
    final end = (_position + chunkFrames).clamp(0, _pending.length);
    final slice = _pending.sublist(_position, end);
    _position = end;
    FlutterPcmSound.feed(PcmArrayInt16(bytes: slice.buffer.asByteData(
      slice.offsetInBytes,
      slice.lengthInBytes,
    )));
  }

  void _complete() {
    if (_completer != null && !_completer!.isCompleted) {
      _completer!.complete();
    }
  }

  /// Stops any current playback.
  Future<void> stop() async {
    _stopped = true;
    _position = _pending.length;
    _complete();
    _completer = null;
  }

  /// Releases native audio resources.
  Future<void> dispose() async {
    await stop();
    if (_initialized) {
      FlutterPcmSound.setFeedCallback(null);
      await FlutterPcmSound.release();
      _initialized = false;
    }
  }
}
