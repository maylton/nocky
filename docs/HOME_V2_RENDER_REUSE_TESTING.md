# Home V2 render reuse validation

PR #84 addresses the performance regression that appears after loading multiple
YouTube Home continuation pages.

## Automated contracts

- a clean mounted YouTube Home can be reused when navigating back;
- dirty, unmounted, searched, local-source and non-Home routes require rendering;
- playback widget identities prefer stable collection IDs and fall back to titles;
- play/pause updates existing Home controls rather than rebuilding the widget tree;
- genuine large YouTube Home rebuilds remove the previous tree immediately instead
  of retaining two full trees during a crossfade.

## Real-account smoke test

1. Open Home V2 and load at least two additional recommendation pages.
2. Record the current vertical position and horizontal positions of a few rows.
3. Toggle play/pause repeatedly and verify there is no Home crossfade, jump or stall.
4. Start a different playlist or album from Home and verify the active button changes.
5. Pause and resume using that same Home button.
6. Navigate to Albums or Playlists and return to Home; unchanged Home content and
   scroll positions should appear immediately.
7. Change a Home chip or synchronize data while away; the next visit should perform
   one necessary refresh and remain responsive afterward.
