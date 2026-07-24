import 'dart:typed_data';

import '../modem/modem_config.dart';

/// Messages sent to the decoder isolate.
sealed class DecoderCommand {
  const DecoderCommand();
}

/// Initial handshake carrying the config (the reply port is sent separately).
class InitCommand extends DecoderCommand {
  const InitCommand(this.config);
  final ModemConfig config;
}

/// A chunk of normalized PCM samples to decode.
class SamplesCommand extends DecoderCommand {
  const SamplesCommand(this.samples);
  final Float64List samples;
}

/// Reset the decoder state.
class ResetCommand extends DecoderCommand {
  const ResetCommand();
}

/// Shut the isolate down.
class ShutdownCommand extends DecoderCommand {
  const ShutdownCommand();
}
