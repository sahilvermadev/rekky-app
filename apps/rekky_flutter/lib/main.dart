import 'dart:convert';
import 'dart:math';

import 'package:flutter/material.dart';

import 'identity.dart';
import 'rekky_api.dart';

const apiBaseUrl = String.fromEnvironment('API_BASE_URL');
void main() => runApp(const RekkyApp());

class RekkyApp extends StatelessWidget {
  const RekkyApp({super.key});
  @override
  Widget build(BuildContext context) => MaterialApp(
    title: 'Rekky',
    theme: ThemeData(
      useMaterial3: true,
      colorScheme: ColorScheme.fromSeed(
        seedColor: const Color(0xFF74452F),
        surface: const Color(0xFFFFFBF5),
      ),
      scaffoldBackgroundColor: const Color(0xFFFFFBF5),
    ),
    home: const RekkyHome(),
  );
}

class RekkyHome extends StatefulWidget {
  const RekkyHome({super.key});
  @override
  State<RekkyHome> createState() => _RekkyHomeState();
}

class _RekkyHomeState extends State<RekkyHome> {
  final api = RekkyApi(apiBaseUrl);
  final identity = IdentityService();
  final question = TextEditingController();
  bool busy = true, signedIn = false, disclosed = false, searching = false;
  int destination = 0;
  String? issue;
  List<RekkyItem> library = [];
  List<Map<String, dynamic>> matches = [];

  @override
  void initState() {
    super.initState();
    _restore();
  }

  @override
  void dispose() {
    question.dispose();
    super.dispose();
  }

  Future<void> _restore() async {
    try {
      api.token = await identity.storedToken();
      if (api.token != null) {
        final me = await api.me();
        signedIn = true;
        disclosed =
            (me['account']
                    as Map<String, dynamic>)['visibility_disclosure_accepted']
                as bool;
        if (disclosed) library = await api.items();
      }
    } on ApiFailure catch (failure) {
      if (failure.status == 401) {
        await identity.clearToken();
        api.token = null;
      } else {
        issue = failure.message;
      }
    } catch (error) {
      issue = '$error';
    }
    if (mounted) setState(() => busy = false);
  }

  Future<void> _signIn(String provider) async {
    setState(() {
      busy = true;
      issue = null;
    });
    try {
      final idToken = provider == 'google'
          ? await identity.googleIdToken()
          : await identity.appleIdToken();
      final response = await api.exchange(provider, idToken);
      api.token =
          (response['session'] as Map<String, dynamic>)['token'] as String;
      await identity.storeToken(api.token!);
      final me = await api.me();
      signedIn = true;
      disclosed =
          (me['account']
                  as Map<String, dynamic>)['visibility_disclosure_accepted']
              as bool;
      if (disclosed) library = await api.items();
    } catch (error) {
      issue = '$error';
    }
    if (mounted) setState(() => busy = false);
  }

  Future<void> _accept() async {
    setState(() {
      busy = true;
      issue = null;
    });
    try {
      await api.acceptDisclosure();
      disclosed = true;
      library = await api.items();
    } catch (error) {
      issue = '$error';
    }
    if (mounted) setState(() => busy = false);
  }

  Future<void> _signOut() async {
    setState(() => busy = true);
    var remoteRevoked = true;
    try {
      await api.signOut();
    } catch (_) {
      remoteRevoked = false;
    }
    await identity.clearToken();
    api.token = null;
    if (mounted) {
      setState(() {
        signedIn = false;
        disclosed = false;
        library = [];
        matches = [];
        busy = false;
        issue = remoteRevoked ? null : 'Signed out on this device. Server revocation could not be confirmed.';
      });
    }
  }

  Future<void> _reload() async {
    try {
      final items = await api.items();
      if (mounted) {
        setState(() {
          library = items;
          issue = null;
        });
      }
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    }
  }

