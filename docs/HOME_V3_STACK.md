# Home V3 MetroList stack

This branch restarts the YouTube Music Home work from a clean main baseline.

## Direction

Home V3 is a new Home surface inspired by MetroList. It is separate from the previous Home implementation and should be built in small reviewable steps.

## Plan

1. Add the clean Home V3 helper contract and parser coverage.
2. Add a small Rust bridge contract for Home V3.
3. Add an isolated Home V3 renderer.
4. Wire the YouTube source Home to the isolated renderer.
5. Add chips, continuation, loading and empty states.
6. Polish the MetroList-inspired visual hierarchy after the data and render path is stable.

## Validation checkpoints

Manual validation is needed when the first Home V3 data path reaches the UI, when chips and continuation are wired, and when the visual layout is ready to judge spacing, density and artwork behavior.
