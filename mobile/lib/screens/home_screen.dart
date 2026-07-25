import 'package:flutter/material.dart';
import 'package:sonic_share/controllers/sonic_controller.dart';
import 'package:sonic_share/models/transfer_models.dart';
import 'package:sonic_share/screens/chat_panel.dart';
import 'package:sonic_share/screens/receive_panel.dart';
import 'package:sonic_share/screens/send_panel.dart';

class HomeScreen extends StatelessWidget {
  const HomeScreen({super.key, required this.controller});

  final SonicController controller;

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) => Scaffold(
        body: Container(
          decoration: const BoxDecoration(
            gradient: RadialGradient(
              center: Alignment(-0.7, -0.8),
              radius: 1.4,
              colors: [Color(0xFF123044), Color(0xFF07111E)],
            ),
          ),
          child: SafeArea(
            child: Center(
              child: ConstrainedBox(
                constraints: const BoxConstraints(maxWidth: 620),
                child: ListView(
                  padding: const EdgeInsets.fromLTRB(22, 20, 22, 36),
                  children: [
                    const _Header(),
                    const SizedBox(height: 28),
                    _ModeSelector(controller: controller),
                    const SizedBox(height: 12),
                    _ProfileSelector(controller: controller),
                    const SizedBox(height: 28),
                    AnimatedSwitcher(
                      duration: const Duration(milliseconds: 320),
                      child: switch (controller.mode) {
                        SonicMode.send => SendPanel(
                          key: const ValueKey('send'),
                          controller: controller,
                        ),
                        SonicMode.receive => ReceivePanel(
                          key: const ValueKey('receive'),
                          controller: controller,
                        ),
                        SonicMode.chat => ChatPanel(
                          key: const ValueKey('chat'),
                          controller: controller,
                        ),
                      },
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _ProfileSelector extends StatelessWidget {
  const _ProfileSelector({required this.controller});

  final SonicController controller;

  @override
  Widget build(BuildContext context) => Row(
    children: [
      Expanded(
        child: SegmentedButton<bool>(
          segments: const [
            ButtonSegment(value: false, label: Text('OFDM')),
            ButtonSegment(value: true, label: Text('Robust FSK')),
          ],
          selected: {controller.robustProfile},
          onSelectionChanged: controller.isBusy
              ? null
              : (value) => controller.setRobustProfile(value.first),
          showSelectedIcon: false,
        ),
      ),
      const SizedBox(width: 10),
      SegmentedButton<int>(
        segments: const [
          ButtonSegment(value: 0, label: Text('L0')),
          ButtonSegment(value: 1, label: Text('L1')),
        ],
        selected: {controller.acousticLane},
        onSelectionChanged: controller.isBusy
            ? null
            : (value) => controller.setAcousticLane(value.first),
        showSelectedIcon: false,
      ),
    ],
  );
}

class _Header extends StatelessWidget {
  const _Header();

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Container(
          width: 48,
          height: 48,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(15),
            color: Theme.of(context).colorScheme.primary,
            boxShadow: const [
              BoxShadow(color: Color(0x5543E6D1), blurRadius: 24),
            ],
          ),
          child: const Icon(
            Icons.graphic_eq_rounded,
            color: Color(0xFF07111E),
            size: 30,
          ),
        ),
        const SizedBox(width: 14),
        const Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'SONIC SHARE',
                style: TextStyle(
                  fontWeight: FontWeight.w900,
                  letterSpacing: 2.2,
                ),
              ),
              SizedBox(height: 3),
              Text(
                'Файлы и сообщения через звук',
                style: TextStyle(color: Color(0xFF7F97AA)),
              ),
            ],
          ),
        ),
        const _ProtocolBadge(),
      ],
    );
  }
}

class _ProtocolBadge extends StatelessWidget {
  const _ProtocolBadge();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
      decoration: BoxDecoration(
        border: Border.all(color: const Color(0x3343E6D1)),
        borderRadius: BorderRadius.circular(99),
        color: const Color(0x1243E6D1),
      ),
      child: const Text(
        'OFDM + FEC',
        style: TextStyle(color: Color(0xFF43E6D1), fontSize: 11),
      ),
    );
  }
}

class _ModeSelector extends StatelessWidget {
  const _ModeSelector({required this.controller});

  final SonicController controller;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(5),
      decoration: BoxDecoration(
        color: const Color(0xFF0A1724),
        borderRadius: BorderRadius.circular(18),
      ),
      child: Row(
        children: [
          _ModeButton(
            icon: Icons.north_east_rounded,
            label: 'Отправить',
            selected: controller.mode == SonicMode.send,
            onTap: () => controller.setMode(SonicMode.send),
          ),
          _ModeButton(
            icon: Icons.south_west_rounded,
            label: 'Получить',
            selected: controller.mode == SonicMode.receive,
            onTap: () => controller.setMode(SonicMode.receive),
          ),
          _ModeButton(
            icon: Icons.forum_rounded,
            label: 'Чат',
            selected: controller.mode == SonicMode.chat,
            onTap: () => controller.setMode(SonicMode.chat),
          ),
        ],
      ),
    );
  }
}

class _ModeButton extends StatelessWidget {
  const _ModeButton({
    required this.icon,
    required this.label,
    required this.selected,
    required this.onTap,
  });

  final IconData icon;
  final String label;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Expanded(
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: onTap,
        child: AnimatedContainer(
          duration: const Duration(milliseconds: 220),
          padding: const EdgeInsets.symmetric(vertical: 13),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            color: selected ? const Color(0xFF183448) : Colors.transparent,
          ),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(
                icon,
                size: 19,
                color: selected
                    ? const Color(0xFF43E6D1)
                    : const Color(0xFF6F8799),
              ),
              const SizedBox(width: 8),
              Text(
                label,
                style: TextStyle(
                  fontWeight: FontWeight.w700,
                  color: selected ? Colors.white : const Color(0xFF8297A8),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
