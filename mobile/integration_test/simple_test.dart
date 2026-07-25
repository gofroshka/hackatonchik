import 'package:flutter_test/flutter_test.dart';
import 'package:integration_test/integration_test.dart';
import 'package:sonic_share/main.dart';
import 'package:sonic_share/src/rust/api/tx.dart';
import 'package:sonic_share/src/rust/frb_generated.dart';

void main() {
  IntegrationTestWidgetsFlutterBinding.ensureInitialized();
  setUpAll(RustLib.init);

  testWidgets('initializes Rust and renders the app', (tester) async {
    final tx = await TxSession.fromData(
      name: 'test.txt',
      contentType: 'text/plain',
      data: 'hello'.codeUnits,
    );
    final info = await tx.info();
    expect(info.name, 'test.txt');
    await tester.pumpWidget(const SonicShareApp());
    expect(find.text('SONIC SHARE'), findsOneWidget);
  });
}
