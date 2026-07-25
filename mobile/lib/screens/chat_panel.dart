import 'package:flutter/material.dart';
import 'package:sonic_share/controllers/sonic_controller.dart';
import 'package:sonic_share/models/transfer_models.dart';
import 'package:sonic_share/utils/display_formatters.dart';
import 'package:sonic_share/widgets/panel_styles.dart';

class ChatPanel extends StatefulWidget {
  const ChatPanel({super.key, required this.controller});

  final SonicController controller;

  @override
  State<ChatPanel> createState() => _ChatPanelState();
}

class _ChatPanelState extends State<ChatPanel> {
  final input = TextEditingController();

  @override
  void dispose() {
    input.dispose();
    super.dispose();
  }

  void send() {
    final text = input.text.trim();
    if (text.isEmpty) return;
    input.clear();
    widget.controller.sendChatMessage(text);
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final listening = controller.phase == TransferPhase.listening;
    final transmitting =
        controller.phase == TransferPhase.preparing ||
        controller.phase == TransferPhase.sending ||
        controller.phase == TransferPhase.handshaking ||
        controller.phase == TransferPhase.handshakeWaitAck ||
        controller.phase == TransferPhase.endWaitAck;
    final messages = controller.chatMessages;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Expanded(
              child: Text(
                'Акустический чат',
                style: Theme.of(context).textTheme.displaySmall,
              ),
            ),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
              decoration: BoxDecoration(
                color: const Color(0x18FF8B6A),
                borderRadius: BorderRadius.circular(99),
                border: Border.all(color: const Color(0x44FF8B6A)),
              ),
              child: const Text(
                'HALF DUPLEX',
                style: TextStyle(
                  color: Color(0xFFFFA387),
                  fontSize: 10,
                  fontWeight: FontWeight.w800,
                  letterSpacing: 0.8,
                ),
              ),
            ),
          ],
        ),
        const SizedBox(height: 8),
        const Text(
          'Сообщения звучат через динамик. Интернет, Wi-Fi и Bluetooth не используются.',
        ),
        const SizedBox(height: 18),
        _ChatChannelCard(
          controller: controller,
          listening: listening,
          transmitting: transmitting,
        ),
        const SizedBox(height: 14),
        Container(
          constraints: const BoxConstraints(minHeight: 250, maxHeight: 430),
          padding: const EdgeInsets.fromLTRB(14, 18, 14, 12),
          decoration: panelCardDecoration(),
          child: messages.isEmpty
              ? const _EmptyChat()
              : ListView.separated(
                  shrinkWrap: true,
                  reverse: true,
                  itemCount: messages.length,
                  separatorBuilder: (_, _) => const SizedBox(height: 10),
                  itemBuilder: (context, index) {
                    final message = messages[messages.length - index - 1];
                    return _ChatBubble(message: message);
                  },
                ),
        ),
        const SizedBox(height: 14),
        Container(
          padding: const EdgeInsets.fromLTRB(16, 8, 8, 8),
          decoration: BoxDecoration(
            color: const Color(0xFF101E2D),
            borderRadius: BorderRadius.circular(20),
            border: Border.all(color: const Color(0x1FFFFFFF)),
          ),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(
                child: TextField(
                  controller: input,
                  enabled: !transmitting,
                  minLines: 1,
                  maxLines: 4,
                  textCapitalization: TextCapitalization.sentences,
                  textInputAction: TextInputAction.send,
                  onSubmitted: (_) => send(),
                  decoration: const InputDecoration(
                    hintText: 'Сообщение через звук…',
                    border: InputBorder.none,
                    contentPadding: EdgeInsets.symmetric(vertical: 12),
                  ),
                ),
              ),
              const SizedBox(width: 8),
              IconButton.filled(
                onPressed: transmitting ? null : send,
                icon: const Icon(Icons.graphic_eq_rounded),
                tooltip: 'Передать сообщение',
                style: IconButton.styleFrom(
                  backgroundColor: const Color(0xFF43E6D1),
                  foregroundColor: const Color(0xFF07111E),
                  disabledBackgroundColor: const Color(0xFF263A49),
                ),
              ),
            ],
          ),
        ),
        if (controller.phase == TransferPhase.error) ...[
          const SizedBox(height: 12),
          Text(
            controller.error ?? 'Ошибка акустического канала',
            textAlign: TextAlign.center,
            style: const TextStyle(color: Color(0xFFFF7A88), fontSize: 12),
          ),
        ],
      ],
    );
  }
}

class _ChatChannelCard extends StatelessWidget {
  const _ChatChannelCard({
    required this.controller,
    required this.listening,
    required this.transmitting,
  });

