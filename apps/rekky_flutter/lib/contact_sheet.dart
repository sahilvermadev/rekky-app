import 'package:flutter/material.dart';
import 'package:flutter_contacts/flutter_contacts.dart';

import 'contact_matching.dart';
import 'rekky_api.dart';

class ContactSheet extends StatefulWidget {
  const ContactSheet({
    super.key,
    required this.item,
    required this.book,
    required this.enableMatching,
    required this.save,
  });
  final RekkyItem item;
  final ContactBook book;
  final Future<int?> Function() enableMatching;
  final Future<RekkyItem> Function(Map<String, dynamic>) save;
  @override
  State<ContactSheet> createState() => _ContactSheetState();
}

class _ContactSheetState extends State<ContactSheet> {
  late final phone = TextEditingController(
    text: widget.item.recommendation?.contactPhone ?? '',
  );
  late String? savedName = widget.item.recommendation?.contactSavedName;
  bool busy = false;
  String? error;
  List<LocalContact>? candidates;
  @override
  void dispose() {
    phone.dispose();
    super.dispose();
  }

  Future<void> find() async {
    setState(() {
      busy = true;
      error = null;
    });
    try {
      final generation = await widget.enableMatching();
      if (generation == null) return;
      final contacts = await widget.book.read();
      if (!mounted) return;
      final automatic = automaticContactMatch(
        widget.item.subject,
        contacts,
        recommendation: widget.item.recommendation,
      );
      if (automatic != null &&
          widget.item.recommendation?.contactPhone == null &&
          widget.item.recommendation?.contactMatchingOff != true) {
        // Explicit contact setup also completes the current recommendation.
        if (!await widget.book.hasAccess() || !mounted) return;
        final saved = await widget.save({
          'mode': 'automatic',
          'phone': automatic.phone,
          'saved_name': automatic.contact.name.trim(),
          'generation': generation,
        });
        if (mounted) Navigator.pop(context, saved);
        return;
      }
      setState(
        () => candidates = contactCandidates(
          widget.item.subject,
          contacts,
          recommendation: widget.item.recommendation,
        ),
      );
    } catch (_) {
      if (mounted) {
        setState(
          () => error = 'Couldn’t find contacts. You can enter a number below.',
        );
      }
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> pick() async {
    setState(() {
      busy = true;
      error = null;
    });
    try {
      if (!await widget.book.requestAccess()) {
        if (mounted) {
          setState(
            () => error = 'Contact access is off. Enter a number instead, or allow Contacts in phone settings.',
          );
        }
        return;
      }
      final selected = await FlutterContacts.native.showPicker(
        properties: {ContactProperty.name, ContactProperty.phone},
      );
      if (!mounted || selected == null) return;
      final contact = LocalContact.fromNative(selected);
      setState(() => candidates = [contact]);
    } catch (_) {
      if (mounted) {
        setState(() => error = 'Couldn’t open contacts. Enter a number below.');
      }
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  Future<void> save({bool remove = false}) async {
    final number = internationalPhone(phone.text);
    if (!remove && number == null) {
      setState(
        () => error =
            'Include + and the country code, for example +91 98765 43210.',
      );
      return;
    }
    setState(() {
      busy = true;
      error = null;
    });
    try {
      final updated = await widget.save(
        remove
            ? {'mode': 'none'}
            : {
                'mode': 'set',
                'phone': number,
                if (savedName != null) 'saved_name': savedName,
              },
      );
      if (mounted) Navigator.pop(context, updated);
    } catch (e) {
      if (mounted) {
        setState(
          () => error = e is ApiFailure
              ? e.message
              : 'Couldn’t save. Please try again.',
        );
      }
    } finally {
      if (mounted) setState(() => busy = false);
    }
  }

  @override
  Widget build(BuildContext context) => PopScope(
    canPop: !busy,
    child: Padding(
      padding: EdgeInsets.fromLTRB(
        24,
        0,
        24,
        24 + MediaQuery.viewInsetsOf(context).bottom,
      ),
      child: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              widget.item.subject,
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 8),
            Text(
              widget.item.visibility == 'friends'
                  ? 'Friends can see and use the attached number.'
                  : 'Only you can see it. Sharing this recommendation also shares its number.',
            ),
            const SizedBox(height: 16),
            Wrap(
              spacing: 8,
              children: [
                OutlinedButton.icon(
                  onPressed: busy ? null : find,
                  icon: const Icon(Icons.person_search_outlined),
                  label: const Text('Find a saved contact'),
                ),
                TextButton(
                  onPressed: busy ? null : pick,
                  child: const Text('Choose from contacts'),
                ),
              ],
            ),
            if (candidates != null) ...[
              const SizedBox(height: 8),
              Text(
                candidates!.isEmpty
                    ? 'No matching contact. Choose one or enter a number.'
                    : 'Choose the provider’s number',
              ),
              for (final candidate in candidates!)
                for (final number in candidate.phones)
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    title: Text(candidate.name),
                    subtitle: Text(number),
                    trailing: const Icon(Icons.add),
                    onTap: busy
                        ? null
                        : () {
                            setState(() {
                              phone.text = number;
                              savedName = candidate.name.trim();
                              error = null;
                            });
                          },
                  ),
            ],
            const SizedBox(height: 16),
            TextField(
              controller: phone,
              enabled: !busy,
              keyboardType: TextInputType.phone,
              onChanged: (_) => setState(() => savedName = null),
              decoration: const InputDecoration(
                labelText: 'Phone number',
                hintText: '+91 …',
                border: OutlineInputBorder(),
              ),
            ),
            if (savedName != null)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(savedName!),
              ),
            if (error != null)
              Padding(
                padding: const EdgeInsets.only(top: 12),
                child: Text(
                  error!,
                  style: TextStyle(color: Theme.of(context).colorScheme.error),
                ),
              ),
            const SizedBox(height: 16),
            Row(
              children: [
                if (widget.item.recommendation?.contactPhone != null)
                  TextButton(
                    onPressed: busy ? null : () => save(remove: true),
                    child: const Text('Remove number'),
                  ),
                const Spacer(),
                FilledButton(
                  onPressed: busy ? null : save,
                  child: Text(busy ? 'Working…' : 'Save'),
                ),
              ],
            ),
          ],
        ),
      ),
    ),
  );
}
