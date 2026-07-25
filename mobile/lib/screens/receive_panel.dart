import 'package:flutter/material.dart';
import 'package:sonic_share/controllers/sonic_controller.dart';
import 'package:sonic_share/models/transfer_models.dart';
import 'package:sonic_share/widgets/panel_styles.dart';
import 'package:sonic_share/widgets/radar.dart';
import 'package:sonic_share/widgets/transfer_cards.dart';

class ReceivePanel extends StatelessWidget {
  const ReceivePanel({super.key, required this.controller});

  final SonicController controller;

  @override
  Widget build(BuildContext context) {
    final listening = controller.phase == TransferPhase.listening;
    final received = controller.received;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Text('Приём', style: Theme.of(context).textTheme.displaySmall),
        const SizedBox(height: 8),
        const Text('Направьте микрофон в сторону телефона-отправителя.'),
        const SizedBox(height: 22),
        Radar(level: controller.microphoneLevel, active: listening),
        const SizedBox(height: 20),
        Text(
          controller.status,
          textAlign: TextAlign.center,
          style: TextStyle(
            fontWeight: FontWeight.w700,
            fontSize: 16,
            color: controller.status.contains('связи')
                ? const Color(0xFFFFD700)
                : controller.status.contains('подтверждена')
                ? const Color(0xFF43E6D1)
                : null,
          ),
        ),
        if (listening && controller.totalGroups > 0) ...[
          const SizedBox(height: 14),
          LinearProgressIndicator(
            value: controller.progress,
            minHeight: 7,
            borderRadius: BorderRadius.circular(99),
          ),
          const SizedBox(height: 8),
          Text(
            '${controller.completedGroups}/${controller.totalGroups} FEC-групп',
            textAlign: TextAlign.center,
            style: const TextStyle(color: Color(0xFF7E95A7)),
          ),
        ],
        const SizedBox(height: 22),
        if (received != null)
          ReceivedCard(received: received, onReset: controller.reset)
        else if (controller.phase == TransferPhase.error)
          ErrorCard(
            message: controller.error ?? 'Ошибка приёма',
            onRetry: controller.reset,
          )
        else
          FilledButton.icon(
            onPressed: listening ? controller.stop : controller.startReceiving,
            icon: Icon(listening ? Icons.stop_rounded : Icons.mic_rounded),
            label: Text(listening ? 'Остановить' : 'Начать приём'),
            style: primaryPanelButtonStyle(context),
          ),
      ],
    );
  }
}
