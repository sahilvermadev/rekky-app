import 'package:flutter/material.dart';

import 'rekky_api.dart';

// Hallmark · component: private quotation · existing warm Material tokens.
// Pre-emit critique: P4 H5 E4 S4 R5 V4. Quote marks are decorative, text selectable.
class OriginalNoteView extends StatefulWidget {
  const OriginalNoteView({super.key, required this.source});
  final RekkySource source;
  @override
  State<OriginalNoteView> createState() => _OriginalNoteViewState();
}

class _OriginalNoteViewState extends State<OriginalNoteView> {
  bool raw = false;
  @override
  void didUpdateWidget(covariant OriginalNoteView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.source != widget.source) raw = false;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final edited = widget.source.readableText;
    final hasEdited =
        edited != null &&
        edited.trim().isNotEmpty &&
        edited != widget.source.text;
    final text = hasEdited && !raw ? edited : widget.source.text;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          width: double.infinity,
          padding: const EdgeInsets.fromLTRB(20, 16, 20, 12),
          decoration: BoxDecoration(
            color: theme.colorScheme.surfaceContainerLow,
            borderRadius: BorderRadius.circular(16),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              ExcludeSemantics(
                child: Text(
                  '“',
                  style: theme.textTheme.displaySmall?.copyWith(
                    color: theme.colorScheme.primary,
                    height: .8,
                  ),
                ),
              ),
              const SizedBox(height: 8),
              SelectableText(
                text,
                style: theme.textTheme.bodyLarge?.copyWith(
                  height: 1.65,
                  letterSpacing: 0,
                ),
              ),
              Align(
                alignment: Alignment.centerRight,
                child: ExcludeSemantics(
                  child: Text(
                    '”',
                    style: theme.textTheme.displaySmall?.copyWith(
                      color: theme.colorScheme.primary,
                      height: 1,
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
        if (hasEdited)
          TextButton(
            onPressed: () => setState(() => raw = !raw),
            child: Text(raw ? 'Back to note' : 'Original transcript'),
          ),
      ],
    );
  }
}
