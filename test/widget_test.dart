import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:hackatonchik/shared/widgets/energy_graph.dart';

void main() {
  testWidgets('EnergyGraph renders without error', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(
          body: EnergyGraph(values: [0.1, 0.5, 0.2, 0.9, 0.3]),
        ),
      ),
    );
    expect(find.byType(EnergyGraph), findsOneWidget);
  });

  testWidgets('EnergyGraph handles empty input', (tester) async {
    await tester.pumpWidget(
      const MaterialApp(
        home: Scaffold(body: EnergyGraph(values: [])),
      ),
    );
    expect(find.byType(EnergyGraph), findsOneWidget);
  });
}
