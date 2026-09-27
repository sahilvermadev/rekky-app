import 'package:flutter/material.dart';

import 'rekky_haptics.dart';
import 'rekky_theme.dart';

// Hallmark · component: three-part navigation · flat primary colours.
// Pre-emit critique: P5 H5 E5 S5 R5 V4.
class RekkyNavigation extends StatefulWidget {
  const RekkyNavigation({
    super.key,
    required this.destination,
    required this.onSelect,
    required this.onRemember,
  });
  final int destination;
  final ValueChanged<int> onSelect;
  final VoidCallback onRemember;

  @override
  State<RekkyNavigation> createState() => _RekkyNavigationState();
}

class _RekkyNavigationState extends State<RekkyNavigation> {
  final _askStates = WidgetStatesController();
  final _libraryStates = WidgetStatesController();
  final _recommendStates = WidgetStatesController();

  @override
  void initState() {
    super.initState();
    for (final controller in [_askStates, _libraryStates, _recommendStates]) {
      controller.addListener(_refreshStates);
    }
  }

  void _refreshStates() {
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    _askStates.dispose();
    _libraryStates.dispose();
    _recommendStates.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colors = theme.colorScheme;
    final reducedMotion = MediaQuery.disableAnimationsOf(context);
    Duration motion(int milliseconds) =>
        reducedMotion ? Duration.zero : Duration(milliseconds: milliseconds);
    final labelStyle = theme.textTheme.labelLarge!.copyWith(
      fontSize: 15,
      fontWeight: FontWeight.w600,
      letterSpacing: 0,
    );
    Widget tab(int index, String label, double labelWidth) {
      final selected = widget.destination == index;
      final states = index == 0 ? _askStates : _libraryStates;
      final base = index == 0 ? RekkyTheme.navAsk : RekkyTheme.navLibrary;
      final foreground = index == 0
          ? RekkyTheme.onNavAsk
          : RekkyTheme.onNavLibrary;
      final amount = states.value.contains(WidgetState.pressed)
          ? .16
          : states.value.contains(WidgetState.focused)
          ? .12
          : selected
          ? .06
          : 0.0;
      return AnimatedContainer(
        key: ValueKey('nav-side-$index'),
        duration: motion(220),
        curve: Curves.easeOutCubic,
        color: Color.lerp(base, foreground, amount),
        child: Semantics(
          selected: selected,
          child: TextButton(
            statesController: states,
            onPressed: () {
              if (!selected) RekkyHaptics.selection();
              widget.onSelect(index);
            },
            style: TextButton.styleFrom(
              enableFeedback: false,
              foregroundColor: foreground,
              overlayColor: Colors.transparent,
              splashFactory: NoSplash.splashFactory,
              minimumSize: const Size(48, 60),
              padding: EdgeInsets.zero,
              shape: const RoundedRectangleBorder(),
            ),
            child: Align(
              alignment: index == 0
                  ? Alignment.centerLeft
                  : Alignment.centerRight,
              child: SizedBox(
                width: labelWidth,
                child: Center(
                  child: AnimatedSlide(
                    duration: motion(200),
                    curve: Curves.easeOutCubic,
                    offset: selected && !reducedMotion
                        ? const Offset(0, -.025)
                        : Offset.zero,
                    child: AnimatedDefaultTextStyle(
                      duration: motion(200),
                      curve: Curves.easeOutCubic,
                      style: labelStyle.copyWith(
                        color: foreground,
                        fontWeight: selected
                            ? FontWeight.w700
                            : FontWeight.w500,
                      ),
                      child: Text(label, maxLines: 1),
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
    }

    Widget recommend() => Semantics(
      hint: 'Start recording a recommendation',
      child: ClipRRect(
        clipper: const _CapsuleClipper(),
        child: AnimatedScale(
          scale:
              !reducedMotion &&
                  _recommendStates.value.contains(WidgetState.pressed)
              ? .975
              : 1,
          duration: motion(
            _recommendStates.value.contains(WidgetState.pressed) ? 100 : 190,
          ),
          curve: Curves.easeOutCubic,
          child: FilledButton(
            statesController: _recommendStates,
            onPressed: widget.onRemember,
            style: FilledButton.styleFrom(
              enableFeedback: false,
              backgroundColor: RekkyTheme.capture,
              foregroundColor: RekkyTheme.onCapture,
              textStyle: labelStyle.copyWith(fontWeight: FontWeight.w700),
              minimumSize: const Size(48, 60),
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
              shape: const StadiumBorder(),
            ),
            child: const Text('Recommend', maxLines: 1),
          ),
        ),
      ),
    );
    return ColoredBox(
      color: colors.surface,
      child: SafeArea(
        top: false,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(12, 0, 12, 4),
          child: Center(
            heightFactor: 1,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 560),
              child: LayoutBuilder(
                builder: (context, constraints) {
                  double measure(String text) {
                    final painter = TextPainter(
                      text: TextSpan(
                        text: text,
                        style: labelStyle.copyWith(fontWeight: FontWeight.w700),
                      ),
                      textDirection: Directionality.of(context),
                      textScaler: MediaQuery.textScalerOf(context),
                    )..layout();
                    final width = painter.width;
                    painter.dispose();
                    return width;
                  }

                  final sideWidth = (measure('Library') + 16).clamp(
                    constraints.maxWidth / 4,
                    double.infinity,
                  );
                  final centreWidth = measure('Recommend') + 32;
                  final fits =
                      sideWidth * 2 + centreWidth <= constraints.maxWidth;
                  return ClipRRect(
                    clipper: const _CapsuleClipper(),
                    child: fits
                        ? SizedBox(
                            height: 60,
                            child: Stack(
                              fit: StackFit.expand,
                              children: [
                                Row(
                                  crossAxisAlignment:
                                      CrossAxisAlignment.stretch,
                                  children: [
                                    Expanded(child: tab(0, 'Ask', sideWidth)),
                                    Expanded(
                                      child: tab(1, 'Library', sideWidth),
                                    ),
                                  ],
                                ),
                                Positioned(
                                  left: sideWidth,
                                  right: sideWidth,
                                  top: 0,
                                  bottom: 0,
                                  child: recommend(),
                                ),
                              ],
                            ),
                          )
                        : Column(
                            // Reflow only when scaled labels cannot fit in one row.
                            mainAxisSize: MainAxisSize.min,
                            crossAxisAlignment: CrossAxisAlignment.stretch,
                            children: [
                              recommend(),
                              Row(
                                children: [
                                  Expanded(child: tab(0, 'Ask', sideWidth)),
                                  Expanded(child: tab(1, 'Library', sideWidth)),
                                ],
                              ),
                            ],
                          ),
                  );
                },
              ),
            ),
          ),
        ),
      ),
    );
  }
}

// An explicit clipper makes the visible curve the hit boundary as well.
class _CapsuleClipper extends CustomClipper<RRect> {
  const _CapsuleClipper();

  @override
  RRect getClip(Size size) => RRect.fromRectAndRadius(
    Offset.zero & size,
    Radius.circular(size.height / 2),
  );

  @override
  bool shouldReclip(_CapsuleClipper oldClipper) => false;
}
