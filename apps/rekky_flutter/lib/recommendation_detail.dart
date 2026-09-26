import 'package:flutter/material.dart';

import 'rekky_api.dart';
import 'original_note_view.dart';
import 'recommendation_view.dart';

// Hallmark · component: reading sheet · existing warm Material tokens.
// Pre-emit critique: P4 H5 E4 S4 R5 V4. Native focus/interaction states.
class RecommendationDetailSheet extends StatefulWidget {
  const RecommendationDetailSheet({
    super.key,
    required this.item,
    required this.loadSource,
    this.loadPlace,
    required this.changeAudience,
    required this.deleteItem,
    required this.editRecommendation,
    required this.refineItem,
  });
  final RekkyItem item;
  final Future<RekkySource?> Function() loadSource;
  final Future<ResolvedPlace?> Function()? loadPlace;
  final Future<RekkyItem> Function(RekkyItem, String) changeAudience;
  final Future<void> Function(RekkyItem) deleteItem;
  final void Function(RekkyItem) editRecommendation, refineItem;

  @override
  State<RecommendationDetailSheet> createState() =>
      _RecommendationDetailSheetState();
}

class _RecommendationDetailSheetState extends State<RecommendationDetailSheet> {
  late RekkyItem item = widget.item;
  RekkySource? source;
  bool sourceLoaded = false, sourceLoading = false;
  ResolvedPlace? place;
  bool saving = false;
  String? sourceError, actionError;
  final sourceKey = GlobalKey();

  @override
  void initState() {
    super.initState();
    loadSource();
    loadPlace();
  }

  Future<void> loadPlace() async {
    try {
      final loaded = await widget.loadPlace?.call();
      if (mounted) setState(() => place = loaded);
    } catch (_) {
      // A missing/slow external address never blocks the saved recommendation.
    }
  }

  String errorText(Object error, String fallback) =>
      error is ApiFailure ? error.message : fallback;

  Future<void> loadSource() async {
    if (sourceLoading || sourceLoaded) return;
    setState(() {
      sourceLoading = true;
      sourceError = null;
    });
    try {
      final loaded = await widget.loadSource();
      if (!mounted) return;
      setState(() {
        source = loaded;
        sourceLoaded = true;
      });
    } catch (error) {
      if (mounted) {
        setState(() {
          sourceError = errorText(
            error,
            'Couldn’t load your original note. Try again.',
          );
        });
      }
    } finally {
      if (mounted) setState(() => sourceLoading = false);
    }
  }

