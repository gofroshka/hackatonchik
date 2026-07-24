import 'dart:developer' as developer;

import 'package:flutter/foundation.dart';

/// Lightweight logging wrapper.
///
/// Logs are emitted both via `debugPrint` (so they appear directly in the
/// `flutter run` terminal) and `dart:developer.log` (so they appear in the
/// DevTools Logging view, filterable by the `hackatonchik` name).
class AppLogger {
  const AppLogger._();

  static void info(String message) => _log('INFO', message, level: 800);

  static void warning(String message) => _log('WARN', message, level: 900);

  static void error(String message, [Object? error, StackTrace? stackTrace]) =>
      _log('ERROR', message, level: 1000, error: error, stackTrace: stackTrace);

  static void _log(
    String tag,
    String message, {
    required int level,
    Object? error,
    StackTrace? stackTrace,
  }) {
    final buffer = StringBuffer('[hackatonchik][$tag] $message');
    if (error != null) buffer.write('\n  error: $error');
    if (stackTrace != null) buffer.write('\n  stack: $stackTrace');
    debugPrint(buffer.toString());

    developer.log(
      message,
      name: 'hackatonchik',
      level: level,
      error: error,
      stackTrace: stackTrace,
    );
  }
}
