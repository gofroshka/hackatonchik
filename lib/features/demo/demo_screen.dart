import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app/providers.dart';
import 'demo_controller.dart';

class DemoScreen extends ConsumerWidget {
  const DemoScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final controller = ref.watch(demoControllerProvider);

    return Scaffold(
      appBar: AppBar(title: const Text('Demo Mode')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text('Сообщение',
                      style: TextStyle(fontWeight: FontWeight.bold)),
                  const SizedBox(height: 4),
                  const SelectableText(
                    DemoController.demoMessage,
                    style: TextStyle(fontSize: 20),
                  ),
                  const Divider(height: 24),
                  _row('Пакетов', '${controller.packetCount}'),
                  _row('Байт на линии', '${controller.byteCount}'),
                  _row('Бит в сигнале (с кодированием)',
                      '${controller.bitCount}'),
                  _row('Длительность аудио',
                      '${controller.durationSeconds.toStringAsFixed(2)} c'),
                ],
              ),
            ),
          ),
          const SizedBox(height: 12),
          Text(
            'Полный конвейер: String → UTF-8 → пакет → CRC16 → '
            'повторение×3 → биты → BFSK PCM',
            style: Theme.of(context).textTheme.bodySmall,
          ),
          const SizedBox(height: 16),
          FilledButton.icon(
            onPressed: controller.busy ? null : controller.play,
            icon: const Icon(Icons.play_arrow),
            label: const Text('Воспроизвести сигнал'),
          ),
          const SizedBox(height: 8),
          OutlinedButton.icon(
            onPressed: controller.busy ? null : controller.decodeGeneratedSignal,
            icon: const Icon(Icons.loop),
            label: const Text('Loopback (без микрофона)'),
          ),
          const SizedBox(height: 8),
          OutlinedButton.icon(
            onPressed: controller.busy ? null : () => controller.saveWav(),
            icon: const Icon(Icons.save),
            label: const Text('Сохранить WAV'),
          ),
          const SizedBox(height: 8),
          OutlinedButton.icon(
            onPressed: controller.busy ? null : controller.importAndDecodeWav,
            icon: const Icon(Icons.file_open),
            label: const Text('Открыть WAV и декодировать'),
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
                      if (controller.busy)
                        const Padding(
                          padding: EdgeInsets.only(right: 8),
                          child: SizedBox(
                            width: 14,
                            height: 14,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                        ),
                      Expanded(child: Text(controller.status)),
                    ],
                  ),
                  if (controller.decodedResult != null) ...[
                    const SizedBox(height: 12),
                    const Text('Результат декодирования:'),
                    SelectableText(
                      controller.decodedResult!,
                      style: const TextStyle(
                        fontSize: 18,
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _row(String label, String value) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 3),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Flexible(child: Text(label)),
            Text(value, style: const TextStyle(fontWeight: FontWeight.bold)),
          ],
        ),
      );
}
