import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app/providers.dart';
import '../../modem/streaming_decoder.dart';

class ReceiveScreen extends ConsumerWidget {
  const ReceiveScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final controller = ref.watch(receiveControllerProvider);
    final scheme = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(title: const Text('Приём')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SizedBox(
            height: 64,
            child: FilledButton.icon(
              onPressed: controller.isReceiving
                  ? null
                  : () => controller.start(),
              icon: const Icon(Icons.mic),
              label: const Text('Начать приём'),
            ),
          ),
          const SizedBox(height: 8),
          OutlinedButton.icon(
            onPressed: controller.isReceiving ? controller.stop : null,
            icon: const Icon(Icons.stop),
            label: const Text('Остановить'),
          ),
          const SizedBox(height: 16),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Icon(
                        controller.isReceiving ? Icons.mic : Icons.mic_off,
                        color: controller.isReceiving
                            ? scheme.primary
                            : scheme.outline,
                      ),
                      const SizedBox(width: 8),
                      Expanded(child: Text(controller.micStatus)),
                    ],
                  ),
                  const SizedBox(height: 12),
                  const Text('Уровень входного сигнала'),
                  const SizedBox(height: 4),
                  LinearProgressIndicator(
                    value: (controller.inputLevel * 4).clamp(0.0, 1.0),
                    minHeight: 10,
                  ),
                  const SizedBox(height: 12),
                  _StateChip(state: controller.decoderState),
                ],
              ),
            ),
          ),
          if (controller.errorText != null) ...[
            const SizedBox(height: 12),
            Card(
              color: scheme.errorContainer,
              child: Padding(
                padding: const EdgeInsets.all(16),
                child: Row(
                  children: [
                    Icon(Icons.warning_amber, color: scheme.onErrorContainer),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        controller.errorText!,
                        style: TextStyle(color: scheme.onErrorContainer),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
          const SizedBox(height: 16),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: [
                      const Text('Принятый текст',
                          style: TextStyle(fontWeight: FontWeight.bold)),
                      IconButton(
                        onPressed: controller.receivedText.isEmpty
                            ? null
                            : () {
                                Clipboard.setData(
                                  ClipboardData(text: controller.receivedText),
                                );
                                ScaffoldMessenger.of(context).showSnackBar(
                                  const SnackBar(
                                    content: Text('Скопировано'),
                                    duration: Duration(seconds: 1),
                                  ),
                                );
                              },
                        icon: const Icon(Icons.copy),
                      ),
                    ],
                  ),
                  SelectableText(
                    controller.receivedText.isEmpty
                        ? '—'
                        : controller.receivedText,
                    style: const TextStyle(fontSize: 18),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text('История сообщений',
                  style: TextStyle(fontWeight: FontWeight.bold)),
              TextButton(
                onPressed: controller.history.isEmpty
                    ? null
                    : controller.clearHistory,
                child: const Text('Очистить'),
              ),
            ],
          ),
          ...controller.history.map(
            (m) => ListTile(
              dense: true,
              leading: const Icon(Icons.message),
              title: Text(m.text),
              subtitle: Text(
                '${m.timestamp.hour.toString().padLeft(2, '0')}:'
                '${m.timestamp.minute.toString().padLeft(2, '0')}:'
                '${m.timestamp.second.toString().padLeft(2, '0')}',
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _StateChip extends StatelessWidget {
  const _StateChip({required this.state});
  final DecoderState state;

  @override
  Widget build(BuildContext context) {
    final (label, color, icon) = switch (state) {
      DecoderState.idle => ('Ожидание', Colors.grey, Icons.hourglass_empty),
      DecoderState.signalDetected =>
        ('Обнаружен сигнал', Colors.blue, Icons.sensors),
      DecoderState.preambleFound =>
        ('Найдена преамбула', Colors.indigo, Icons.flag),
      DecoderState.receivingPacket =>
        ('Принимается пакет', Colors.orange, Icons.download),
      DecoderState.checkingCrc =>
        ('Проверка CRC', Colors.amber, Icons.verified),
      DecoderState.messageReceived =>
        ('Сообщение принято', Colors.green, Icons.check_circle),
      DecoderState.error => ('Ошибка', Colors.red, Icons.error),
    };
    return Chip(
      avatar: Icon(icon, color: color, size: 18),
      label: Text(label),
    );
  }
}
