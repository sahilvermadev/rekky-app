import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart';

import 'rekky_api.dart';
import 'library_style.dart';
import 'library_collection.dart';
import 'original_note_view.dart';
import 'recommendation_view.dart';
import 'recommendation_review.dart';

// Hallmark · personal field guide · shared type and collection emblems.
// Pre-emit critique: P4 H5 E4 S4 R5 V4. Native focus/interaction states.
class RecommendationDetailSheet extends StatefulWidget {
  const RecommendationDetailSheet({
    super.key,
    required this.item,
    required this.loadSource,
    this.loadPlace,
    this.manageContact,
    this.changePin,
    this.contactUpdates,
    required this.changeAudience,
    required this.deleteItem,
    required this.editRecommendation,
    required this.refineItem,
  });
  final RekkyItem item;
  final Future<RekkyItem> Function(RekkyItem, bool)? changePin;
  final ValueListenable<RekkyItem>? contactUpdates;
  final Future<RekkyItem?> Function(RekkyItem)? manageContact;
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

  @override
  void initState() {
    super.initState();
    widget.contactUpdates?.addListener(contactUpdated);
    loadSource();
    loadPlace();
  }

  void contactUpdated() {
    final updated = widget.contactUpdates?.value;
    if (mounted &&
        updated != null &&
        updated.id == item.id &&
        (updated.revision > item.revision ||
            updated.pinRevision > item.pinRevision)) {
      setState(() {
        final latestContent = updated.revision > item.revision ? updated : item;
        final latestPin = updated.pinRevision > item.pinRevision
            ? updated
            : item;
        item = latestContent.withPin(latestPin.pinned, latestPin.pinRevision);
      });
    }
  }

  @override
  void dispose() {
    widget.contactUpdates?.removeListener(contactUpdated);
    super.dispose();
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

  Future<void> pin() async {
    if (saving || widget.changePin == null) return;
    setState(() {
      saving = true;
      actionError = null;
    });
    try {
      final updated = await widget.changePin!(item, !item.pinned);
      if (mounted) setState(() => item = updated);
    } catch (error) {
      if (mounted) {
        setState(
          () => actionError = errorText(
            error,
            'Couldn’t save the pin. Try again.',
          ),
        );
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
                if (RecommendationReviewButton.needed(item))
                  RecommendationReviewButton(
                    item: item,
                    onEdit: saving
                        ? null
                        : () {
                            Navigator.pop(context);
                            widget.editRecommendation(item);
                          },
                  ),
                PopupMenuButton<String>(
                  enabled: !saving,
                  tooltip: 'Recommendation options',
                  icon: const Icon(Icons.more_horiz),
                  onSelected: (value) {
                    if (value == 'pin') {
                      pin();
                      return;
                    }
                    if (value == 'delete') {
                      remove();
                      return;
                    }
                    Navigator.pop(context);
                    widget.editRecommendation(item);
                  },
                  itemBuilder: (_) => [
                    if (widget.changePin != null)
                      PopupMenuItem(
                        value: 'pin',
                        child: Text(
                          item.pinned ? 'Unpin from Library' : 'Pin in Library',
                        ),
                      ),
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
                  Row(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Expanded(
                        child: Text(
                          item.subject,
                          style: LibraryStyle.heading(context, 30),
                        ),
                      ),
                      const SizedBox(width: 12),
                      ExcludeSemantics(
                        child: Container(
                          width: 44,
                          height: 48,
                          alignment: Alignment.center,
                          decoration: BoxDecoration(
                            color: LibraryStyle.tint(
                              context,
                              LibraryShelf.of(item),
                            ),
                            borderRadius: BorderRadius.circular(14),
                          ),
                          child: Icon(
                            LibraryStyle.itemIcon(item),
                            size: 24,
                            color: LibraryStyle.accent(
                              context,
                              LibraryShelf.of(item),
                            ),
                          ),
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 10),
                  RecommendationView(
                    item: item,
                    expanded: true,
                    place: place,
                    onManageContact: widget.manageContact == null || saving
                        ? null
                        : () async {
                            setState(() {
                              saving = true;
                              actionError = null;
                            });
                            try {
                              final updated = await widget.manageContact!(item);
                              if (mounted && updated != null) {
                                setState(() => item = updated);
                              }
                            } catch (e) {
                              if (mounted) {
                                setState(
                                  () => actionError = errorText(
                                    e,
                                    'Couldn’t update contact.',
                                  ),
                                );
                              }
                            } finally {
                              if (mounted) setState(() => saving = false);
                            }
                          },
                  ),
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
                  // Owner-only quote loads independently of the saved content.
                  Column(
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