  Future<void> _ask() async {
    final q = question.text.trim();
    if (q.isEmpty) return;
    setState(() {
      searching = true;
      issue = null;
    });
    try {
      final found = await api.ask(q);
      if (mounted) setState(() => matches = found);
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    }
    if (mounted) setState(() => searching = false);
  }

  String _newKey() => base64UrlEncode(
    List<int>.generate(24, (_) => Random.secure().nextInt(256)),
  ).replaceAll('=', '');

  Future<void> _remember() async {
    final subject = TextEditingController(), body = TextEditingController();
    var visibility = 'friends', saving = false;
    String? formIssue;
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (sheetContext) => StatefulBuilder(
        builder: (context, update) => Padding(
          padding: EdgeInsets.fromLTRB(
            24,
            8,
            24,
            MediaQuery.viewInsetsOf(context).bottom + 24,
          ),
          child: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  'Keep something useful',
                  style: Theme.of(context).textTheme.headlineSmall,
                ),
                const SizedBox(height: 8),
                const Text(
                  'Save one thought for now. Voice and automatic organization are coming next.',
                ),
                const SizedBox(height: 20),
                TextField(
                  controller: subject,
                  maxLength: 120,
                  decoration: const InputDecoration(
                    labelText: 'Who or what is this about?',
                    border: OutlineInputBorder(),
                  ),
                ),
                TextField(
                  controller: body,
                  minLines: 3,
                  maxLines: 7,
                  maxLength: 20000,
                  decoration: const InputDecoration(
                    labelText: 'What should you remember?',
                    border: OutlineInputBorder(),
                  ),
                ),
                SwitchListTile(
                  contentPadding: EdgeInsets.zero,
                  title: Text(visibility == 'private' ? 'Only me' : 'Friends'),
                  subtitle: const Text(
                    'Friends items will be visible to accepted friends when sharing launches.',
                  ),
                  value: visibility == 'private',
                  onChanged: (value) =>
                      update(() => visibility = value ? 'private' : 'friends'),
                ),
                if (formIssue != null)
                  Text(
                    formIssue!,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                FilledButton(
                  onPressed: saving
                      ? null
                      : () async {
                          if (subject.text.trim().isEmpty ||
                              body.text.trim().isEmpty) {
                            update(
                              () => formIssue = 'Add a subject and a thought.',
                            );
                            return;
                          }
                          update(() {
                            saving = true;
                            formIssue = null;
                          });
                          try {
                            await api.save(
                              subject.text.trim(),
                              body.text.trim(),
                              visibility,
                              _newKey(),
                            );
                            if (sheetContext.mounted) {
                              Navigator.pop(sheetContext);
                            }
                            await _reload();
                          } catch (error) {
                            update(() {
                              saving = false;
                              formIssue = '$error';
                            });
                          }
                        },
                  child: Text(saving ? 'Saving…' : 'Save memory'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
    subject.dispose();
    body.dispose();
  }

  Future<void> _openItem(RekkyItem item) async {
    RekkySource? source;
    try {
      source = await api.source(item);
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
      return;
    }
    if (!mounted) return;
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (sheetContext) => Padding(
        padding: const EdgeInsets.fromLTRB(24, 8, 24, 32),
        child: SafeArea(
          top: false,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                item.subject,
                style: Theme.of(sheetContext).textTheme.headlineSmall,
              ),
              const SizedBox(height: 12),
              Text(item.body),
              const SizedBox(height: 16),
              Text(
                'Source text · only you can see this',
                style: Theme.of(sheetContext).textTheme.labelLarge,
              ),
              const SizedBox(height: 4),
              Text(source?.text ?? 'Source removed'),
              const SizedBox(height: 16),
              Text(
                'Audience: ${item.visibility == 'private' ? 'Only me' : 'Friends'}',
              ),
              const SizedBox(height: 12),
              Wrap(
                spacing: 8,
                children: [
                  if (source != null)
                    TextButton(
                      onPressed: () async {
                        final confirmed = await showDialog<bool>(
                          context: sheetContext,
                          builder: (dialogContext) => AlertDialog(
                            title: const Text('Delete private source?'),
                            content: const Text(
                              'Your saved memory stays available, but its original source text will be removed.',
                            ),
                            actions: [
                              TextButton(
                                onPressed: () =>
                                    Navigator.pop(dialogContext, false),
                                child: const Text('Cancel'),
                              ),
                              FilledButton(
                                onPressed: () =>
                                    Navigator.pop(dialogContext, true),
                                child: const Text('Delete source'),
                              ),
                            ],
                          ),
                        );
                        if (confirmed != true) return;
                        try {
                          await api.deleteSource(item, source!);
                          if (sheetContext.mounted) Navigator.pop(sheetContext);
                        } catch (error) {
                          if (mounted) setState(() => issue = '$error');
                          if (sheetContext.mounted) Navigator.pop(sheetContext);
                        }
                      },
                      child: const Text('Delete source'),
                    ),
                  OutlinedButton(
                    onPressed: () async {
                      try {
                        await api.changeVisibility(
                          item,
                          item.visibility == 'private' ? 'friends' : 'private',
                        );
                        if (sheetContext.mounted) Navigator.pop(sheetContext);
                        await _reload();
                      } catch (error) {
                        if (mounted) setState(() => issue = '$error');
                        if (sheetContext.mounted) Navigator.pop(sheetContext);
                      }
                    },
                    child: Text(
                      item.visibility == 'private'
                          ? 'Share with friends'
                          : 'Make private',
                    ),
                  ),
                  TextButton(
                    onPressed: () async {
                      final confirmed = await showDialog<bool>(
                        context: sheetContext,
                        builder: (dialogContext) => AlertDialog(
                          title: const Text('Delete this memory?'),
                          content: const Text(
                            'This also deletes its private source text.',
                          ),
                          actions: [
                            TextButton(
                              onPressed: () =>
                                  Navigator.pop(dialogContext, false),
                              child: const Text('Cancel'),
                            ),
                            FilledButton(
                              onPressed: () =>
                                  Navigator.pop(dialogContext, true),
                              child: const Text('Delete'),
                            ),
                          ],
                        ),
                      );
                      if (confirmed != true) return;
                      try {
                        await api.delete(item);
                        if (sheetContext.mounted) Navigator.pop(sheetContext);
                        await _reload();
                      } catch (error) {
                        if (mounted) setState(() => issue = '$error');
                        if (sheetContext.mounted) Navigator.pop(sheetContext);
                      }
                    },
                    child: const Text('Delete'),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    if (busy && !signedIn) {
      return const Scaffold(body: Center(child: CircularProgressIndicator()));
    }
    if (!signedIn) {
      return Scaffold(
        body: SafeArea(
          child: Padding(
            padding: const EdgeInsets.all(28),
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  'Rekky',
                  style: Theme.of(context).textTheme.displayLarge
                      ?.copyWith(fontWeight: FontWeight.w700),
                ),
                const SizedBox(height: 12),
                Text(
                  'The good things you and your friends remember.',
                  style: Theme.of(context).textTheme.headlineSmall,
                ),
                const SizedBox(height: 40),
                FilledButton(
                  onPressed: busy ? null : () => _signIn('google'),
                  child: const Text('Continue with Google'),
                ),
                if (identity.supportsApple)
                  OutlinedButton(
                    onPressed: busy ? null : () => _signIn('apple'),
                    child: const Text('Continue with Apple'),
                  ),
                if (issue != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 16),
                    child: Text(
                      issue!,
                      style: TextStyle(
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      );
    }
    if (!disclosed) {
      return Scaffold(
        appBar: AppBar(
          title: const Text('Before you begin'),
          actions: [
            TextButton(onPressed: _signOut, child: const Text('Sign out')),
          ],
        ),
        body: SafeArea(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  'Useful knowledge is better together.',
                  style: Theme.of(context).textTheme.headlineMedium,
                ),
                const SizedBox(height: 20),
                const Text(
                  'Completed memories are set to Friends by default. Accepted friends may see them, including older Friends memories, when friend sharing launches. You can set individual memories to Only me.',
                ),
                const SizedBox(height: 12),
                const Text(
                  'Your source text and future voice transcripts stay private. Voice audio will be temporary and deleted after accepted transcription.',
                ),
                const Spacer(),
                if (issue != null)
                  Text(
                    issue!,
                    style: TextStyle(
                      color: Theme.of(context).colorScheme.error,
                    ),
                  ),
                FilledButton(
                  onPressed: busy ? null : _accept,
                  child: const Text('Continue'),
                ),
              ],
            ),
          ),
        ),
      );
    }
    return Scaffold(
      appBar: AppBar(
        title: const Text('Rekky'),
        actions: [
          IconButton(
            tooltip: 'Sign out',
            onPressed: _signOut,
            icon: const Icon(Icons.logout),
          ),
        ],
      ),
      body: SafeArea(
        child: Column(
          children: [
            if (issue != null)
              MaterialBanner(
                content: Text(issue!),
                actions: [
                  TextButton(
                    onPressed: () => setState(() => issue = null),
                    child: const Text('Dismiss'),
                  ),
                ],
              ),
            Expanded(child: destination == 0 ? _askPage() : _libraryPage()),
          ],
        ),
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _remember,
        icon: const Icon(Icons.add),
        label: const Text('Remember'),
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: destination,
        onDestinationSelected: (value) => setState(() => destination = value),
        destinations: const [
          NavigationDestination(icon: Icon(Icons.search), label: 'Ask'),
          NavigationDestination(
            icon: Icon(Icons.bookmarks_outlined),
            label: 'Library',
          ),
        ],
      ),
    );
  }

  Widget _askPage() => Padding(
    padding: const EdgeInsets.all(20),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'What are you trying to remember?',
          style: Theme.of(context).textTheme.headlineSmall,
        ),
        const SizedBox(height: 16),
        TextField(
          controller: question,
          textInputAction: TextInputAction.search,
          onSubmitted: (_) => _ask(),
          decoration: InputDecoration(
            hintText: 'Who fixed our kitchen tap?',
            suffixIcon: IconButton(
              tooltip: 'Search',
              onPressed: _ask,
              icon: const Icon(Icons.arrow_forward),
            ),
            border: const OutlineInputBorder(),
          ),
        ),
        const SizedBox(height: 8),
        const Text(
          'Searching your own saved memories. Friend answers come later.',
        ),
        const SizedBox(height: 20),
        if (searching) const LinearProgressIndicator(),
        Expanded(
          child: matches.isEmpty
              ? Center(
                  child: Text(
                    question.text.isEmpty
                        ? 'Ask about something you saved.'
                        : 'No matching memory yet.',
                  ),
                )
              : ListView.builder(
                  itemCount: matches.length,
                  itemBuilder: (context, index) {
                    final match = matches[index];
                    return Card(
                      child: ListTile(
                        title: Text(match['subject'] as String),
                        subtitle: Text(match['body'] as String),
                        trailing: const Icon(Icons.chevron_right),
                        onTap: () {
                          final item = library
                              .where((item) => item.id == match['item_id'])
                              .firstOrNull;
                          if (item != null) _openItem(item);
                        },
                      ),
                    );
                  },
                ),
        ),
      ],
    ),
  );

  Widget _libraryPage() => RefreshIndicator(
    onRefresh: _reload,
    child: library.isEmpty
        ? ListView(
            children: const [
              SizedBox(height: 160),
              Center(child: Text('Your saved memories will appear here.')),
            ],
          )
        : ListView.builder(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 100),
            itemCount: library.length,
            itemBuilder: (context, index) {
              final item = library[index];
              return Card(
                child: ListTile(
                  title: Text(item.subject),
                  subtitle: Text(
                    item.body,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                  trailing: Icon(
                    item.visibility == 'private'
                        ? Icons.lock_outline
                        : Icons.people_outline,
                  ),
                  onTap: () => _openItem(item),
                ),
              );
            },
          ),
  );
}