  void openOriginal() {
    loadSource();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted && sourceKey.currentContext != null) {
        Scrollable.ensureVisible(sourceKey.currentContext!);
      }
    });
  }

  Future<void> audience(String value) async {
    if (saving || item.visibility == value) return;
    setState(() {
      saving = true;
      actionError = null;
    });
    try {
      final updated = await widget.changeAudience(item, value);
      if (mounted) setState(() => item = updated);
    } catch (error) {
      if (mounted) {
        setState(() {
          actionError = errorText(
            error,
            'Couldn’t change who can see this. Try again.',
          );
        });
      }
    } finally {
      if (mounted) setState(() => saving = false);
    }
  }

  Future<void> remove() async {
    if (saving) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Delete recommendation?'),
        content: const Text(
          'This removes the recommendation. Its original note is also deleted if no other saved recommendation uses it.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Cancel'),
          ),
          TextButton(
            style: TextButton.styleFrom(
              foregroundColor: Theme.of(context).colorScheme.error,
            ),
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Delete'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted || saving) return;
    setState(() {
      saving = true;
      actionError = null;
    });
    try {
      await widget.deleteItem(item);
      if (mounted) Navigator.pop(context);
    } catch (error) {
      if (mounted) {
        setState(() {
          actionError = errorText(error, 'Couldn’t delete it. Try again.');
        });
      }
    } finally {
      if (mounted) setState(() => saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return SafeArea(
      top: false,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              children: [
                Expanded(
                  child: Align(
                    alignment: Alignment.centerLeft,
                    child: PopupMenuButton<String>(
                      enabled: !saving,
                      tooltip: 'Who can see this',
                      onSelected: audience,
                      itemBuilder: (_) => [
                        CheckedPopupMenuItem(
                          value: 'private',
                          checked: item.visibility == 'private',
                          child: const Text('Only me'),
                        ),
                        CheckedPopupMenuItem(
                          value: 'friends',
                          checked: item.visibility == 'friends',
                          child: const Text('Friends'),
                        ),
                      ],
                      child: Padding(
                        padding: const EdgeInsets.symmetric(
                          horizontal: 8,
                          vertical: 12,
                        ),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            Icon(
                              item.visibility == 'private'
                                  ? Icons.lock_outline
                                  : Icons.people_outline,
                              size: 18,
                              color: theme.colorScheme.onSurfaceVariant,
                            ),
                            const SizedBox(width: 6),
                            Flexible(
                              child: Text(
                                item.visibility == 'private'
                                    ? 'Only me'
                                    : 'Friends',
                                style: theme.textTheme.labelLarge,
                              ),
                            ),
                            const Icon(Icons.expand_more, size: 18),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
                if (saving)
                  const SizedBox.square(
                    dimension: 16,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  ),
                PopupMenuButton<String>(
                  enabled: !saving,
                  tooltip: 'Recommendation options',
                  icon: const Icon(Icons.more_horiz),
                  onSelected: (value) {
                    if (value == 'delete') {
                      remove();
                      return;
                    }
                    Navigator.pop(context);
                    widget.editRecommendation(item);
                  },
                  itemBuilder: (_) => [
                    const PopupMenuItem(
                      value: 'edit',
                      child: Text('Edit recommendation'),
                    ),
                    PopupMenuItem(
                      value: 'delete',
                      child: Text(
                        'Delete recommendation',
                        style: TextStyle(color: theme.colorScheme.error),
                      ),
                    ),
                  ],
                ),
                IconButton(
                  tooltip: 'Close recommendation',
                  onPressed: () => Navigator.pop(context),
                  icon: const Icon(Icons.close),
                ),
              ],
            ),
          ),
          Flexible(
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(24, 8, 24, 24),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    item.subject,
                    style: theme.textTheme.headlineMedium?.copyWith(
                      fontWeight: FontWeight.w600,
                      height: 1.15,
                      letterSpacing: -.5,
                    ),
                  ),
                  const SizedBox(height: 10),
                  RecommendationView(item: item, expanded: true, place: place),
                  if (actionError != null) ...[
                    const SizedBox(height: 16),
                    Semantics(
                      liveRegion: true,
                      child: Text(
                        actionError!,
                        style: TextStyle(color: theme.colorScheme.error),
                      ),
                    ),
                  ],
                  if (item.needsReview) ...[
                    const SizedBox(height: 24),
                    Text(
                      'This saved note may be incomplete.',
                      style: theme.textTheme.titleSmall,
                    ),
                    const SizedBox(height: 4),
                    Text(
                      'Compare it with your original note.',
                      style: theme.textTheme.bodyMedium,
                    ),
                    TextButton(
                      onPressed: openOriginal,
                      child: const Text('View original note'),
                    ),
                  ],
                  // Owner-only quote loads independently of the saved content.
                  Column(
                    key: sourceKey,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      if (sourceLoading)
                        const Padding(
                          padding: EdgeInsets.symmetric(vertical: 20),
                          child: LinearProgressIndicator(
                            semanticsLabel: 'Loading note',
                          ),
                        ),
                      if (sourceError != null) ...[
                        const SizedBox(height: 20),
                        Text(
                          sourceError!,
                          style: TextStyle(color: theme.colorScheme.error),
                        ),
                        TextButton(
                          onPressed: loadSource,
                          child: const Text('Retry'),
                        ),
                      ],
                      if (source != null) ...[
                        const SizedBox(height: 24),
                        OriginalNoteView(source: source!),
                        if (source!.kind == 'transcript' &&
                            item.recommendation == null)
                          TextButton(
                            onPressed: saving
                                ? null
                                : () {
                                    Navigator.pop(context);
                                    widget.refineItem(item);
                                  },
                            child: const Text('Update recommendation'),
                          ),
                      ],
                    ],
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}
