import 'package:flutter/material.dart';

BoxDecoration panelCardDecoration({Color color = const Color(0xCC101E2D)}) =>
    BoxDecoration(
      color: color,
      borderRadius: BorderRadius.circular(24),
      border: Border.all(color: const Color(0x1FFFFFFF)),
      boxShadow: const [
        BoxShadow(
          color: Color(0x22000000),
          blurRadius: 24,
          offset: Offset(0, 12),
        ),
      ],
    );

ButtonStyle primaryPanelButtonStyle(BuildContext context) =>
    FilledButton.styleFrom(
      minimumSize: const Size.fromHeight(56),
      backgroundColor: Theme.of(context).colorScheme.primary,
      foregroundColor: const Color(0xFF06121D),
      textStyle: const TextStyle(fontWeight: FontWeight.w800, fontSize: 16),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(17)),
    );
