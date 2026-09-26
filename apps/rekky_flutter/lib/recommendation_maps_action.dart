import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'rekky_api.dart';

/// A search handoff, never a claim that a particular listing was matched.
/// Only a place name and its stated venue location leave Rekky, on a tap.
Uri? recommendationMapsSearch(RekkyItem item) {
  final recommendation = item.recommendation;
  if (recommendation == null ||
      recommendation.destinationMode != 'auto' ||
      recommendation.entityKind != 'place') {
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

Uri? recommendationDestination(RekkyItem item) {
  final r = item.recommendation;
  if (r?.destinationMode != 'custom') return recommendationMapsSearch(item);
  final uri = Uri.tryParse(r!.destinationUrl);
  if (uri == null ||
      uri.scheme != 'https' ||
      uri.host.isEmpty ||
      uri.userInfo.isNotEmpty) {
    return null;
  }
  return uri;
}

Future<bool> _openMaps(Uri uri) =>
    launchUrl(uri, mode: LaunchMode.externalApplication);

class RecommendationMapsAction extends StatefulWidget {
  const RecommendationMapsAction({
    super.key,
    required this.item,
    this.openUrl = _openMaps,
    this.place,
  });

  final RekkyItem item;
  final ResolvedPlace? place;
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
    final matched = widget.item.recommendation?.destinationMode == 'auto'
        ? widget.place
        : null;
    final uri =
        matched?.destination(widget.item.subject) ??
        recommendationDestination(widget.item);
    final custom = widget.item.recommendation?.destinationMode == 'custom';
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
                : Icon(custom ? Icons.open_in_new : Icons.map_outlined),
            style: matched == null
                ? null
                : FilledButton.styleFrom(
                    foregroundColor:
                        Theme.of(context).brightness == Brightness.dark
                        ? Colors.white
                        : const Color(0xff1f1f1f),
                    textStyle: Theme.of(context).textTheme.labelLarge?.copyWith(
                      fontSize: 14,
                      fontWeight: FontWeight.w400,
                      letterSpacing: 0,
                    ),
                  ),
            label: Text(
              matched != null
                  ? 'Google Maps'
                  : opening
                  ? (custom ? 'Opening…' : 'Opening Maps…')
                  : (custom
                        ? widget.item.recommendation!.destinationLabel
                        : matched != null
                        ? 'Google Maps'
                        : 'Search Maps'),
              softWrap: false,
            ),
          ),
          if (failed)
            Semantics(
              liveRegion: true,
              child: Text(
                custom
                    ? 'Couldn’t open the link. Try again.'
                    : 'Couldn’t open Maps. Try again.',
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
        ],
      ),
    );
  }
}
