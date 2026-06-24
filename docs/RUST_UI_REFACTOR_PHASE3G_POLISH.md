# Footer geometry polish after Phase 3G

## Problems addressed

- The lyrics/volume utility card sat lower than the metadata card in Compact
  mode because its group had an 8 px top margin.
- Long titles could increase the natural width of the metadata card beyond the
  width selected by the Full or Compact footer policy.

## Changes

- utility-group top margin: `8` → `0`;
- title natural-width limit: `22` characters;
- artist width limit represented by an explicit `18`-character constant;
- the now-playing card no longer requests horizontal expansion;
- the card is explicitly aligned to the start side;
- regression tests freeze the alignment and width contracts.

## Preserved

- Full and Compact policy widths;
- ellipsis behavior;
- title and artist contents;
- favorite and source controls;
- compact volume reveal and spring;
- callbacks and application state;
- CSS classes and theme behavior.

Marker: `nocky_footer_optical_alignment_metadata_width_v1`.
