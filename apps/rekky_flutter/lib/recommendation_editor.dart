import 'dart:convert';

import 'package:flutter/material.dart';

import 'category_editor.dart';
import 'rekky_api.dart';

// Hallmark · component: recommendation editor · existing warm Material tokens.
// Pre-emit critique: P4 H5 E4 S4 R5 V4. One staged save, native focus states.
class RecommendationEditor extends StatefulWidget {
  const RecommendationEditor({
    super.key,
    required this.item,
    required this.loadConcepts,
    required this.onSave,
  });
  final RekkyItem item;
  final Future<List<CategoryConcept>> Function() loadConcepts;
  final Future<RekkyItem> Function(Map<String, dynamic>) onSave;
  @override
  State<RecommendationEditor> createState() => _RecommendationEditorState();
}

class _Line {
  _Line(this.kind, this.text);
  final key = UniqueKey();
  String kind, text;
}

class _RecommendationEditorState extends State<RecommendationEditor> {
  final form = GlobalKey<FormState>();
  late String subject, summary, kind, experience, attribution, visibility;
  late String linkMode, linkUrl, linkLabel, baseline;
  late List<String> types, facets;
  late List<_Line> observations, locations, useCases, descriptors;
  List<CategoryConcept>? concepts;
  String? error, categoryError;
  bool saving = false,
      leaving = false,
      confirmingExit = false,
      linkConfirmed = false;
  bool categoriesLoading = false;
  String ratingChoice = 'keep';

  static const kinds = {
    'place': 'Place',
    'person_service': 'Person or service',
    'thing': 'Thing',
    'activity_event': 'Activity or event',
    'idea_tip': 'Idea or tip',
  };
  static const experiences = {
    'firsthand': 'My experience',
    'secondhand': 'Heard from someone',
    'interest': 'Not tried yet',
    'unspecified': 'Not specified',
  };
  static const detailKinds = {
    'context': 'Detail',
    'suggestion': 'Suggestion',
    'praise': 'Highlight',
    'caution': 'Caution',
    'suitability': 'Good for',
    'price': 'Price',
  };
  static const locationRoles = {
    'venue': 'Venue',
    'practice': 'Practice location',
    'service_area': 'Service area',
    'past_experience': 'Past experience',
    'context': 'Other location',
  };

  @override
  void initState() {
    super.initState();
    final r = widget.item.recommendation;
    subject = widget.item.subject;
    summary = r?.summary ?? widget.item.body;
    kind = r?.entityKind ?? 'idea_tip';
    if (!kinds.containsKey(kind)) kind = 'idea_tip';
    experience = r?.experience ?? 'unspecified';
    attribution = r?.attribution ?? '';
    visibility = widget.item.visibility;
    linkMode = r?.destinationMode ?? 'auto';
    linkUrl = r?.destinationUrl ?? '';
    linkLabel = r?.destinationLabel ?? '';
    types = r?.classification?.types.map((c) => c.id).toList() ?? [];
    facets = r?.classification?.facets.map((c) => c.id).toList() ?? [];
    observations =
        r?.observations.map((o) => _Line(o.kind, o.text)).toList() ?? [];
    locations = r?.locations.map((l) => _Line(l.kind, l.text)).toList() ?? [];
    useCases = r?.useCases.map((s) => _Line('', s)).toList() ?? [];
    descriptors =
        r?.classification?.descriptors.map((s) => _Line('', s)).toList() ?? [];
    baseline = jsonEncode(payload());
  }

  Map<String, dynamic> payload() => {
    'subject': subject,
    'summary': summary,
    'visibility': visibility,
    'entity_kind': kind,
    'experience': experience,
    'attribution': attribution,
    'types': types,
    'facets': facets,
    'observations': observations
        .map((o) => {'kind': o.kind, 'text': o.text})
        .toList(),
    'locations': locations
        .map((l) => {'role': l.kind, 'text': l.text})
        .toList(),
    'use_cases': useCases.map((l) => l.text).toList(),
    'descriptors': descriptors.map((l) => l.text).toList(),
    'destination': {
      'mode': linkMode,
      'url': linkMode == 'custom' ? linkUrl : '',
      'label': linkMode == 'custom' ? linkLabel : '',
    },
    'destination_confirmed': linkConfirmed,
    'rating': ratingChoice == 'keep' || ratingChoice == 'none'
        ? {'mode': ratingChoice}
        : {'mode': 'set', 'value': double.parse(ratingChoice)},
  };
  bool get dirty => baseline != jsonEncode(payload());
  bool get ratingWillClear {
    final r = widget.item.recommendation;
    if (ratingChoice != 'keep' || r?.rating == null) return false;
    return subject.trim() != widget.item.subject ||
        kind != r!.entityKind ||
        experience != r.experience ||
        (r.rating!.origin != 'user' &&
            (summary.trim() != r.summary ||
                attribution.trim() != r.attribution ||
                jsonEncode(
                      observations.map((o) => [o.kind, o.text.trim()]).toList(),
                    ) !=
                    jsonEncode(
                      r.observations.map((o) => [o.kind, o.text]).toList(),
                    )));
  }

