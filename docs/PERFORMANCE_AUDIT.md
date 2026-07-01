# Nocky performance audit

This checkpoint tracks the Home and Queue 2.0 rendering work introduced after Home V2.

## Immediate goals

- Patch delayed Home artwork into mounted cards instead of rebuilding the full feed.
- Share decoded artwork textures across Home, browser rows, the player and Queue 2.0.
- Replace one long horizontal Home rail with responsive multi-row sections: six cards at normal desktop widths, fewer in compact windows and more on wide layouts.
- Share each Home section playback queue instead of cloning it into every card.
- Bound speculative stream preloading so it does not compete with the first interaction.
- Calculate Queue 2.0 drag targets from row geometry instead of measuring every row on every pointer update.

## Structural direction

Keep performance-sensitive presentation helpers outside the already large `browser.rs` and `queue_presentation.rs` files. New layout, artwork-cache and geometry helpers should remain small, testable modules. A later behavior-neutral refactor should continue splitting Home, collection, search and row presentation into domain-focused files.
