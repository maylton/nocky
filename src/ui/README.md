# UI modules

This directory is the transitional home of Nocky's visual modules.

During phase 3, the files are grouped physically by domain while their Rust
module identity remains at the crate root through `#[path]` declarations in
`main.rs`. This keeps the refactor behavior-neutral and preserves existing
`super::` imports.

A later phase can convert these compatibility declarations into a native
`ui::{footer, player, settings, widgets}` hierarchy.
