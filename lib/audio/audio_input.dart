import 'dart:async';
import 'dart:typed_data';

import 'package:record/record.dart';

import 'pcm_converter.dart';

/// Errors that can be surfaced to the user in a friendly way.
enum AudioInputError {
  permissionDenied,
  permissionPermanentlyDenied,
  deviceUnavailable,
  unsupportedConfig,
  unknown,
}

class AudioInputException implements Exception {
  const AudioInputException(this.error, this.message);
  final AudioInputError error;
  final String message;
  @override
  String toString() => 'AudioInputException($error): $message';
}

/// Captures raw PCM from the microphone as a stream of normalized samples.
class AudioInput {
  AudioInput() : _recorder = AudioRecorder();

  final AudioRecorder _recorder;
  StreamSubscription<Uint8List>? _subscription;
  bool _recording = false;

  bool get isRecording => _recording;

  /// Whether the app currently has microphone permission (optionally requesting
  /// it).
  Future<bool> hasPermission({bool request = true}) {
    return _recorder.hasPermission(request: request);
  }

  /// Starts streaming microphone audio.
  ///
  /// Returns a broadcast stream of normalized float samples. The [onData]
  /// callback is invoked for each chunk. Throws [AudioInputException] on
  /// failure with a user-friendly reason.
  Future<Stream<Float64List>> start({
    required int sampleRate,
    void Function(int actualSampleRate)? onSampleRateChanged,
  }) async {
    final allowed = await hasPermission();
    if (!allowed) {
      throw const AudioInputException(
        AudioInputError.permissionDenied,
        'Нет разрешения на использование микрофона.',
      );
    }

    final controller = StreamController<Float64List>();

    await _recorder.setOnConfigChanged((config) {
      onSampleRateChanged?.call(config.sampleRate);
    });

    Stream<Uint8List>? raw;
    // Try progressively more permissive audio sources. `unprocessed` gives raw
    // mic samples with NO device-side AGC/echo-cancel/noise-suppression, which
    // is essential so the phone's DSP does not mangle the BFSK tones. If a
    // device does not support it we fall back to `mic`, then the default.
    for (final source in const [
      AndroidAudioSource.unprocessed,
      AndroidAudioSource.mic,
      AndroidAudioSource.defaultSource,
    ]) {
      try {
        raw = await _recorder.startStream(
          RecordConfig(
            encoder: AudioEncoder.pcm16bits,
            sampleRate: sampleRate,
            numChannels: 1,
            autoGain: false,
            echoCancel: false,
            noiseSuppress: false,
            androidConfig: AndroidRecordConfig(
              audioSource: source,
              audioManagerMode: AudioManagerMode.modeNormal,
            ),
          ),
        );
        break;
      } catch (_) {
        // Try the next source.
      }
    }

    if (raw == null) {
      await controller.close();
      throw const AudioInputException(
        AudioInputError.deviceUnavailable,
        'Не удалось запустить запись: микрофон занят или недоступен.',
      );
    }

    _recording = true;
    _subscription = raw.listen(
      (bytes) {
        if (bytes.isEmpty) return;
        controller.add(PcmConverter.bytesToFloat(bytes));
      },
      onError: (Object error) {
        controller.addError(
          AudioInputException(
            AudioInputError.unknown,
            'Ошибка аудиопотока: $error',
          ),
        );
      },
      onDone: () {
        _recording = false;
        if (!controller.isClosed) controller.close();
      },
      cancelOnError: false,
    );

    controller.onCancel = () async {
      await stop();
    };

    return controller.stream;
  }

  /// Stops recording and releases the stream subscription.
  Future<void> stop() async {
    _recording = false;
    await _subscription?.cancel();
    _subscription = null;
    try {
      await _recorder.stop();
    } catch (_) {
      // Ignore stop errors; the recorder may already be stopped.
    }
  }

  /// Fully disposes the recorder.
  Future<void> dispose() async {
    await stop();
    await _recorder.dispose();
  }
}
