# Remote YouTube Music playlist mutation architecture

The canonical Phase 12D architecture gate now lives at
`docs/PLAYLIST_MUTATION_ARCHITECTURE.md`.

This compatibility note keeps the earlier YouTube-specific filename discoverable
for existing references. The gate remains documentation-only for destructive
playlist operations: Nocky must not expose delete, remove or reorder controls
until ownership, editability, occurrence identity and reconciliation
requirements are satisfied by the later checkpoints.
