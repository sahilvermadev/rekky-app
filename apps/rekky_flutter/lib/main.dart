import 'dart:async';

import 'package:flutter/material.dart';

import 'identity.dart';
import 'contact_matching.dart';
import 'contact_sheet.dart';
import 'recommendation_editor.dart';
import 'recommendation_view.dart';
import 'recommendation_detail.dart';
import 'rekky_api.dart';
import 'voice_capture_sheet.dart';
import 'voice_drafts.dart';
import 'voice_processing.dart';

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

class _RekkyHomeState extends State<RekkyHome> with WidgetsBindingObserver {
  final api = RekkyApi(apiBaseUrl);
  final identity = IdentityService();
  final voiceStore = VoiceDraftStore();
  final question = TextEditingController();
  bool busy = true, signedIn = false, disclosed = false, searching = false;
  bool recordingScreenOpen = false;
  int destination = 0;
  String? issue;
  List<RekkyItem> library = [];
  List<Map<String, dynamic>> matches = [];
  List<VoiceDraft> voiceDrafts = [];
  String? accountId;
  VoiceProcessingCoordinator? voiceProcessing;
  ContactMatchingCoordinator? contactMatching;
  final contactBook = DeviceContactBook();
  int itemScreensOpen = 0;
  final _contactReaders = <String, ValueNotifier<RekkyItem>>{};
  String? processingMessage;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _restore();
  }

  @override
  void dispose() {
    question.dispose();
    WidgetsBinding.instance.removeObserver(this);
    voiceProcessing?.stop();
    contactMatching?.stop();
    unawaited(voiceStore.dispose());
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      voiceProcessing?.resume();
      if (signedIn && disclosed) unawaited(_reload());
    } else if (state == AppLifecycleState.paused) {
      voiceProcessing?.pause();
    }
  }

  void _startVoiceProcessing() {
    final owner = accountId;
    if (owner == null || !disclosed) return;
    voiceProcessing?.stop();
    _startContactMatching();
    voiceProcessing = VoiceProcessingCoordinator(
      ownerId: owner,
      store: voiceStore,
      api: RekkyApi(apiBaseUrl)..token = api.token,
      onChanged: () async {
        if (mounted && accountId == owner && signedIn) {
          await _reloadVoiceAndLibrary();
        }
      },
      onStatus: (message) {
        if (mounted && accountId == owner) {
          setState(() => processingMessage = message);
        }
      },
    );
    unawaited(voiceProcessing!.process());
  }

  void _startContactMatching() {
    final owner = accountId;
    final token = api.token;
    if (owner == null || token == null || !disclosed) return;
    contactMatching?.stop();
    contactMatching = ContactMatchingCoordinator(
      api: RekkyApi(apiBaseUrl)..token = token,
      book: contactBook,
      isCurrent: () =>
          mounted &&
          signedIn &&
          accountId == owner &&
          api.token == token &&
          itemScreensOpen == 0 &&
          WidgetsBinding.instance.lifecycleState == AppLifecycleState.resumed,
      onSaved: (updated) {
        setState(
          () => library = library
              .map((i) => i.id == updated.id ? updated : i)
              .toList(),
        );
        _contactReaders[updated.id]?.value = updated;
      },
    );
    unawaited(contactMatching!.process(library));
  }

  Future<bool> _contactSettings({bool enableOnly = false}) async {
    final owner = accountId;
    final token = api.token;
    final scoped = RekkyApi(apiBaseUrl)..token = token;
    bool current() =>
        mounted && signedIn && accountId == owner && api.token == token;
    try {
      final settings = await scoped.contactPreference();
      if (!mounted || !current()) return false;
      final enabled = settings['enabled'] == true;
      if (!(enableOnly && enabled)) {
        final accepted = await showDialog<bool>(
          context: context,
          builder: (dialog) => AlertDialog(
            title: Text(
              enabled
                  ? 'Contact matching is on'
                  : 'Find saved contacts automatically?',
            ),
            content: Text(
              enabled
                  ? 'Turning this off stops future matching. Numbers already attached stay with their recommendations; you can remove them in Edit.'
                  : 'Rekky checks names and numbers on this phone for clear matches to people you recommend. Matched numbers are added automatically, including to existing recommendations, and friends can see them when the recommendation is shared. Only attached numbers and their saved contact names are saved to Rekky; your address book stays on this phone.',
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.pop(dialog, false),
                child: const Text('Not now'),
              ),
              FilledButton(
                onPressed: () => Navigator.pop(dialog, true),
                child: Text(enabled ? 'Turn off' : 'Enable matching'),
              ),
            ],
          ),
        );
        if (accepted != true || !current()) return false;
        contactMatching?.stop();
        if (enabled) {
          await scoped.setContactPreference(false);
          if (current()) _startContactMatching();
          return false;
        }
      }
      if (!await contactBook.requestAccess()) {
        if (mounted && current()) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(
              content: Text(
                'Contact access is off. You can still add a number manually, or allow Contacts in phone settings.',
              ),
            ),
          );
        }
        return false;
      }
      if (!mounted || !current()) return false;
      if (!enabled) await scoped.setContactPreference(true);
      if (!mounted || !current()) return false;
      _startContactMatching();
      return true;
    } catch (_) {
      if (mounted && current()) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text(
              'Couldn’t update contact matching. Please try again.',
            ),
          ),
        );
      }
      return false;
    }
  }

  Future<RekkyItem?> _manageContact(RekkyItem item) async {
    final owner = accountId;
    final token = api.token;
    final scoped = RekkyApi(apiBaseUrl)..token = token;
    void check() {
      if (!mounted || !signedIn || accountId != owner || api.token != token) {
        throw StateError('Account changed');
      }
    }

    itemScreensOpen++;
    try {
      final updated = await showModalBottomSheet<RekkyItem>(
        context: context,
        isScrollControlled: true,
        useSafeArea: true,
        showDragHandle: true,
        builder: (_) => ContactSheet(
          item: item,
          book: contactBook,
          enableMatching: () async {
            if (!await _contactSettings(enableOnly: true)) return null;
            check();
            final preference = await scoped.contactPreference();
            check();
            return preference['enabled'] == true
                ? preference['generation'] as int
                : null;
          },
          save: (contact) async {
            check();
            final updated = await scoped.attachContact(item, contact);
            check();
            setState(
              () => library = library
                  .map((i) => i.id == updated.id ? updated : i)
                  .toList(),
            );
            return updated;
          },
        ),
      );
      check();
      return updated;
    } finally {
      itemScreensOpen--;
      if (mounted && accountId == owner) unawaited(_reload());
    }
  }

  Future<void> _restore() async {
    try {
      api.token = await identity.storedToken();
      if (api.token != null) {
        final me = await api.me();
        accountId = (me['account'] as Map<String, dynamic>)['id'] as String;
        signedIn = true;
        disclosed =
            (me['account']
                    as Map<String, dynamic>)['visibility_disclosure_accepted']
                as bool;
        if (disclosed) {
          final voice = await api.voicePermission();
          final extraction = await api.extractionPermission();
          if ((voice['voice_transcription'] as Map)['generation'] == 0 &&
              (extraction['transcript_extraction'] as Map)['generation'] == 0) {
            disclosed = false;
          } else {
            library = await api.items();
          }
        }
        voiceDrafts = await voiceStore.forOwner(accountId!);
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
    if (signedIn && disclosed) _startVoiceProcessing();
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
      accountId = (me['account'] as Map<String, dynamic>)['id'] as String;
      signedIn = true;
      disclosed =
          (me['account']
                  as Map<String, dynamic>)['visibility_disclosure_accepted']
              as bool;
      if (disclosed) {
        final voice = await api.voicePermission();
        final extraction = await api.extractionPermission();
        if ((voice['voice_transcription'] as Map)['generation'] == 0 &&
            (extraction['transcript_extraction'] as Map)['generation'] == 0) {
          disclosed = false;
        } else {
          library = await api.items();
        }
      }
      voiceDrafts = await voiceStore.forOwner(accountId!);
    } catch (error) {
      issue = '$error';
    }
    if (mounted) setState(() => busy = false);
    if (signedIn && disclosed) _startVoiceProcessing();
  }

  Future<void> _accept() async {
    setState(() {
      busy = true;
      issue = null;
    });
    try {
      await api.acceptDisclosure();
      await api.setVoicePermission(true);
      await api.setExtractionPermission(true);
      disclosed = true;
      library = await api.items();
    } catch (error) {
      issue = '$error';
    }
    if (mounted) setState(() => busy = false);
    if (disclosed) _startVoiceProcessing();
  }

  Future<void> _signOut() async {
    contactMatching?.stop();
    contactMatching = null;
    voiceProcessing?.stop();
    voiceProcessing = null;
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
        voiceDrafts = [];
        processingMessage = null;
        accountId = null;
        busy = false;
        issue = remoteRevoked ? null : 'Signed out on this device. Server revocation could not be confirmed.';
      });
    }
  }

  Future<void> _reload() async {
    final owner = accountId;
    if (owner == null || !signedIn || !disclosed) return;
    try {
      final items = await api.items();
      if (mounted && accountId == owner && signedIn) {
        setState(() {
          library = items;
          issue = null;
        });
        unawaited(contactMatching?.process(library) ?? Future<void>.value());
      }
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    }
  }

  Future<void> _reloadVoiceDrafts() async {
    final owner = accountId;
    if (owner == null) return;
    try {
      final drafts = await voiceStore.forOwner(owner);
      if (mounted && accountId == owner) setState(() => voiceDrafts = drafts);
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    }
  }

  Future<void> _reloadVoiceAndLibrary() async {
    await _reloadVoiceDrafts();
    await _reload();
  }

  Future<bool> _recordVoiceDraft() async {
    final owner = accountId;
    if (owner == null) return false;
    return await Navigator.push<bool>(
          context,
          MaterialPageRoute(
            builder: (_) => VoiceCaptureSheet(
              ownerId: owner,
              store: voiceStore,
              onChanged: _reloadVoiceDrafts,
              onCaptured: () =>
                  unawaited(voiceProcessing?.process() ?? Future<void>.value()),
            ),
          ),
        ) ??
        false;
  }

  Future<void> _openVoiceDrafts() async {
    final owner = accountId;
    if (owner == null) return;
    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      isDismissible: false,
      enableDrag: false,
      showDragHandle: true,
      builder: (_) => VoiceDraftsSheet(
        ownerId: owner,
        store: voiceStore,
        api: api,
        onChanged: _reloadVoiceAndLibrary,
        onOpenLibrary: () {
          Navigator.pop(context);
          setState(() => destination = 1);
        },
      ),
    );
  }

  Future<void> _processingSettings() async {
    final owner = accountId;
    if (owner == null) return;
    try {
      final voice = await api.voicePermission();
      final extraction = await api.extractionPermission();
      final enabled =
          (voice['voice_transcription'] as Map)['enabled'] == true &&
          (extraction['transcript_extraction'] as Map)['enabled'] == true;
      if (!mounted) return;
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (dialogContext) => AlertDialog(
          title: Text(
            enabled ? 'Voice processing is on' : 'Turn on voice processing?',
          ),
          content: Text(
            enabled
                ? 'Turning this off stops new voice and transcript processing. Unprocessed recordings on this phone will be deleted; saved memories and transcripts remain.'
                : 'Rekky will send new recordings and their private transcript text to OpenAI to make recommendations. Audio is deleted after a usable transcript is saved. New completed recommendations default to Friends; you can change them to Only me. Incomplete results and transcripts stay private. Existing recommendations keep their audience.',
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(dialogContext, false),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () => Navigator.pop(dialogContext, true),
              child: Text(enabled ? 'Turn off' : 'Turn on'),
            ),
          ],
        ),
      );
      if (confirmed != true) return;
      if (enabled) {
        voiceProcessing?.stop();
        await api.setVoicePermission(false);
        await api.setExtractionPermission(false);
        await voiceStore.deleteAll(owner);
        await voiceStore.clearPendingRemembers(owner);
        if (mounted) setState(() => processingMessage = null);
      } else {
        await api.setVoicePermission(true);
        await api.setExtractionPermission(true);
        unawaited(voiceProcessing?.process() ?? Future<void>.value());
      }
      _startVoiceProcessing();
      await _reloadVoiceDrafts();
    } catch (error) {
      _startVoiceProcessing();
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

  Future<void> _remember() async {
    if (recordingScreenOpen) return;
    recordingScreenOpen = true;
    try {
      final saved = await _recordVoiceDraft();
      if (saved && mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Got it. We’ll add it to your Library.'),
          ),
        );
      }
    } catch (error) {
      if (mounted) setState(() => issue = '$error');
    } finally {
      recordingScreenOpen = false;
    }
  }

  Future<void> _refineItem(RekkyItem item) async {
    final owner = accountId;
    final scopedApi = RekkyApi(apiBaseUrl)..token = api.token;
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(content: Text('Updating your recommendation…')),
    );
    try {
      await scopedApi.refineItem(item);
      if (!mounted || accountId != owner || !signedIn) return;
      await _reload();
      if (!mounted || accountId != owner || !signedIn) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Recommendation updated.')));
    } catch (error) {
      if (mounted && accountId == owner) setState(() => issue = '$error');
    }
  }

  Future<void> _openItem(RekkyItem item) async {
    final owner = accountId;
    final scopedApi = RekkyApi(apiBaseUrl)..token = api.token;
    void checkAccount() {
      if (!mounted || !signedIn || accountId != owner) {
        throw StateError('Account changed');
      }
    }

    checkAccount();
    final contactUpdates = ValueNotifier(item);
    _contactReaders[item.id] = contactUpdates;
    try {
      await showModalBottomSheet<void>(
        context: context,
        isScrollControlled: true,
        useSafeArea: true,
        showDragHandle: true,
        backgroundColor: Theme.of(context).colorScheme.surface,
        builder: (sheetContext) => ConstrainedBox(
          constraints: BoxConstraints(
            maxHeight: MediaQuery.sizeOf(sheetContext).height * .9,
          ),
          child: RecommendationDetailSheet(
            item: item,
            contactUpdates: contactUpdates,
            manageContact: _manageContact,
            loadSource: () async {
              checkAccount();
              final source = await scopedApi.source(item);
              checkAccount();
              return source;
            },
            loadPlace: () async {
              checkAccount();
              final place = await scopedApi.place(item);
              checkAccount();
              return place;
            },
            changeAudience: (current, visibility) async {
              checkAccount();
              final updated = await scopedApi.changeVisibility(
                current,
                visibility,
              );
              checkAccount();
              setState(() {
                library = library
                    .map((i) => i.id == updated.id ? updated : i)
                    .toList();
              });
              return updated;
            },
            deleteItem: (current) async {
              checkAccount();
              await scopedApi.delete(current);
              checkAccount();
              setState(() {
                library.removeWhere((i) => i.id == current.id);
                matches.removeWhere((m) => m['item_id'] == current.id);
              });
            },
            editRecommendation: (current) {
              if (mounted && signedIn && accountId == owner) {
                unawaited(_editRecommendation(current));
              }
            },
            refineItem: (current) {
              if (mounted && signedIn && accountId == owner) {
                unawaited(_refineItem(current));
              }
            },
          ),
        ),
      );
    } finally {
      _contactReaders.remove(item.id);
      contactUpdates.dispose();
      if (mounted && signedIn && accountId == owner) unawaited(_reload());
    }
  }

  Future<void> _editRecommendation(RekkyItem item) async {
    final owner = accountId;
    final scopedApi = RekkyApi(apiBaseUrl)..token = api.token;
    void checkAccount() {
      if (!mounted || !signedIn || accountId != owner) {
        throw StateError('Account changed');
      }
    }

    itemScreensOpen++;
    RekkyItem? updated;
    try {
      updated = await Navigator.push<RekkyItem>(
        context,
        MaterialPageRoute(
          builder: (_) => RecommendationEditor(
            item: item,
            loadConcepts: () async {
              checkAccount();
              final concepts = await scopedApi.categoryConcepts();
              checkAccount();
              return concepts;
            },
            onSave: (content) async {
              checkAccount();
              final updated = await scopedApi.editRecommendation(item, content);
              checkAccount();
              setState(() {
                library = library
                    .map((i) => i.id == updated.id ? updated : i)
                    .toList();
                matches = matches
                    .map(
                      (m) => m['item_id'] == updated.id
                          ? {
                              ...m,
                              'subject': updated.subject,
                              'body': updated.body,
                              'visibility': updated.visibility,
                              'revision': updated.revision,
                            }
                          : m,
                    )
                    .toList();
              });
              return updated;
            },
          ),
        ),
      );
    } finally {
      itemScreensOpen--;
    }
    if (updated != null && mounted && signedIn && accountId == owner) {
      unawaited(_openItem(updated));
    }
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
                  'When you use Remember, Rekky automatically sends the selected audio and its private transcript text to OpenAI to create your recommendation. Audio is deleted after a usable transcript is saved. The transcript stays private. You can turn future processing off in settings.',
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
          PopupMenuButton<String>(
            tooltip: 'Account and recovery',
            onSelected: (value) {
              if (value == 'contacts') unawaited(_contactSettings());
              if (value == 'recordings') unawaited(_openVoiceDrafts());
              if (value == 'processing') unawaited(_processingSettings());
              if (value == 'signout') unawaited(_signOut());
            },
            itemBuilder: (_) => const [
              PopupMenuItem(value: 'contacts', child: Text('Contact matching')),
              PopupMenuItem(
                value: 'processing',
                child: Text('Voice processing'),
              ),
              PopupMenuItem(
                value: 'recordings',
                child: Text('Pending recordings'),
              ),
              PopupMenuItem(value: 'signout', child: Text('Sign out')),
            ],
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
      bottomNavigationBar: SafeArea(
        top: false,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 8, 12, 10),
          child: Row(
            children: [
              Expanded(
                child: Semantics(
                  selected: destination == 0,
                  child: TextButton(
                    style: TextButton.styleFrom(
                      backgroundColor: destination == 0
                          ? Theme.of(context).colorScheme.secondaryContainer
                          : null,
                    ),
                    onPressed: () => setState(() => destination = 0),
                    child: const Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [Icon(Icons.search), Text('Ask')],
                    ),
                  ),
                ),
              ),
              Expanded(
                flex: 2,
                child: FilledButton(
                  onPressed: _remember,
                  child: const Padding(
                    padding: EdgeInsets.symmetric(vertical: 10),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.mic),
                        Text('Remember', textAlign: TextAlign.center),
                      ],
                    ),
                  ),
                ),
              ),
              Expanded(
                child: Semantics(
                  selected: destination == 1,
                  child: TextButton(
                    style: TextButton.styleFrom(
                      backgroundColor: destination == 1
                          ? Theme.of(context).colorScheme.secondaryContainer
                          : null,
                    ),
                    onPressed: () => setState(() => destination = 1),
                    child: const Column(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Icon(Icons.bookmarks_outlined),
                        Text('Library'),
                      ],
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
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
            children: [
              if (processingMessage != null)
                ListTile(title: Text(processingMessage!)),
              const SizedBox(height: 160),
              const Center(
                child: Text('Your saved memories will appear here.'),
              ),
            ],
          )
        : ListView.builder(
            padding: const EdgeInsets.fromLTRB(16, 8, 16, 100),
            itemCount: library.length + (processingMessage == null ? 0 : 1),
            itemBuilder: (context, index) {
              if (processingMessage != null && index == 0) {
                return ListTile(title: Text(processingMessage!));
              }
              if (processingMessage != null) index -= 1;
              final item = library[index];
              return RecommendationCard(
                key: ValueKey(item.id),
                item: item,
                onTap: () => _openItem(item),
              );
            },
          ),
  );
}
