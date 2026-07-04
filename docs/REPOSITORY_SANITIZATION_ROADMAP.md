# Repository sanitization roadmap

This is the current working roadmap after the Home V3 MetroList stack was merged into `main`.

## Current baseline

- `main` includes PR #103: Home V3 MetroList stack.
- PR #100 and PR #101 are closed as obsolete because they targeted Home V2 diagnostics.
- PR #98 remains useful as a reference, but should not be merged directly into `main` in its current form.
- Future work should start from the current `main` branch and land as small, reviewable PRs.

## Goals

- Keep the repository clean after the Home V3 merge.
- Remove temporary diagnostics, stale bridge code, obsolete helpers and duplicated implementation paths.
- Avoid reintroducing Home V2 artwork/reconciliation regressions.
- Preserve the stable Home V3 behavior now present on `main`.
- Extract only the useful parts of older Material Expressive work into smaller PRs.

## Phase 1: repository audit inventory

Create an inventory before deleting or refactoring anything.

Checklist:

- Search for stale implementation markers: `TODO`, `FIXME`, `HACK`, `TEMP`, `temporary`, `debug`, `trace`, `experimental`, `legacy`, `fallback`.
- Search for visible experimental Home V3 copy.
- Search for debug output such as `println!`, `dbg!`, `eprintln!` and Python `print(..., file=sys.stderr)` that was meant only for validation.
- List the largest Rust, Python, CSS and Markdown files to identify files that may need splitting.
- Review helper installation paths so app installs always include the matching binary, Python helpers and assets from the same commit.

Recommended local commands:

```bash
rg -n \
  "TODO|FIXME|HACK|TEMP|temporary|temporário|debug|trace|experimental|em construção|in progress|legacy|fallback|old Home|Home antiga|force_live|NOCKY_HOME_SOURCE_TRACE|println!|dbg!|eprintln!" \
  src helpers tests docs assets \
  --glob '!target'

find src helpers tests docs assets -type f \
  \( -name '*.rs' -o -name '*.py' -o -name '*.css' -o -name '*.md' \) \
  -not -path '*/target/*' \
  -print0 |
  xargs -0 wc -l |
  sort -nr |
  head -40
```

Output of this phase should be a triage list with four buckets:

1. Remove now.
2. Rename or clarify.
3. Keep temporarily with explanation.
4. Split into a smaller follow-up PR.

## Phase 2: close or split obsolete PRs

The old Home V2 diagnostic PRs are superseded by the merged Home V3 stack.

- PR #100: closed as obsolete.
- PR #101: closed as obsolete.

PR #98 should be treated as superseded by #103 and split into smaller follow-up work instead of being merged directly.

Planned split:

- #98: close as `superseded by #103 / to be split`.
- New PR 1: Material buttons/widgets foundation.
- New PR 2: Material card helpers.
- New PR 3: isolated Carousel component.
- New PR 4: Typography / Google Sans Flex.
- New PR 5: Noctalia/Frosted compatibility polish.

## Phase 3: Home V3 cleanup

After the inventory, review Home V3-specific code for temporary scaffolding.

Focus areas:

- `src/youtube/home_v3_*` modules.
- `src/browser.rs` Home V3 rendering paths.
- `src/app/controller/youtube.rs` Home loading and continuation flow.
- `helpers/nocky_youtube.py` and `helpers/nocky_youtube_home_v3.py`.
- `assets/themes/material-expressive/110-youtube-home-v3-cards.css`.
- `docs/HOME_V3_STACK.md`.

Rules:

- Do not reintroduce weak artwork matching by title/section/layout.
- Keep cover updates keyed by strong identity: `video_id`, `browse_id + result_type`, or `params + result_type`.
- Keep continuation behavior aligned with the current Rust contract until native InnerTube continuation is wired end-to-end.
- Preserve cache-first chip switching and incremental load-more behavior.

## Phase 4: installer and packaging sanity

The installed app must never mix a binary from one commit with helpers/assets from another commit.

Checklist:

- Ensure `nocky_youtube.py` and `nocky_youtube_home_v3.py` are installed together.
- Ensure Material Expressive assets are installed together, especially `110-youtube-home-v3-cards.css`.
- Confirm desktop launchers point to the expected installed binary.
- Add or update install validation commands that compare installed helper hashes against the current worktree/build source.

## Phase 5: documentation consolidation

Consolidate historical implementation notes into current-state documentation.

Candidates:

- Keep `docs/HOME_V3_STACK.md` as the Home V3 architecture/contract document.
- Remove or rewrite sections that describe temporary implementation steps rather than current behavior.
- Document known limitations separately from completed migration notes.

## Phase 6: small follow-up PR policy

All sanitization work should be landed in small PRs.

Recommended sequence:

1. `docs: add repository sanitization roadmap`.
2. `chore: remove stale Home V2 diagnostics`.
3. `chore(home-v3): remove temporary trace and experimental copy`.
4. `refactor(home-v3): clarify legacy bridge naming`.
5. `chore: harden local install helper sync`.
6. `refactor(material): extract buttons/widgets foundation`.
7. `refactor(material): extract card helpers`.
8. `refactor(material): add isolated carousel component`.
9. `style(material): evaluate Google Sans Flex typography`.
10. `style(themes): polish Noctalia and Frosted compatibility`.

Each PR should run the full quality gate before merge:

```bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
git diff --check
```
