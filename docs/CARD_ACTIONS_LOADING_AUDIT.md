# Card Actions And Loading States Audit

Status: active checkpoint on `codex/card-actions-loading-states`.

## Scope

This audit covers collection-card actions and loading presentation across:

- Local and YouTube Home sections;
- Albums and Artists collection pages;
- Playlists and mixes;
- categorized search results;
- remote first-paint and collection-loading states.

## Current action coverage

### Home collection cards

Already implemented:

- local albums: open, play/pause, play next, append to queue, favorite;
- local playlists: open, play/pause, play next, append to queue, favorite;
- local mixes: open/play and active play/pause state;
- YouTube albums: open, play/pause, play next, append to queue, favorite,
  offline action;
- YouTube playlists: open, play/pause, play next, append to queue, favorite,
  offline action;
- playable YouTube track cards: direct play and active play/pause state;
- stable play/pause and overflow opacity during carousel scrolling;
- inline Material Loading Indicator after starting a remote collection.

Intentional exception:

- artist cards remain navigation-first. A contextual play action should only be
  added after a deterministic artist queue can be resolved without silently
  choosing unrelated collaboration tracks.

Remaining inconsistencies:

- local mixes and individual track cards have no overflow menu because they do
  not currently expose collection-level queue mutations;
- artist cards have neither play nor overflow actions;
- action controls are richer on Home than on the dedicated collection grids.

### Albums and Artists pages

Current behavior:

- album cards and artist-album cards are navigation-only;
- compact artist cards are navigation-only;
- collection-grid buttons do not yet reuse the Home action overlay;
- loading currently falls back to one placeholder card or one status row.

Recommended action follow-up:

1. extract a reusable collection-card action overlay from the Home card builder;
2. add play/pause and overflow to album and playlist grid cards;
3. keep artist cards navigation-only until artist queue resolution is explicit;
4. preserve a single full-card navigation target and independent accessible
   names for floating controls.

### Search

Current behavior:

- track rows already provide track-level actions through the queue/action menu;
- album, artist and playlist result rows are navigation-only;
- loading uses section headings, status banners and empty/searching rows.

Recommended follow-up:

- keep search rows compact;
- add only the most useful trailing action rather than reproducing the complete
  Home overlay;
- prioritize keyboard-first navigation before adding several icon-only actions.

## Current loading coverage

Implemented:

- reusable Material Loading Indicator;
- inline loading inside remote collection play controls;
- skeleton classes on an existing card while its collection starts loading;
- YouTube Home loading banner;
- loading rows for playlist and collection track pages;
- cached content remains visible during background refreshes.

Missing:

- dedicated placeholder rails before the first remote Home sections arrive;
- placeholder groups for empty album/artist grids while synchronization is in
  progress;
- palette-derived typed placeholders;
- animated removal when placeholder cards are replaced.

## Placeholder-rail contract

The first implementation should be limited to the YouTube Home first paint:

- render one Featured-shaped rail and two Compact-shaped rails;
- use the same final card and scroller dimensions as real content;
- render non-interactive skeleton cards without play or overflow controls;
- retain the localized loading banner above the rails;
- replace the complete placeholder tree when real server sections arrive;
- avoid shimmer when reduced motion is enabled;
- avoid changing the Local Home.

## Acceptance criteria

- no empty-page flash before the first YouTube Home response;
- no fake action buttons on skeletons;
- no horizontal or vertical layout jump when content replaces placeholders;
- carousel edge spring remains disabled for placeholder-only slots;
- cached real sections continue to win over placeholders;
- Local Home rendering remains unchanged;
- Material, Noctalia and Frosted Glass remain source-compatible;
- full formatting, check, tests, Clippy and quality gate pass.

## Implementation order

1. update the active documentation and roadmap contract;
2. add first-paint YouTube Home placeholder rails;
3. validate replacement with cached and fresh Home responses;
4. extract reusable album/playlist action overlays for collection grids;
5. audit keyboard and screen-reader behavior before extending search-row actions.