  Map<String, String> get ratingOptions {
    final rating = widget.item.recommendation?.rating;
    return {
      'keep': rating == null
          ? 'No rating'
          : ratingWillClear
          ? 'Clear previous rating'
          : 'Keep ${rating.label}/10${rating.estimated ? ' · estimated' : ''}',
      if (rating != null) 'none': 'Remove rating',
      for (var half = 0; half <= 20; half++)
        (half / 2).toString(): '${half.isEven ? half ~/ 2 : half / 2}/10',
    };
  }

  bool get linkNeedsCheck {
    final r = widget.item.recommendation;
    return linkMode == 'custom' &&
        r?.destinationMode == 'custom' &&
        linkUrl == r?.destinationUrl &&
        (subject.trim() != widget.item.subject ||
            kind != r?.entityKind ||
            jsonEncode(
                  locations.map((l) => [l.kind, l.text.trim()]).toList(),
                ) !=
                jsonEncode(r?.locations.map((l) => [l.kind, l.text]).toList()));
  }

  Future<void> close() async {
    if (saving || confirmingExit) return;
    confirmingExit = true;
    final discard =
        !dirty ||
        await showDialog<bool>(
              context: context,
              builder: (context) => AlertDialog(
                title: const Text('Discard changes?'),
                content: const Text('Your changes haven’t been saved.'),
                actions: [
                  TextButton(
                    onPressed: () => Navigator.pop(context, false),
                    child: const Text('Keep editing'),
                  ),
                  TextButton(
                    onPressed: () => Navigator.pop(context, true),
                    child: const Text('Discard'),
                  ),
                ],
              ),
            ) ==
            true;
    confirmingExit = false;
    if (discard && mounted) {
      setState(() => leaving = true);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) Navigator.pop(context);
      });
    }
  }

  Future<void> save() async {
    if (saving || !dirty) return;
    if (!form.currentState!.validate()) {
      setState(() => error = 'Check the highlighted fields before saving.');
      return;
    }
    if (linkNeedsCheck && !linkConfirmed) {
      setState(() => error = 'Check the existing link below, or remove it.');
      return;
    }
    setState(() {
      saving = true;
      error = null;
    });
    try {
      final saved = await widget.onSave(payload());
      if (!mounted) return;
      setState(() => leaving = true);
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) Navigator.pop(context, saved);
      });
    } catch (e) {
      if (mounted) {
        setState(() {
          error = e is ApiFailure
              ? e.message
              : 'Couldn’t save. Your changes are still here. Try again.';
          saving = false;
        });
      }
    }
  }

  Future<void> chooseCategory() async {
    if (categoriesLoading) return;
    if (concepts == null) {
      setState(() {
        categoriesLoading = true;
        categoryError = null;
      });
      try {
        final loaded = await widget.loadConcepts();
        if (!mounted) return;
        setState(() => concepts = loaded);
      } catch (_) {
        if (mounted) {
          setState(
            () => categoryError = 'Couldn’t load categories. Tap to retry.',
          );
        }
        return;
      } finally {
        if (mounted) setState(() => categoriesLoading = false);
      }
    }
    if (!mounted) return;
    final draft = RekkyItem(
      id: widget.item.id,
      captureId: widget.item.captureId,
      subject: subject,
      body: summary,
      visibility: visibility,
      revision: widget.item.revision,
      createdAt: widget.item.createdAt,
      recommendation: RekkyRecommendation(
        summary: summary,
        shelf: kinds[kind]!,
        experience: experience,
        entityKind: kind,
        observations: [],
        locations: [],
        useCases: [],
        classification: RekkyClassification(
          types: types
              .map((id) => concepts!.firstWhere((c) => c.id == id))
              .toList(),
          facets: facets
              .map((id) => concepts!.firstWhere((c) => c.id == id))
              .toList(),
          descriptors: [],
        ),
      ),
    );
    await showModalBottomSheet<bool>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (context) => ConstrainedBox(
        constraints: BoxConstraints(
          maxHeight: MediaQuery.sizeOf(context).height * .8,
        ),
        child: CategoryEditor(
          item: draft,
          concepts: concepts!,
          saveLabel: 'Apply',
          onSave: (t, f) async {
            setState(() {
              types = t;
              facets = f;
            });
          },
        ),
      ),
    );
  }

  String get categoryLabel {
    final known = [
      ...?concepts,
      ...?widget.item.recommendation?.classification?.types,
      ...?widget.item.recommendation?.classification?.facets,
    ];
    final names = [
      ...types,
      ...facets,
    ].map((id) => known.where((c) => c.id == id).firstOrNull?.label ?? id);
    return names.isEmpty ? 'Choose a category' : names.join(' · ');
  }

  Widget textField(
    String label,
    String value,
    ValueChanged<String> change, {
    int max = 2000,
    bool optional = false,
    bool multiline = false,
  }) => Padding(
    padding: const EdgeInsets.only(bottom: 16),
    child: TextFormField(
      initialValue: value,
      enabled: !saving,
      decoration: InputDecoration(
        labelText: label,
        alignLabelWithHint: multiline,
        border: const OutlineInputBorder(),
        counterText: '',
      ),
      minLines: multiline ? 3 : 1,
      maxLines: multiline ? null : 1,
      maxLength: max,
      keyboardType: multiline ? TextInputType.multiline : TextInputType.text,
      textCapitalization: TextCapitalization.sentences,
      validator: (value) =>
          !optional && (value?.trim().isEmpty ?? true) ? 'Enter $label' : null,
      onChanged: (value) => setState(() {
        change(value);
        linkConfirmed = false;
      }),
    ),
  );
  Widget choice(
    String label,
    String value,
    Map<String, String> values,
    ValueChanged<String> change,
  ) => Padding(
    padding: const EdgeInsets.only(bottom: 16),
    child: DropdownButtonFormField<String>(
      key: ValueKey('$label:$value'),
      initialValue: value,
      isExpanded: true,
      decoration: InputDecoration(
        labelText: label,
        border: const OutlineInputBorder(),
      ),
      items: values.entries
          .map(
            (e) => DropdownMenuItem(
              value: e.key,
              child: Text(
                e.value,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
              ),
            ),
          )
          .toList(),
      onChanged: saving
          ? null
          : (value) => setState(() {
              change(value!);
              linkConfirmed = false;
            }),
    ),
  );
  Widget section(String title, List<Widget> children) => Padding(
    padding: const EdgeInsets.only(top: 12, bottom: 12),
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(title, style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 16),
        ...children,
      ],
    ),
  );
  Widget lines(
    List<_Line> values,
    String label,
    int limit, {
    Map<String, String>? roles,
    String defaultRole = '',
    int max = 2000,
  }) => Column(
    crossAxisAlignment: CrossAxisAlignment.start,
    children: [
      for (final line in values)
        Padding(
          key: line.key,
          padding: const EdgeInsets.only(bottom: 12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Expanded(
                    child: Text(
                      '$label ${values.indexOf(line) + 1}',
                      style: Theme.of(context).textTheme.labelLarge,
                    ),
                  ),
                  IconButton(
                    tooltip: 'Remove $label ${values.indexOf(line) + 1}',
                    onPressed: saving
                        ? null
                        : () => setState(() {
                            values.remove(line);
                            linkConfirmed = false;
                          }),
                    icon: const Icon(Icons.close, size: 20),
                  ),
                ],
              ),
              if (roles != null)
                choice('$label type', line.kind, roles, (v) => line.kind = v),
              textField(
                label,
                line.text,
                (v) => line.text = v,
                max: max,
                multiline: true,
              ),
            ],
          ),
        ),
      if (values.length < limit)
        TextButton.icon(
          onPressed: saving
              ? null
              : () => setState(() {
                  values.add(_Line(defaultRole, ''));
                }),
          icon: const Icon(Icons.add),
          label: Text('Add ${label.toLowerCase()}'),
        ),
    ],
  );

  @override
  Widget build(BuildContext context) => PopScope<RekkyItem>(
    canPop: leaving || (!dirty && !saving),
    onPopInvokedWithResult: (didPop, _) {
      if (!didPop) close();
    },
    child: Scaffold(
      appBar: AppBar(
        automaticallyImplyLeading: false,
        leading: IconButton(
          tooltip: 'Close editor',
          onPressed: saving ? null : close,
          icon: const Icon(Icons.close),
        ),
        title: const Text(
          'Edit recommendation',
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
        actions: [
          TextButton(
            onPressed: saving || !dirty ? null : save,
            child: Text(saving ? 'Saving…' : 'Save'),
          ),
        ],
      ),
      body: SafeArea(
        child: Column(
          children: [
            if (error != null)
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 8, 20, 8),
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
            if (saving)
              const LinearProgressIndicator(
                semanticsLabel: 'Saving recommendation',
              ),
            Expanded(
              child: Form(
                key: form,
                child: SingleChildScrollView(
                  padding: const EdgeInsets.all(20),
                  keyboardDismissBehavior:
                      ScrollViewKeyboardDismissBehavior.onDrag,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      textField('Name', subject, (v) => subject = v, max: 120),
                      textField(
                        'Recommendation',
                        summary,
                        (v) => summary = v,
                        max: 20000,
                        multiline: true,
                      ),
                      choice('Who can see this', visibility, const {
                        'private': 'Only me',
                        'friends': 'Friends',
                      }, (v) => visibility = v),
                      choice(
                        'Rating',
                        ratingChoice,
                        ratingOptions,
                        (v) => ratingChoice = v,
                      ),
                      if (ratingWillClear)
                        const Padding(
                          padding: EdgeInsets.only(bottom: 16),
                          child: Text(
                            'This edit changes the experience. Choose a score above to keep a rating.',
                          ),
                        ),
                      section('Experience & source', [
                        choice(
                          'Experience',
                          experience,
                          experiences,
                          (v) => experience = v,
                        ),
                        textField(
                          'Who told you? (optional)',
                          attribution,
                          (v) => attribution = v,
                          max: 300,
                          optional: true,
                        ),
                      ]),
                      section('Category', [
                        choice('Kind of recommendation', kind, kinds, (v) {
                          if (v != kind) {
                            kind = v;
                            types = [];
                            facets = [];
                          }
                        }),
                        ListTile(
                          contentPadding: EdgeInsets.zero,
                          title: Text(categoryLabel),
                          subtitle: categoryError == null
                              ? const Text('Type and attributes')
                              : Text(categoryError!),
                          trailing: categoriesLoading
                              ? const SizedBox.square(
                                  dimension: 20,
                                  child: CircularProgressIndicator(
                                    strokeWidth: 2,
                                  ),
                                )
                              : const Icon(Icons.chevron_right),
                          onTap: saving || categoriesLoading
                              ? null
                              : chooseCategory,
                        ),
                        ExpansionTile(
                          tilePadding: EdgeInsets.zero,
                          title: const Text('Other descriptions'),
                          children: [
                            lines(descriptors, 'Description', 4, max: 100),
                          ],
                        ),
                      ]),
                      section('Locations', [
                        lines(
                          locations,
                          'Location',
                          12,
                          roles: locationRoles,
                          defaultRole: kind == 'place'
                              ? 'venue'
                              : kind == 'person_service'
                              ? 'practice'
                              : 'context',
                          max: 300,
                        ),
                      ]),
                      section('Details', [
                        lines(
                          observations,
                          'Detail',
                          40,
                          roles: detailKinds,
                          defaultRole: 'context',
                        ),
                      ]),
                      ExpansionTile(
                        tilePadding: EdgeInsets.zero,
                        title: const Text('Related needs'),
                        initiallyExpanded: useCases.isNotEmpty,
                        children: [
                          lines(useCases, 'Related need', 20, max: 200),
                        ],
                      ),
                      section('Useful link', [
                        choice('Link', linkMode, const {
                          'auto': 'Automatic (places)',
                          'custom': 'Use my link',
                          'none': 'No link',
                        }, (v) => linkMode = v),
                        if (linkMode == 'custom') ...[
                          TextFormField(
                            initialValue: linkUrl,
                            enabled: !saving,
                            decoration: const InputDecoration(
                              labelText: 'HTTPS link',
                              hintText: 'https://',
                              border: OutlineInputBorder(),
                            ),
                            keyboardType: TextInputType.url,
                            autocorrect: false,
                            onChanged: (v) => setState(() {
                              linkUrl = v;
                              linkConfirmed = false;
                            }),
                            validator: (v) {
                              final uri = Uri.tryParse(v?.trim() ?? '');
                              return uri == null ||
                                      uri.scheme != 'https' ||
                                      uri.host.isEmpty ||
                                      uri.userInfo.isNotEmpty ||
                                      (v?.length ?? 0) > 2048
                                  ? 'Enter an HTTPS link without a username or password'
                                  : null;
                            },
                          ),
                          const SizedBox(height: 16),
                          textField(
                            'Link label (optional)',
                            linkLabel,
                            (v) => linkLabel = v,
                            max: 40,
                            optional: true,
                          ),
                          if (linkNeedsCheck)
                            CheckboxListTile(
                              contentPadding: EdgeInsets.zero,
                              title: const Text(
                                'This link still matches the updated name and location',
                              ),
                              value: linkConfirmed,
                              onChanged: saving
                                  ? null
                                  : (v) => setState(() => linkConfirmed = v!),
                            ),
                        ],
                      ]),
                      const SizedBox(height: 24),
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
