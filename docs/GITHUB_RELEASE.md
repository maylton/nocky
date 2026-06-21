# Publishing Nocky 0.2.4 on GitHub

Run these commands from the extracted `nocky-0.2.4` source directory.

## 1. Validate the source

```bash
cargo fmt --check
cargo check
python3 -m py_compile helpers/nocky_youtube.py
./scripts/verify-release.sh
```

Test local playback and the optional YouTube runtime before publishing:

```bash
./scripts/check-playback.sh
./scripts/check-youtube.sh
cargo run
```

Never include browser cookies, copied cURL requests, `.env` files, `youtube-session.json`, cache files or terminal logs containing session headers.

## 2. Commit the release

```bash
git status
git add .
git commit -m "Release Nocky 0.2.4 beta"
git push origin main
```

## 3. Create and push the tag

```bash
git tag -a v0.2.4-beta -m "Nocky 0.2.4 Beta — Automatic Sync and Library Carousels"
git push origin v0.2.4-beta
```

## 4. Create the GitHub release

Place the generated archives and checksum file beside the project directory, then run:

```bash
gh release create v0.2.4-beta \
  ../nocky-0.2.4-source.zip \
  ../nocky-0.2.4-source.tar.gz \
  ../SHA256SUMS-nocky-0.2.4.txt \
  --title "Nocky 0.2.4 Beta — Automatic Sync and Library Carousels" \
  --notes-file RELEASE_NOTES.md \
  --prerelease \
  --verify-tag
```

## 5. Verify the published release

- GitHub Actions completes successfully.
- The release is marked as a pre-release.
- ZIP, TAR.GZ and SHA-256 checksums are attached.
- No account session or cookie data exists in the repository or release assets.
- The README shows the Nocky icon and documents the default YouTube runtime and `--without-youtube`.
