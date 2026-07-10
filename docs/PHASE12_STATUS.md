# Phase 12 status

This checkpoint records the current delivery state of the YouTube Music account
and remote-library work. It complements the broader product roadmap and should
be updated whenever a Phase 12 slice is merged.

## Completed

- Assisted browser login and first-run onboarding.
- Active account profile name and handle presentation.
- Privacy-safe profile discovery diagnostics.
- Real authenticated like and unlike operations.
- Optimistic favorite feedback, duplicate suppression, rollback and library
  reconciliation.
- Safe empty-playlist creation contract with private-by-default privacy.
- Native GTK playlist-creation dialog with asynchronous execution and immediate
  playlist-list/cache update.
- Remote playlist mutation safety architecture.
- Read-only playlist editability metadata integrated with native playlist detail
  loading.
- Native single-track addition to confirmed-owned playlists, with duplicate
  submit protection and post-success playlist reconciliation.
- Phase 12D architecture gate documented in
  `PLAYLIST_MUTATION_ARCHITECTURE.md`, with delete, remove, reorder and
  append-source-playlist actions still blocked from native UI.

## In progress

### Remote playlist metadata edits

This remains the next Phase 12 playlist-mutation checkpoint, but the immediate
product priority has temporarily moved to Material 3 Expressive visual-system
consolidation.

The next checkpoint changes playlist properties rather than playlist
membership. It must preserve the same mutation boundary as the shipped
creation/addition slices:

- authenticated ownership and effective editability checks;
- current title, description and privacy shown before submission;
- no submission when no value changes;
- worker-thread execution with duplicate-submit protection;
- fresh playlist/account reconciliation after success;
- localized success, permission, authentication and reconciliation failures.

## Next delivery order

1. Rename an owned playlist and change privacy with explicit validation.
2. Remove one exact track occurrence only when both `videoId` and `setVideoId`
   are available.
3. Implement playlist deletion as a separate destructive checkpoint with typed
   confirmation.
4. Return to library robustness: incremental synchronization, cache expiration,
   offline indicators, bounded retry and diagnostics.

## Safety rules

- Never expose edit controls unless ownership is explicitly confirmed.
- Never remove a playlist occurrence without complete occurrence identity.
- Never expose delete, remove or reorder controls from route/library presence
  alone.
- Keep creation, metadata edits, item changes and deletion as separate review
  checkpoints.
- Every remote change requires duplicate suppression, actionable failure
  feedback and post-success reconciliation.
- Profile switching remains unavailable while discovery is ambiguous.
- Local Home and the local-library model remain outside Phase 12 changes.

## Administrative state

- Profile foundation issue: completed and closed.
- Playlist creation issue: completed and closed.
- Read-only playlist metadata issue: completed and closed.
- Playlist item-addition checkpoint: completed on main.
- Playlist mutation architecture issue: architecture gate completed; keep open
  as the umbrella tracker until destructive membership and deletion checkpoints
  are separately designed, implemented and validated.
