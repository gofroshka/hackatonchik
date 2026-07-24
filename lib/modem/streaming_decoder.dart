import 'dart:typed_data';

import '../protocol/message_assembler.dart';
import '../protocol/packet_decoder.dart';
import 'bfsk_demodulator.dart';
import 'modem_config.dart';
import 'ring_buffer.dart';
import 'signal_detector.dart';

/// High level decoder state machine surfaced to the UI.
enum DecoderState {
  idle,
  signalDetected,
  preambleFound,
  receivingPacket,
  checkingCrc,
  messageReceived,
  error,
}

/// An event emitted by the [StreamingDecoder] as audio is processed.
class DecoderEvent {
  const DecoderEvent({
    required this.state,
    required this.diagnostics,
    this.message,
    this.inputLevel = 0,
    this.errorText,
  });

  final DecoderState state;
  final DemodDiagnostics diagnostics;

  /// A fully assembled message when [state] is [DecoderState.messageReceived].
  final AssembledMessage? message;

  /// Normalized input level (0..1) for the VU meter.
  final double inputLevel;

  final String? errorText;
}

/// Consumes a live stream of PCM samples and emits [DecoderEvent]s.
///
/// The decoder keeps recent samples in a [RingBuffer]. When the signal
/// detector reports energy, it accumulates until a trailing silence is seen and
/// then runs the offline frame decoder over the captured region. This keeps the
/// expensive DSP off the boundaries of the live callback while remaining
/// responsive.
class StreamingDecoder {
  StreamingDecoder({ModemConfig? config})
      : config = config ?? const ModemConfig() {
    _demodulator = BfskDemodulator(this.config);
    _detector = SignalDetector(snrThreshold: this.config.snrThreshold);
    // Hold up to ~8 seconds of audio.
    _buffer = RingBuffer(this.config.sampleRate * 8);
    _assembler = MessageAssembler();
  }

  final ModemConfig config;
  late final BfskDemodulator _demodulator;
  late final SignalDetector _detector;
  late final RingBuffer _buffer;
  late final MessageAssembler _assembler;

  bool _capturing = false;
  int _silenceCounter = 0;
  DecoderState _state = DecoderState.idle;

  DecoderState get state => _state;

  /// Feeds a chunk of normalized samples and returns any events produced.
  List<DecoderEvent> addSamples(Float64List chunk) {
    final events = <DecoderEvent>[];
    if (chunk.isEmpty) return events;

    final present = _detector.update(chunk);
    final level = _detector.lastRms.clamp(0.0, 1.0);

    _buffer.addAll(chunk);

    if (present) {
      _silenceCounter = 0;
      if (!_capturing) {
        _capturing = true;
        _state = DecoderState.signalDetected;
        events.add(_event(level: level));
      }
    } else if (_capturing) {
      _silenceCounter += chunk.length;
      // After ~250 ms of silence, assume the burst is complete and decode.
      if (_silenceCounter > config.sampleRate ~/ 4) {
        events.addAll(_decodeCaptured(level));
        _capturing = false;
        _silenceCounter = 0;
      }
    } else {
      // Keep the buffer from growing without bound while idle.
      final maxIdle = config.sampleRate; // 1 s
      if (_buffer.length > maxIdle) {
        _buffer.drop(_buffer.length - maxIdle);
      }
    }

    // Always emit a lightweight level tick so the UI meter and live energy
    // graph stay responsive even when no state transition happened.
    if (events.isEmpty) {
      events.add(
        DecoderEvent(
          state: _state,
          diagnostics: DemodDiagnostics(
            noiseFloor: _detector.noiseFloor,
            snr: _detector.lastSnr,
          ),
          inputLevel: level,
        ),
      );
    }

    return events;
  }

  List<DecoderEvent> _decodeCaptured(double level) {
    final events = <DecoderEvent>[];
    final samples = _buffer.toList();

    _state = DecoderState.receivingPacket;
    events.add(_event(level: level));

    int searchStart = 0;
    final s = config.samplesPerSymbol;
    bool anyMessage = false;
    DemodDiagnostics lastDiag = const DemodDiagnostics();
    int guard = 0;

    while (searchStart + config.preambleBits * s < samples.length) {
      final frame =
          _demodulator.decodeFrame(samples, searchStart: searchStart);
      lastDiag = frame.diagnostics;

      if (frame.status == PacketDecodeStatus.noSync) break;

      _state = DecoderState.checkingCrc;

      if (frame.status == PacketDecodeStatus.ok) {
        final packet = frame.packetResult!.packet!;
        final assembled = _assembler.addPacket(packet);
        if (assembled != null) {
          anyMessage = true;
          _state = DecoderState.messageReceived;
          events.add(
            DecoderEvent(
              state: DecoderState.messageReceived,
              diagnostics: frame.diagnostics,
              message: assembled,
              inputLevel: level,
            ),
          );
        }
      } else if (frame.status == PacketDecodeStatus.crcError) {
        _state = DecoderState.error;
        events.add(
          DecoderEvent(
            state: DecoderState.error,
            diagnostics: frame.diagnostics,
            inputLevel: level,
            errorText: 'Ошибка CRC — пакет отклонён',
          ),
        );
      }

      final next = frame.consumedUntil;
      searchStart = next <= searchStart ? searchStart + s : next;
      if (++guard > 5000) break;
    }

    // Clear consumed audio and go back to idle.
    _buffer.clear();
    if (!anyMessage && _state != DecoderState.error) {
      _state = DecoderState.idle;
      events.add(
        DecoderEvent(
          state: DecoderState.idle,
          diagnostics: lastDiag,
          inputLevel: level,
        ),
      );
    }
    return events;
  }

  DecoderEvent _event({double level = 0}) => DecoderEvent(
        state: _state,
        diagnostics: const DemodDiagnostics(),
        inputLevel: level,
      );

  /// Resets all decoder state (e.g. after a timeout or stop).
  void reset() {
    _buffer.clear();
    _assembler.reset();
    _detector.reset();
    _capturing = false;
    _silenceCounter = 0;
    _state = DecoderState.idle;
  }
}
