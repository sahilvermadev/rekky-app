import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import 'contact_matching.dart';

class ContactAction extends StatelessWidget {
  const ContactAction({super.key, this.phone, this.savedName, this.onManage});
  final String? phone, savedName;
  final VoidCallback? onManage;
  @override
  Widget build(BuildContext context) {
    if (phone == null) {
      return onManage == null
          ? const SizedBox.shrink()
          : TextButton.icon(
              onPressed: onManage,
              icon: const Icon(Icons.person_add_alt_outlined),
              label: const Text('Add contact'),
            );
    }
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (savedName != null && savedName!.isNotEmpty)
            Padding(
              padding: const EdgeInsets.only(bottom: 4),
              child: Text(
                savedName!,
                style: Theme.of(context).textTheme.titleSmall,
              ),
            ),
          Wrap(
            crossAxisAlignment: WrapCrossAlignment.center,
            spacing: 8,
            children: [
              OutlinedButton.icon(
                onPressed: () async {
                  final number = internationalPhone(phone!);
                  bool opened = false;
                  if (number != null) {
                    try {
                      opened = await launchUrl(
                        Uri(scheme: 'tel', path: number),
                      );
                    } catch (_) {}
                  }
                  if (!opened && context.mounted) {
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(
                        content: Text(
                          'Couldn’t open the dialer. You can copy the number.',
                        ),
                      ),
                    );
                  }
                },
                icon: const Icon(Icons.call_outlined),
                label: const Text('Call'),
              ),
              SelectableText(phone!),
              if (onManage != null)
                IconButton(
                  tooltip: 'Change contact',
                  onPressed: onManage,
                  icon: const Icon(Icons.edit_outlined, size: 18),
                ),
            ],
          ),
        ],
      ),
    );
  }
}
