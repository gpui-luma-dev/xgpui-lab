# Shared lab presentation

Color Spaces is the reference for lab presentation, originally reviewed at
`ed58794d72b3aae73e8fdddb9df5cb84e4656c59` (issue #29). Issue #33 applies
these patterns to Italy Photo Comparison. The reusable implementation is
`src/lab_ui.rs`; both panes in `src/color_demos/` consume it.

Use the shared lab page patterns and match Color Spaces:

- Flat page background and foreground from `Look::chrome()`. Resolve surfaces,
  borders, muted text, and interaction states from the current Radix Look.
  The app theme generates its accent and gray scales from #5A6169 for both
  modes; never copy screenshot RGB values or add a separate palette.
- Full-width centered container, maximum 1240 logical pixels, p6 outer padding,
  gap5 between breadcrumb, header, and content. Content sections use gap4.
- Muted small breadcrumb, semibold text3xl title, gap2 to one muted subtitle.
- Main visualization directly on the page, without an enclosing body card.
- Wrapping, vertically centered control rows use gap3 and the reference
  Radix switch label typography. Keep actual controls as Luma/Radix entities.
- Supporting panels use p4/gap3, rounded-lg corners, a subtle surface and thin
  border. Comparison cards use p3/gap3 and the same surface/radius hierarchy.
- Captions, status, explanations, and provenance use shared small muted text.

## Supported variations

A visualization owns its geometry. Color Spaces retains its responsive color
field; Italy uses a maximum 600 × 400 logical-pixel photograph with a 3:2
aspect ratio, shrinking with available width rather than stretching the fixture.
Do not apply square field geometry to photography.

Comparison cards may contain numerical values or selectable photographs.
Italy uses three centered, wrapping 200-pixel cards with fixed 174 × 116
(3:2) thumbnails. The main photograph is centered on the same content grid.
Photo selection is through these cards; only presentation uses a dropdown. Selection,
hover, press, and keyboard focus resolve from Radix interaction roles and gray
steps. Photo cards remain Luma buttons so input and accessibility behavior
stay with the control. Keep all cards and the large selected image synchronized.

Shared helpers own chrome only. Models, color conversion, media loading,
rendering paths, presentation negotiation, and error state stay in each lab.
Do not migrate unrelated labs simply because these helpers exist.

## Verification

Compare Color Spaces before/after extraction and Italy alongside it at the
same window size in light and dark themes. Check a narrow window for wrapping,
vertical scrolling, visible controls, and undistorted photos. Run the existing
Italy selection/presentation and fixture tests and the Color Spaces interaction
tests; native wide-gamut fidelity still requires a capable display.
