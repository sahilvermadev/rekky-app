import 'package:flutter/material.dart';

import 'library_style.dart';

class AskScreen extends StatelessWidget {
  const AskScreen({
    super.key,
    required this.controller,
    required this.matches,
    required this.searching,
    required this.onAsk,
    required this.onOpen,
  });
  final TextEditingController controller;
  final List<Map<String, dynamic>> matches;
  final bool searching;
  final VoidCallback onAsk;
  final ValueChanged<String> onOpen;
  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return CustomScrollView(
      key: const PageStorageKey('ask-scroll'),
      keyboardDismissBehavior: ScrollViewKeyboardDismissBehavior.onDrag,
      slivers: [
        SliverPadding(
          padding: const EdgeInsets.fromLTRB(20, 12, 20, 20),
          sliver: SliverToBoxAdapter(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'What are you trying to remember?',
                  style: LibraryStyle.heading(context, 30),
                ),
                const SizedBox(height: 24),
                TextField(
                  controller: controller,
                  textInputAction: TextInputAction.search,
                  onSubmitted: (_) => onAsk(),
                  decoration: InputDecoration(
                    hintText: 'Who fixed our kitchen tap?',
                    prefixIcon: const Icon(Icons.search, size: 22),
                    suffixIcon: IconButton(
                      tooltip: 'Search',
                      onPressed: searching ? null : onAsk,
                      icon: const Icon(Icons.arrow_forward),
                    ),
                  ),
                ),
                const SizedBox(height: 12),
                Text(
                  'Search your saved recommendations.',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
                if (searching)
                  const Padding(
                    padding: EdgeInsets.only(top: 16),
                    child: LinearProgressIndicator(semanticsLabel: 'Searching'),
                  ),
              ],
            ),
          ),
        ),
        if (matches.isEmpty)
          SliverFillRemaining(
            hasScrollBody: false,
            child: Padding(
              padding: const EdgeInsets.all(24),
              child: Center(
                child: Text(
                  controller.text.isEmpty
                      ? 'Ask about something you saved.'
                      : searching
                      ? 'Looking through your library…'
                      : 'No matching memory yet.',
                  style: theme.textTheme.bodyMedium?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                  textAlign: TextAlign.center,
                ),
              ),
            ),
          )
        else
          SliverPadding(
            padding: const EdgeInsets.fromLTRB(20, 0, 20, 24),
            sliver: SliverList.builder(
              itemCount: matches.length,
              itemBuilder: (context, index) {
                final match = matches[index];
                return Padding(
                  padding: const EdgeInsets.only(bottom: 8),
                  child: Material(
                    color: theme.colorScheme.surfaceContainerLow,
                    shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(16),
                      side: BorderSide(
                        color: theme.colorScheme.outlineVariant.withValues(
                          alpha: .35,
                        ),
                      ),
                    ),
                    clipBehavior: Clip.antiAlias,
                    child: InkWell(
                      onTap: () => onOpen(match['item_id'] as String),
                      child: Padding(
                        padding: const EdgeInsets.all(16),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              match['subject'] as String,
                              style: theme.textTheme.titleMedium,
                            ),
                            const SizedBox(height: 8),
                            Text(
                              match['body'] as String,
                              maxLines: 3,
                              overflow: TextOverflow.ellipsis,
                              style: theme.textTheme.bodyMedium?.copyWith(
                                color: theme.colorScheme.onSurfaceVariant,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
                );
              },
            ),
          ),
      ],
    );
  }
}
