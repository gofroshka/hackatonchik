import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../features/demo/demo_screen.dart';
import '../features/diagnostics/diagnostics_screen.dart';
import '../features/receive/receive_screen.dart';
import '../features/send/send_screen.dart';
import 'theme.dart';

/// Root application widget.
class AcousticModemApp extends StatelessWidget {
  const AcousticModemApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Акустический модем',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.light(),
      darkTheme: AppTheme.dark(),
      home: const _HomeShell(),
    );
  }
}

class _HomeShell extends ConsumerStatefulWidget {
  const _HomeShell();

  @override
  ConsumerState<_HomeShell> createState() => _HomeShellState();
}

class _HomeShellState extends ConsumerState<_HomeShell> {
  int _index = 0;

  static const List<Widget> _screens = [
    SendScreen(),
    ReceiveScreen(),
    DiagnosticsScreen(),
    DemoScreen(),
  ];

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: _screens[_index],
      bottomNavigationBar: NavigationBar(
        selectedIndex: _index,
        onDestinationSelected: (i) => setState(() => _index = i),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.upload),
            label: 'Передача',
          ),
          NavigationDestination(
            icon: Icon(Icons.download),
            label: 'Приём',
          ),
          NavigationDestination(
            icon: Icon(Icons.insights),
            label: 'Диагностика',
          ),
          NavigationDestination(
            icon: Icon(Icons.science),
            label: 'Demo',
          ),
        ],
      ),
    );
  }
}
