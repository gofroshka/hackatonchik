import 'package:flutter/material.dart';

/// A lightweight live bar/line graph of recent energy (or level) values.
///
/// Rendered with a [CustomPainter] to stay cheap even at high refresh rates.
class EnergyGraph extends StatelessWidget {
  const EnergyGraph({
    super.key,
    required this.values,
    this.height = 120,
  });

  final List<double> values;
  final double height;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return SizedBox(
      height: height,
      width: double.infinity,
      child: CustomPaint(
        painter: _EnergyPainter(
          values: values,
          lineColor: scheme.primary,
          fillColor: scheme.primary.withValues(alpha: 0.15),
          gridColor: scheme.outlineVariant.withValues(alpha: 0.4),
        ),
      ),
    );
  }
}

class _EnergyPainter extends CustomPainter {
  _EnergyPainter({
    required this.values,
    required this.lineColor,
    required this.fillColor,
    required this.gridColor,
  });

  final List<double> values;
  final Color lineColor;
  final Color fillColor;
  final Color gridColor;

  @override
  void paint(Canvas canvas, Size size) {
    final gridPaint = Paint()
      ..color = gridColor
      ..strokeWidth = 1;
    for (int i = 1; i < 4; i++) {
      final y = size.height * i / 4;
      canvas.drawLine(Offset(0, y), Offset(size.width, y), gridPaint);
    }

    if (values.isEmpty) return;

    double maxValue = 0;
    for (final v in values) {
      if (v > maxValue) maxValue = v;
    }
    if (maxValue <= 0) maxValue = 1;

    final path = Path();
    final fillPath = Path()..moveTo(0, size.height);
    final dx = size.width / (values.length - 1).clamp(1, double.infinity);

    for (int i = 0; i < values.length; i++) {
      final norm = (values[i] / maxValue).clamp(0.0, 1.0);
      final x = dx * i;
      final y = size.height - norm * size.height;
      if (i == 0) {
        path.moveTo(x, y);
      } else {
        path.lineTo(x, y);
      }
      fillPath.lineTo(x, y);
    }
    fillPath
      ..lineTo(size.width, size.height)
      ..close();

    canvas.drawPath(fillPath, Paint()..color = fillColor);
    canvas.drawPath(
      path,
      Paint()
        ..color = lineColor
        ..style = PaintingStyle.stroke
        ..strokeWidth = 2,
    );
  }

  @override
  bool shouldRepaint(covariant _EnergyPainter oldDelegate) => true;
}
