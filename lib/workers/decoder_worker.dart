import 'dart:async';
import 'dart:isolate';
import 'dart:typed_data';

import '../modem/modem_config.dart';
import '../modem/streaming_decoder.dart';
import 'decoder_messages.dart';

/// Runs a [StreamingDecoder] in a background isolate so the heavy Goertzel /
/// synchronization DSP never blocks the Flutter UI thread.
class DecoderWorker {
  Isolate? _isolate;
  SendPort? _toIsolate;
  ReceivePort? _fromIsolate;
  final StreamController<DecoderEvent> _events =
      StreamController<DecoderEvent>.broadcast();
  final Completer<void> _ready = Completer<void>();

  /// Stream of decoder events produced by the isolate.
  Stream<DecoderEvent> get events => _events.stream;

  /// Spawns the isolate and initializes it with [config].
  Future<void> start(ModemConfig config) async {
    _fromIsolate = ReceivePort();
    _isolate = await Isolate.spawn(
      _entryPoint,
      _fromIsolate!.sendPort,
      debugName: 'decoder-worker',
    );

    _fromIsolate!.listen((Object? message) {
      if (message is SendPort) {
        _toIsolate = message;
        _toIsolate!.send(InitCommand(config));
        if (!_ready.isCompleted) _ready.complete();
      } else if (message is DecoderEvent) {
        if (!_events.isClosed) _events.add(message);
      }
    });

    await _ready.future;
  }

  /// Sends a chunk of normalized samples to the decoder.
  void addSamples(Float64List samples) {
    _toIsolate?.send(SamplesCommand(samples));
  }

  /// Resets the decoder state.
  void reset() {
    _toIsolate?.send(const ResetCommand());
  }

  /// Shuts down the isolate and releases resources.
  Future<void> dispose() async {
    _toIsolate?.send(const ShutdownCommand());
    await Future<void>.delayed(const Duration(milliseconds: 50));
    _fromIsolate?.close();
    _isolate?.kill(priority: Isolate.immediate);
    _isolate = null;
    if (!_events.isClosed) await _events.close();
  }

  /// Isolate entry point.
  static void _entryPoint(SendPort mainSendPort) {
    final port = ReceivePort();
    mainSendPort.send(port.sendPort);

    StreamingDecoder? decoder;

    port.listen((Object? message) {
      if (message is InitCommand) {
        decoder = StreamingDecoder(config: message.config);
      } else if (message is SamplesCommand) {
        final d = decoder;
        if (d == null) return;
        final events = d.addSamples(message.samples);
        for (final event in events) {
          mainSendPort.send(event);
        }
      } else if (message is ResetCommand) {
        decoder?.reset();
      } else if (message is ShutdownCommand) {
        port.close();
      }
    });
  }
}
