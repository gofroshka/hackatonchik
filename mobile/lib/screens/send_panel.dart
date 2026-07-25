import 'package:flutter/material.dart';
import 'package:sonic_share/controllers/sonic_controller.dart';
import 'package:sonic_share/models/transfer_models.dart';
import 'package:sonic_share/utils/display_formatters.dart';
import 'package:sonic_share/widgets/panel_styles.dart';
import 'package:sonic_share/widgets/transfer_cards.dart';

class SendPanel extends StatelessWidget {
  const SendPanel({super.key, required this.controller});

  final SonicController controller;

  @override
  Widget build(BuildContext context) {
    final file = controller.selectedFile;
    final info = controller.txInfo;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Передача', style: Theme.of(context).textTheme.displaySmall),
        const SizedBox(height: 8),
        const Text('Положите телефоны рядом и не закрывайте динамик.'),
        const SizedBox(height: 24),
        InkWell(
          borderRadius: BorderRadius.circular(24),
          onTap: controller.isBusy ? null : controller.pickFile,
          child: Container(
            padding: const EdgeInsets.all(20),
            decoration: panelCardDecoration(),
            child: file == null
                ? const Row(
                    children: [
                      FileIcon(icon: Icons.add_rounded),
                      SizedBox(width: 16),
                      Expanded(
                        child: Text(
                          'Выбрать файл',
                          style: TextStyle(
                            fontSize: 17,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ),
                      Icon(
                        Icons.chevron_right_rounded,
                        color: Color(0xFF71899C),
                      ),
                    ],
                  )
                : Row(
                    children: [
                      const FileIcon(icon: Icons.insert_drive_file_rounded),
                      const SizedBox(width: 16),
                      Expanded(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              file.name,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(
                                fontSize: 16,
                                fontWeight: FontWeight.w700,
                              ),
                            ),
                            const SizedBox(height: 5),
                            Text(
                              formatBytes(file.size),
                              style: const TextStyle(color: Color(0xFF7E95A7)),
                            ),
                          ],
                        ),
                      ),
                      if (!controller.isBusy)
                        const Icon(
                          Icons.swap_horiz_rounded,
                          color: Color(0xFF43E6D1),
                        ),
                    ],
                  ),
          ),
        ),
        if (info != null) ...[
          const SizedBox(height: 14),
          MetricRow(
            items: [
              ('MIME', info.contentType),
              ('Пакеты', '${info.packetCount}'),
              ('Время', formatDuration(info.estimatedSeconds)),
            ],
          ),
        ],
        const SizedBox(height: 20),
        if (controller.phase == TransferPhase.handshaking ||
            controller.phase == TransferPhase.handshakeWaitAck ||
            controller.phase == TransferPhase.endWaitAck)
          _HandshakeCard(controller: controller)
        else if (controller.phase == TransferPhase.sending ||
            controller.phase == TransferPhase.preparing)
          ProgressCard(controller: controller)
        else if (controller.phase == TransferPhase.completed)
          SuccessCard(message: controller.status, onReset: controller.reset)
        else if (controller.phase == TransferPhase.error)
          ErrorCard(
            message: controller.error ?? 'Неизвестная ошибка',
            onRetry: controller.reset,
          )
        else ...[
          FilledButton.icon(
            onPressed: file == null ? null : controller.startSending,
            icon: const Icon(Icons.volume_up_rounded),
            label: const Text('Начать передачу'),
            style: primaryPanelButtonStyle(context),
          ),
          const SizedBox(height: 12),
          const Text(
            'Большие файлы могут передаваться очень долго. Приложение покажет точную оценку до старта.',
            textAlign: TextAlign.center,
            style: TextStyle(fontSize: 12, color: Color(0xFF657E91)),
          ),
        ],
      ],
    );
  }
}

class _HandshakeCard extends StatelessWidget {
  const _HandshakeCard({required this.controller});

  final SonicController controller;

  @override
  Widget build(BuildContext context) {
    final phase = controller.phase;
    final isHandshaking = phase == TransferPhase.handshaking;
    final isWaiting = phase == TransferPhase.handshakeWaitAck;
    final isEnd = phase == TransferPhase.endWaitAck;
    final icon = isEnd
        ? Icons.check_circle_outline_rounded
        : isWaiting
        ? Icons.hearing_rounded
        : Icons.wifi_find_rounded;
    final color = isEnd
        ? const Color(0xFFFF8B6A)
        : isWaiting
        ? const Color(0xFF43E6D1)
        : const Color(0xFFFFD700);
    return Container(
      padding: const EdgeInsets.all(20),
      decoration: panelCardDecoration(color: color.withValues(alpha: 0.08)),
      child: Column(
        children: [
          SizedBox(
            width: 56,
            height: 56,
            child: CircularProgressIndicator(
              value: isWaiting ? null : (isEnd ? 1.0 : null),
              strokeWidth: 4,
              color: color,
              backgroundColor: color.withValues(alpha: 0.15),
            ),
          ),
          const SizedBox(height: 16),
          Icon(icon, color: color, size: 32),
          const SizedBox(height: 12),
          Text(
            controller.status,
            textAlign: TextAlign.center,
            style: const TextStyle(fontWeight: FontWeight.w700, fontSize: 15),
          ),
          if (isHandshaking || isEnd)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                'Убедитесь, что устройство-приёмник включено и находится рядом',
                textAlign: TextAlign.center,
                style: const TextStyle(color: Color(0xFF7E95A7), fontSize: 12),
              ),
            ),
          const SizedBox(height: 18),
          OutlinedButton.icon(
            onPressed: controller.stop,
            icon: const Icon(Icons.stop_rounded),
            label: const Text('Отмена'),
          ),
        ],
      ),
    );
  }
}
