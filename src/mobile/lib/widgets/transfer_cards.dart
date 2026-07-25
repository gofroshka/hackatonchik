import 'package:flutter/material.dart';
import 'package:open_filex/open_filex.dart';
import 'package:share_plus/share_plus.dart';
import 'package:sonic_share/controllers/sonic_controller.dart';
import 'package:sonic_share/models/transfer_models.dart';
import 'package:sonic_share/utils/display_formatters.dart';
import 'package:sonic_share/widgets/panel_styles.dart';

class FileIcon extends StatelessWidget {
  const FileIcon({super.key, required this.icon});

  final IconData icon;

  @override
  Widget build(BuildContext context) => Container(
    width: 52,
    height: 52,
    decoration: BoxDecoration(
      color: const Color(0x1843E6D1),
      borderRadius: BorderRadius.circular(16),
    ),
    child: Icon(icon, color: const Color(0xFF43E6D1)),
  );
}

class ProgressCard extends StatelessWidget {
  const ProgressCard({super.key, required this.controller});

  final SonicController controller;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(20),
    decoration: panelCardDecoration(),
    child: Column(
      children: [
        Row(
          children: [
            SizedBox(
              width: 46,
              height: 46,
              child: CircularProgressIndicator(
                value: controller.progress,
                strokeWidth: 5,
              ),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    controller.status,
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                  const SizedBox(height: 4),
                  Text(
                    'Пакет ${controller.packetIndex}/${controller.packetCount}',
                    style: const TextStyle(color: Color(0xFF7E95A7)),
                  ),
                ],
              ),
            ),
            Text(
              '${(controller.progress * 100).round()}%',
              style: const TextStyle(fontWeight: FontWeight.w800, fontSize: 18),
            ),
          ],
        ),
        const SizedBox(height: 18),
        OutlinedButton.icon(
          onPressed: controller.stop,
          icon: const Icon(Icons.stop_rounded),
          label: const Text('Остановить'),
        ),
      ],
    ),
  );
}

class MetricRow extends StatelessWidget {
  const MetricRow({super.key, required this.items});

  final List<(String, String)> items;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      for (final item in items)
        Expanded(
          child: Column(
            children: [
              Text(
                item.$2,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontWeight: FontWeight.w700),
              ),
              const SizedBox(height: 3),
              Text(
                item.$1,
                style: const TextStyle(fontSize: 11, color: Color(0xFF6F8799)),
              ),
            ],
          ),
        ),
    ],
  );
}

class SuccessCard extends StatelessWidget {
  const SuccessCard({super.key, required this.message, required this.onReset});

  final String message;
  final VoidCallback onReset;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(20),
    decoration: panelCardDecoration(color: const Color(0x142FC99B)),
    child: Column(
      children: [
        const Icon(
          Icons.check_circle_rounded,
          color: Color(0xFF43E6D1),
          size: 44,
        ),
        const SizedBox(height: 10),
        Text(message, style: const TextStyle(fontWeight: FontWeight.w700)),
        const SizedBox(height: 12),
        TextButton(onPressed: onReset, child: const Text('Новая передача')),
      ],
    ),
  );
}

class ErrorCard extends StatelessWidget {
  const ErrorCard({super.key, required this.message, required this.onRetry});

  final String message;
  final VoidCallback onRetry;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(20),
    decoration: panelCardDecoration(color: const Color(0x18FF6B7A)),
    child: Column(
      children: [
        const Icon(
          Icons.error_outline_rounded,
          color: Color(0xFFFF6B7A),
          size: 40,
        ),
        const SizedBox(height: 10),
        Text(message, textAlign: TextAlign.center),
        const SizedBox(height: 12),
        TextButton(onPressed: onRetry, child: const Text('Попробовать снова')),
      ],
    ),
  );
}

class ReceivedCard extends StatelessWidget {
  const ReceivedCard({
    super.key,
    required this.received,
    required this.onReset,
  });

  final ReceivedTransfer received;
  final VoidCallback onReset;

  @override
  Widget build(BuildContext context) => Container(
    padding: const EdgeInsets.all(20),
    decoration: panelCardDecoration(color: const Color(0x142FC99B)),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Row(
          children: [
            Icon(Icons.verified_rounded, color: Color(0xFF43E6D1)),
            SizedBox(width: 9),
            Text(
              'SHA-256 VERIFIED',
              style: TextStyle(
                color: Color(0xFF43E6D1),
                fontWeight: FontWeight.w800,
                fontSize: 12,
                letterSpacing: 1.1,
              ),
            ),
          ],
        ),
        const SizedBox(height: 16),
        Text(
          received.name,
          style: const TextStyle(fontWeight: FontWeight.w800, fontSize: 18),
        ),
        const SizedBox(height: 5),
        Text(
          '${received.contentType} · ${formatBytes(received.size)}',
          style: const TextStyle(color: Color(0xFF7E95A7)),
        ),
        if (received.text != null) ...[
          const SizedBox(height: 14),
          Container(
            width: double.infinity,
            constraints: const BoxConstraints(maxHeight: 130),
            padding: const EdgeInsets.all(13),
            decoration: BoxDecoration(
              color: const Color(0xFF091623),
              borderRadius: BorderRadius.circular(13),
            ),
            child: SingleChildScrollView(
              child: Text(
                received.text!,
                style: const TextStyle(fontFamily: 'monospace'),
              ),
            ),
          ),
        ],
        const SizedBox(height: 18),
        Row(
          children: [
            Expanded(
              child: OutlinedButton.icon(
                onPressed: () => OpenFilex.open(received.path),
                icon: const Icon(Icons.open_in_new_rounded),
                label: const Text('Открыть'),
              ),
            ),
            const SizedBox(width: 10),
            Expanded(
              child: FilledButton.icon(
                onPressed: () => SharePlus.instance.share(
                  ShareParams(files: [XFile(received.path)]),
                ),
                icon: const Icon(Icons.ios_share_rounded),
                label: const Text('Поделиться'),
              ),
            ),
          ],
        ),
        Center(
          child: TextButton(
            onPressed: onReset,
            child: const Text('Принять ещё файл'),
          ),
        ),
      ],
    ),
  );
}
