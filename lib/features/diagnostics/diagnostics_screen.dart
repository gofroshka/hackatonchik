import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../app/providers.dart';
import '../../shared/widgets/energy_graph.dart';

class DiagnosticsScreen extends ConsumerWidget {
  const DiagnosticsScreen({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final controller = ref.watch(receiveControllerProvider);
    final config = ref.watch(modemConfigProvider);
    final diag = controller.diagnostics;

    return Scaffold(
      appBar: AppBar(title: const Text('Диагностика')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const Text('Уровни энергии (live)',
                      style: TextStyle(fontWeight: FontWeight.bold)),
                  const SizedBox(height: 12),
                  EnergyGraph(values: controller.energyHistory),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
          _DiagTable(rows: [
            ('Sample rate', '${config.sampleRate} Гц'),
            ('Размер окна (символ)', '${config.samplesPerSymbol} отсчётов'),
            ('Частота бита 0', '${config.freq0} Гц'),
            ('Частота бита 1', '${config.freq1} Гц'),
            ('Энергия f0', diag.energy0.toStringAsExponential(2)),
            ('Энергия f1', diag.energy1.toStringAsExponential(2)),
            ('Noise floor', diag.noiseFloor.toStringAsExponential(2)),
            ('SNR', diag.snr.toStringAsFixed(2)),
            ('Confidence', diag.confidence.toStringAsFixed(3)),
            ('Preamble score', diag.preambleScore.toStringAsFixed(3)),
            ('Найденный offset', '${diag.offset}'),
            ('Принято битов (всего)', '${controller.totalBits}'),
            ('Пакетов принято', '${controller.totalPackets}'),
            ('CRC статус', diag.crcOk ? 'OK' : '—'),
            ('Ошибок CRC', '${controller.crcErrors}'),
            ('Исправлено битов (repetition)', '${diag.correctedBits}'),
            (
              'Скорость (изм.)',
              '${controller.measuredBitRate.toStringAsFixed(0)} бит/с'
            ),
            (
              'Скорость (теор.)',
              '${config.effectiveBitRate.toStringAsFixed(0)} бит/с'
            ),
          ]),
        ],
      ),
    );
  }
}

class _DiagTable extends StatelessWidget {
  const _DiagTable({required this.rows});
  final List<(String, String)> rows;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        child: Column(
          children: [
            for (final row in rows)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 6),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text(row.$1),
                    Text(
                      row.$2,
                      style: const TextStyle(
                        fontWeight: FontWeight.bold,
                        fontFeatures: [FontFeature.tabularFigures()],
                      ),
                    ),
                  ],
                ),
              ),
          ],
        ),
      ),
    );
  }
}
