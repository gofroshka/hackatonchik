import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:sonic_share/main.dart';

void main() {
  testWidgets('switches between file transfer and chat modes', (tester) async {
    await tester.pumpWidget(const SonicShareApp());
    expect(find.text('SONIC SHARE'), findsOneWidget);
    expect(find.text('Отправить'), findsOneWidget);
    expect(find.text('Получить'), findsOneWidget);
    expect(find.text('Чат'), findsOneWidget);
    expect(find.text('Выбрать файл'), findsOneWidget);

    await tester.tap(find.text('Чат'));
    await tester.pumpAndSettle();
    expect(find.text('Акустический чат'), findsOneWidget);
    expect(find.text('Эфир пока пуст'), findsOneWidget);
    expect(find.byType(EditableText), findsOneWidget);
  });
}
