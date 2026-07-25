String formatBytes(int bytes) {
  if (bytes < 1024) return '$bytes Б';
  if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} КБ';
  return '${(bytes / 1024 / 1024).toStringAsFixed(1)} МБ';
}

String formatDuration(double seconds) {
  final duration = Duration(seconds: seconds.ceil());
  if (duration.inHours > 0) {
    return '${duration.inHours}ч ${duration.inMinutes.remainder(60)}м';
  }
  if (duration.inMinutes > 0) {
    return '${duration.inMinutes}м ${duration.inSeconds.remainder(60)}с';
  }
  return '${duration.inSeconds}с';
}

String formatTime(DateTime value) =>
    '${value.hour.toString().padLeft(2, '0')}:'
    '${value.minute.toString().padLeft(2, '0')}';
