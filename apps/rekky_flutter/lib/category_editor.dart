import 'package:flutter/material.dart';

import 'rekky_api.dart';

class CategoryEditor extends StatefulWidget {
  const CategoryEditor({
    super.key,
    required this.item,
    required this.concepts,
    required this.onSave,
    this.saveLabel = 'Save',
  });
  final RekkyItem item;
  final String saveLabel;
  final List<CategoryConcept> concepts;
  final Future<void> Function(List<String>, List<String>) onSave;
  @override
  State<CategoryEditor> createState() => _CategoryEditorState();
}

class _CategoryEditorState extends State<CategoryEditor> {
  late final types =
      widget.item.recommendation!.classification?.types
          .map((c) => c.id)
          .toList() ??
      <String>[];
  late final facets =
      widget.item.recommendation!.classification?.facets
          .map((c) => c.id)
          .toList() ??
      <String>[];
  bool saving = false;
  String? error;

  Future<void> save() async {
    if (saving) return;
    setState(() {
      saving = true;
      error = null;
    });
    try {
      await widget.onSave(List.of(types), List.of(facets));
      if (mounted) Navigator.pop(context, true);
    } catch (e) {
      if (mounted) {
        setState(() {
          error = e is ApiFailure
              ? e.message
              : 'Couldn’t save the category. Try again.';
          saving = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final available = widget.concepts.where(
      (c) => c.entityKinds.contains(widget.item.recommendation!.entityKind),
    );
    Widget options(String dimension, List<String> selected, int limit) => Wrap(
      spacing: 8,
      children: available
          .where((c) => c.dimension == dimension)
          .map(
            (c) => FilterChip(
              label: Text(c.label),
              selected: selected.contains(c.id),
              onSelected:
                  saving ||
                      (!selected.contains(c.id) && selected.length >= limit)
                  ? null
                  : (checked) => setState(() {
                      if (checked) {
                        selected.removeWhere(
                          (id) =>
                              id == c.parentId ||
                              widget.concepts.any(
                                (existing) =>
                                    existing.id == id &&
                                    existing.parentId == c.id,
                              ),
                        );
                        selected.add(c.id);
                      } else {
                        selected.remove(c.id);
                      }
                    }),
            ),
          )
          .toList(),
    );
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(24, 0, 24, 24),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Edit category',
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            const SizedBox(height: 8),
            Flexible(
              child: SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(widget.item.subject),
                    const SizedBox(height: 16),
                    options('type', types, 3),
                    if (types.isNotEmpty)
                      Padding(
                        padding: const EdgeInsets.only(top: 8),
                        child: Text(
                          'Shown on card: ${widget.concepts.firstWhere((c) => c.id == types.first).label}',
                          style: Theme.of(context).textTheme.bodySmall,
                        ),
                      ),
                    for (final dimension
                        in available
                            .where((c) => c.dimension != 'type')
                            .map((c) => c.dimension)
                            .toSet()) ...[
                      const SizedBox(height: 20),
                      Text(
                        available
                            .firstWhere((c) => c.dimension == dimension)
                            .dimensionLabel,
                        style: Theme.of(context).textTheme.titleSmall,
                      ),
                      options(dimension, facets, 4),
                    ],
                    if (error != null)
                      Padding(
                        padding: const EdgeInsets.only(top: 12),
                        child: Semantics(
                          liveRegion: true,
                          child: Text(
                            error!,
                            style: TextStyle(
                              color: Theme.of(context).colorScheme.error,
                            ),
                          ),
                        ),
                      ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 16),
            FilledButton(
              onPressed: saving ? null : save,
              child: Text(saving ? 'Saving…' : widget.saveLabel),
            ),
          ],
        ),
      ),
    );
  }
}
