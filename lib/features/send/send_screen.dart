import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app/providers.dart';

class SendScreen extends ConsumerStatefulWidget {
  const SendScreen({super.key});

  @override
  ConsumerState<SendScreen> createState() => _SendScreenState();
}

class _SendScreenState extends ConsumerState<SendScreen> {
  final TextEditingController _text =
      TextEditingController(text: 'HELLO FROM AUDIO');

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = ref.watch(sendControllerProvider);
    final config = ref.watch(modemConfigProvider);
    final bytes = controller.utf8ByteCount(_text.text);

    return Scaffold(
      appBar: AppBar(title: const Text('Передача')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          TextField(
            controller: _text,
            minLines: 3,
            maxLines: 6,
            onChanged: (_) => setState(() {}),
            decoration: const InputDecoration(
              labelText: 'Текст для передачи',
              border: OutlineInputBorder(),
              alignLabelWithHint: true,
            ),
          ),
          const SizedBox(height: 8),
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text('UTF-8 байт: $bytes'),
              Text('Режим: BFSK ${config.freq0}/${config.freq1} Гц'),
            ],
          ),
          const SizedBox(height: 16),
          FilledButton.icon(
            onPressed: controller.isSending || _text.text.isEmpty
                ? null
                : () => controller.send(_text.text),
            icon: const Icon(Icons.graphic_eq),
            label: const Text('Передать'),
          ),
          const SizedBox(height: 8),
          OutlinedButton.icon(
            onPressed: controller.isSending ? controller.stop : null,
            icon: const Icon(Icons.stop),
            label: const Text('Остановить'),
          ),
          const SizedBox(height: 20),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      if (controller.isSending)
                        const Padding(
                          padding: EdgeInsets.only(right: 8),
                          child: SizedBox(
                            width: 16,
                            height: 16,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          ),
                        ),
                      Expanded(child: Text(controller.status)),
                    ],
                  ),
                  const SizedBox(height: 12),
                  LinearProgressIndicator(value: controller.progress),
                  const SizedBox(height: 12),
                  _infoRow('Пакетов', '${controller.packetCount}'),
                  _infoRow(
                    'Оценка времени',
                    '${controller.estimatedSeconds.toStringAsFixed(1)} c',
                  ),
                  _infoRow(
                    'Скорость (payload)',
                    '${config.effectiveBitRate.toStringAsFixed(0)} бит/с',
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _infoRow(String label, String value) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 2),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            Text(label),
            Text(value, style: const TextStyle(fontWeight: FontWeight.bold)),
          ],
        ),
      );
}