  final SonicController controller;
  final bool listening;
  final bool transmitting;

  @override
  Widget build(BuildContext context) {
    final color = transmitting
        ? const Color(0xFFFF8B6A)
        : listening
        ? const Color(0xFF43E6D1)
        : const Color(0xFF71899C);
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(18),
        border: Border.all(color: color.withValues(alpha: 0.25)),
      ),
      child: Row(
        children: [
            Stack(
              alignment: Alignment.center,
              children: [
                SizedBox(
                  width: 44,
                  height: 44,
                  child: CircularProgressIndicator(
                    value: transmitting
                        ? controller.progress
                        : listening
                        ? null
                        : 0,
                    strokeWidth: 3,
                    color: color,
                    backgroundColor: color.withValues(alpha: 0.12),
                  ),
                ),
                Icon(
                  transmitting
                      ? (controller.phase == TransferPhase.handshaking ||
                              controller.phase ==
                                  TransferPhase.handshakeWaitAck)
                          ? Icons.wifi_find_rounded
                          : Icons.volume_up_rounded
                      : listening
                      ? Icons.mic_rounded
                      : Icons.hearing_disabled_rounded,
                  color: color,
                  size: 21,
                ),
              ],
            ),
          const SizedBox(width: 14),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  controller.status,
                  style: const TextStyle(fontWeight: FontWeight.w700),
                ),
                const SizedBox(height: 3),
                Text(
                  transmitting
                      ? 'Пакет ${controller.packetIndex}/${controller.packetCount}'
                      : listening
                      ? 'Уровень сигнала ${(controller.microphoneLevel * 100).round()}%'
                      : 'Канал сейчас не прослушивается',
                  style: const TextStyle(
                    color: Color(0xFF7E95A7),
                    fontSize: 12,
                  ),
                ),
              ],
            ),
          ),
          if (transmitting)
            IconButton(
              onPressed: controller.stop,
              icon: const Icon(Icons.stop_rounded),
              tooltip: 'Остановить отправку',
            )
          else
            TextButton(
              onPressed: listening
                  ? controller.stop
                  : controller.startChatListening,
              child: Text(listening ? 'Пауза' : 'Слушать'),
            ),
        ],
      ),
    );
  }
}

class _EmptyChat extends StatelessWidget {
  const _EmptyChat();

  @override
  Widget build(BuildContext context) => const Center(
    child: Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(Icons.waves_rounded, size: 40, color: Color(0xFF395367)),
        SizedBox(height: 12),
        Text('Эфир пока пуст', style: TextStyle(fontWeight: FontWeight.w700)),
        SizedBox(height: 4),
        Text(
          'Включите прослушивание или отправьте первое сообщение',
          textAlign: TextAlign.center,
          style: TextStyle(color: Color(0xFF71899C), fontSize: 12),
        ),
      ],
    ),
  );
}

class _ChatBubble extends StatelessWidget {
  const _ChatBubble({required this.message});

  final ChatMessage message;

  @override
  Widget build(BuildContext context) {
    final outgoing = message.outgoing;
    final stateIcon = switch (message.state) {
      ChatMessageState.sending => Icons.graphic_eq_rounded,
      ChatMessageState.sent => Icons.done_rounded,
      ChatMessageState.failed => Icons.error_outline_rounded,
    };
    return Align(
      alignment: outgoing ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        constraints: const BoxConstraints(maxWidth: 390),
        padding: const EdgeInsets.fromLTRB(14, 11, 12, 8),
        decoration: BoxDecoration(
          color: outgoing ? const Color(0xFF18463F) : const Color(0xFF192A3A),
          borderRadius: BorderRadius.only(
            topLeft: const Radius.circular(17),
            topRight: const Radius.circular(17),
            bottomLeft: Radius.circular(outgoing ? 17 : 4),
            bottomRight: Radius.circular(outgoing ? 4 : 17),
          ),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(message.text, style: const TextStyle(color: Colors.white)),
            const SizedBox(height: 5),
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  formatTime(message.createdAt),
                  style: const TextStyle(
                    color: Color(0xFF8AA0AF),
                    fontSize: 10,
                  ),
                ),
                if (outgoing) ...[
                  const SizedBox(width: 5),
                  Icon(
                    stateIcon,
                    size: 13,
                    color: message.state == ChatMessageState.failed
                        ? const Color(0xFFFF7A88)
                        : const Color(0xFF74DACD),
                  ),
                ],
              ],
            ),
          ],
        ),
      ),
    );
  }
}
