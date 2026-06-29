# YouTube like mutation foundation

This checkpoint defines the reversible state model for Phase 12C before any GTK
control is connected to the remote rating endpoint.

## State transitions

A mutation begins with a known previous value and a requested target value.

- `Idle`: no request is active.
- `Pending`: the UI may show the target value optimistically.
- `Confirmed`: the remote request succeeded and the target remains visible.
- `RolledBack`: the remote request failed and the previous value becomes visible again.

Only one pending mutation is allowed per video ID. Separate tracks may mutate in
parallel. Empty IDs and no-op transitions are rejected before network access.

Completed mutations may be removed from the registry. Pending mutations cannot
be cleared accidentally.

## Current boundary

This checkpoint does not:

- expose a like button;
- call the YouTube Music rating endpoint;
- modify the library cache;
- persist mutation state;
- add playlist mutations;
- change profile selection or authentication behavior.

The next checkpoint will connect this state model to the existing authenticated
`YouTubeBridge::rate` call and background-message flow, preserving rollback on
all failures.
