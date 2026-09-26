import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'rekky_api.dart';

/// A search handoff, never a claim that a particular listing was matched.
/// Only a place name and its stated venue location leave Rekky, on a tap.
Uri? recommendationMapsSearch(RekkyItem item) {
  final recommendation = item.recommendation;
  if (recommendation == null || recommendation.entityKind != 'place') {
    return null;
  }
  final locations = recommendation.locations
      .where((location) => location.kind == 'venue')
      .map((location) => location.text.trim())
      .where((text) => text.isNotEmpty)
      .toSet();
  // Don't choose a branch from conflicting locations or use the device's city.
  if (locations.length != 1 || item.subject.trim().isEmpty) return null;
  final uri = Uri.https('www.google.com', '/maps/search/', {
    'api': '1',
    'query': '${item.subject.trim()}, ${locations.single}',
  });
  return uri.toString().length <= 2048 ? uri : null;
}

Future<bool> _openMaps(Uri uri) =>
    launchUrl(uri, mode: LaunchMode.externalApplication);

class RecommendationMapsAction extends StatefulWidget {
  const RecommendationMapsAction({
    super.key,
    required this.item,
    this.openUrl = _openMaps,
  });

  final RekkyItem item;
  final Future<bool> Function(Uri) openUrl;

  @override
  State<RecommendationMapsAction> createState() =>
      _RecommendationMapsActionState();
}

class _RecommendationMapsActionState extends State<RecommendationMapsAction> {
  bool opening = false;
  bool failed = false;

  Future<void> _open(Uri uri) async {
    if (opening) return;
    setState(() {
      opening = true;
      failed = false;
    });
    var opened = false;
    try {
      opened = await widget.openUrl(uri).timeout(const Duration(seconds: 10));
    } catch (_) {
      // Native failures can contain a private query: never display or log them.
    }
    if (!mounted) return;
    setState(() {
      opening = false;
      failed = !opened;
    });
  }

  @override
  Widget build(BuildContext context) {
    final uri = recommendationMapsSearch(widget.item);
    if (uri == null) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          FilledButton.tonalIcon(
            onPressed: opening ? null : () => _open(uri),
            icon: opening
                ? const SizedBox.square(
                    dimension: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.map_outlined),
            label: Text(opening ? 'Opening Maps…' : 'Search in Maps'),
          ),
          if (failed)
            Semantics(
              liveRegion: true,
              child: Text(
                'Couldn’t open Maps. Try again.',
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
        ],
      ),
    );
  }
}
