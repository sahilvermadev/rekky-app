import 'package:flutter/material.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';

import 'library_style.dart';

abstract final class AppearancePreference {
  static const _key = 'rekky_theme_mode_v1';
  static const _storage = FlutterSecureStorage();

  static Future<ThemeMode> load() async {
    try {
      final saved = await _storage.read(key: _key);
      return ThemeMode.values.where((mode) => mode.name == saved).firstOrNull ??
          ThemeMode.system;
    } catch (_) {
      return ThemeMode.system;
    }
  }

  static Future<void> save(ThemeMode mode) =>
      _storage.write(key: _key, value: mode.name);
}

class AppearanceSheet extends StatefulWidget {
  const AppearanceSheet({
    super.key,
    required this.mode,
    required this.onChanged,
  });
  final ThemeMode mode;
  final Future<void> Function(ThemeMode) onChanged;

  @override
  State<AppearanceSheet> createState() => _AppearanceSheetState();
}

class _AppearanceSheetState extends State<AppearanceSheet> {
  bool saving = false;
  String? error;

  Future<void> select(ThemeMode mode) async {
    if (saving) return;
    setState(() {
      saving = true;
      error = null;
    });
    try {
      await widget.onChanged(mode);
      if (mounted) Navigator.pop(context);
    } catch (_) {
      if (mounted) {
        setState(() {
          saving = false;
          error = 'Couldn’t save your appearance. Try again.';
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) => SafeArea(
    child: SingleChildScrollView(
      padding: const EdgeInsets.fromLTRB(20, 0, 20, 20),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('Appearance', style: LibraryStyle.heading(context, 28)),
          const SizedBox(height: 12),
          for (final mode in ThemeMode.values)
            ListTile(
              enabled: !saving,
              contentPadding: const EdgeInsets.symmetric(horizontal: 4),
              leading: Icon(switch (mode) {
                ThemeMode.system => Icons.brightness_auto_outlined,
                ThemeMode.light => Icons.light_mode_outlined,
                ThemeMode.dark => Icons.dark_mode_outlined,
              }),
              title: Text(switch (mode) {
                ThemeMode.system => 'System',
                ThemeMode.light => 'Light',
                ThemeMode.dark => 'Dark',
              }),
              subtitle: mode == ThemeMode.system
                  ? const Text('Follow your phone’s setting')
                  : null,
              trailing: widget.mode == mode ? const Icon(Icons.check) : null,
              selected: widget.mode == mode,
              onTap: () => select(mode),
            ),
          if (saving) const LinearProgressIndicator(),
          if (error != null)
            Text(
              error!,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
        ],
      ),
    ),
  );
}
