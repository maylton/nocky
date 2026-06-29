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

## In progress

### Read-only playlist editability metadata

The current checkpoint preserves only the metadata required to decide whether a
future operation may be offered safely:

- authenticated ownership;
- normalized privacy;
- stable playlist identity;
- per-occurrence `videoId` and `setVideoId` identity;
- duplicate occurrence preservation;
- effective editability checks.

This checkpoint is strictly read-only and does not authorize any remote change.

## Next delivery order

1. Package and integrate read-only editability metadata with native playlist
   detail loading.
2. Add a song to an owned playlist with duplicate-submit protection and server
   reconciliation.
3. Rename an owned playlist and change privacy with explicit validation.
4. Remove one exact track occurrence only when both `videoId` and `setVideoId`
   are available.
5. Implement playlist deletion as a separate destructive checkpoint with typed
   confirmation.
6. Return to library robustness: incremental synchronization, cache expiration,
   offline indicators, bounded retry and diagnostics.

## Safety rules

- Never expose edit controls unless ownership is explicitly confirmed.
- Never remove a playlist occurrence without complete occurrence identity.
- Keep creation, metadata edits, item changes and deletion as separate review
  checkpoints.
- Every remote change requires duplicate suppression, actionable failure
  feedback and post-success reconciliation.
- Profile switching remains unavailable while discovery is ambiguous.
- Local Home and the local-library model remain outside Phase 12 changes.

## Administrative state

- Profile foundation issue: completed and closed.
- Playlist creation issue: completed and closed.
- Playlist mutation architecture issue: remains open as the umbrella tracker.
- Conflicted read-only metadata PR: superseded by a current-main replacement.
