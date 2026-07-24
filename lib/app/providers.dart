import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_riverpod/legacy.dart';

import '../features/demo/demo_controller.dart';
import '../features/receive/receive_controller.dart';
import '../features/send/send_controller.dart';
import '../modem/modem_config.dart';

/// The active modem configuration shared across the app.
final modemConfigProvider = Provider<ModemConfig>((ref) {
  return const ModemConfig();
});

/// Controller for the Send screen.
final sendControllerProvider =
    ChangeNotifierProvider<SendController>((ref) {
  final config = ref.watch(modemConfigProvider);
  final controller = SendController(config: config);
  ref.onDispose(controller.dispose);
  return controller;
});

/// Controller for the Receive + Diagnostics screens.
final receiveControllerProvider =
    ChangeNotifierProvider<ReceiveController>((ref) {
  final config = ref.watch(modemConfigProvider);
  final controller = ReceiveController(config: config);
  ref.onDispose(controller.dispose);
  return controller;
});

/// Controller for the Demo screen.
final demoControllerProvider =
    ChangeNotifierProvider<DemoController>((ref) {
  final config = ref.watch(modemConfigProvider);
  final controller = DemoController(config: config);
  ref.onDispose(controller.dispose);
  return controller;
});
