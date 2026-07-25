import 'package:flutter/material.dart';
import 'package:sonic_share/controllers/sonic_controller.dart';
import 'package:sonic_share/screens/home_screen.dart';
import 'package:sonic_share/services/acoustic_audio_service.dart';
import 'package:sonic_share/src/rust/frb_generated.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  runApp(const SonicShareApp());
}

class SonicShareApp extends StatefulWidget {
  const SonicShareApp({super.key});

  @override
  State<SonicShareApp> createState() => _SonicShareAppState();
}

class _SonicShareAppState extends State<SonicShareApp> {
  late final SonicController controller = SonicController(
    AcousticAudioService(),
  );

  @override
  void dispose() {
    controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    const background = Color(0xFF07111E);
    const cyan = Color(0xFF43E6D1);
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'Sonic Share',
      theme: ThemeData(
        brightness: Brightness.dark,
        scaffoldBackgroundColor: background,
        colorScheme: const ColorScheme.dark(
          primary: cyan,
          secondary: Color(0xFFFF8B6A),
          surface: Color(0xFF101E2D),
          error: Color(0xFFFF6B7A),
        ),
        textTheme: const TextTheme(
          displaySmall: TextStyle(
            fontWeight: FontWeight.w800,
            letterSpacing: -1.4,
          ),
          headlineSmall: TextStyle(
            fontWeight: FontWeight.w700,
            letterSpacing: -0.5,
          ),
          titleMedium: TextStyle(fontWeight: FontWeight.w700),
          bodyMedium: TextStyle(color: Color(0xFFB8C7D6), height: 1.4),
        ),
        useMaterial3: true,
      ),
      home: HomeScreen(controller: controller),
    );
  }
}
