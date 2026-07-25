import 'dart:math' as math;

import 'package:flutter/material.dart';

class Radar extends StatefulWidget {
  const Radar({super.key, required this.level, required this.active});

  final double level;
  final bool active;

  @override
  State<Radar> createState() => _RadarState();
}

class _RadarState extends State<Radar> with SingleTickerProviderStateMixin {
  late final AnimationController animation = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 2),
  )..repeat();

  @override
  void dispose() {
    animation.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 230,
      child: AnimatedBuilder(
        animation: animation,
        builder: (_, _) => CustomPaint(
          painter: _RadarPainter(
            progress: animation.value,
            level: widget.level,
            active: widget.active,
          ),
          child: Center(
            child: Container(
              width: 72,
              height: 72,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: widget.active
                    ? const Color(0xFF43E6D1)
                    : const Color(0xFF203344),
                boxShadow: widget.active
                    ? [
                        BoxShadow(
                          color: const Color(
                            0xFF43E6D1,
                          ).withValues(alpha: 0.3 + widget.level * 0.5),
                          blurRadius: 32 + widget.level * 30,
                        ),
                      ]
                    : null,
              ),
              child: Icon(
                Icons.mic_rounded,
                size: 32,
                color: widget.active
                    ? const Color(0xFF07111E)
                    : const Color(0xFF71899C),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _RadarPainter extends CustomPainter {
  _RadarPainter({
    required this.progress,
    required this.level,
    required this.active,
  });

  final double progress;
  final double level;
  final bool active;

  @override
  void paint(Canvas canvas, Size size) {
    final center = size.center(Offset.zero);
    for (var index = 0; index < 3; index++) {
      final phase = (progress + index / 3) % 1;
      final radius = 45 + phase * 70;
      final alpha = active ? (1 - phase) * (0.18 + level * 0.45) : 0.08;
      canvas.drawCircle(
        center,
        radius,
        Paint()
          ..style = PaintingStyle.stroke
          ..strokeWidth = 1.5
          ..color = const Color(0xFF43E6D1).withValues(alpha: alpha),
      );
    }
    final bars = 25;
    for (var i = 0; i < bars; i++) {
      final angle = i / bars * math.pi * 2;
      final strength = active
          ? 5 +
                level *
                    22 *
                    (0.4 +
                        0.6 *
                            math.sin(angle * 4 + progress * math.pi * 2).abs())
          : 4.0;
      final p1 = center + Offset(math.cos(angle), math.sin(angle)) * 92;
      final p2 =
          center + Offset(math.cos(angle), math.sin(angle)) * (92 + strength);
      canvas.drawLine(
        p1,
        p2,
        Paint()
          ..strokeCap = StrokeCap.round
          ..strokeWidth = 2
          ..color = const Color(
            0xFFFF8B6A,
          ).withValues(alpha: active ? 0.65 : 0.14),
      );
    }
  }

  @override
  bool shouldRepaint(covariant _RadarPainter oldDelegate) =>
      oldDelegate.progress != progress ||
      oldDelegate.level != level ||
      oldDelegate.active != active;
}
