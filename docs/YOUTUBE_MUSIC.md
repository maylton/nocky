# YouTube Music integration

Nocky 0.2.5 uses the same working architecture as the author's Nocturne project: `ytmusicapi` for catalogue/account data and `yt-dlp` with Deno for temporary audio URLs.

## Install

```bash
./install.sh --install-deps
```

This creates an isolated runtime below the selected installation prefix. It does not modify your normal Python environment.

## Public search

Open the **YouTube Music** page and search normally. Public search does not require an account session.

## Connect an account

Account connection unlocks your library, liked songs and playlists.

1. Sign in to `music.youtube.com` in your browser.
2. Open the browser developer tools.
3. Select the **Network** tab and reload YouTube Music.
4. Select a successful request sent to `music.youtube.com`.
5. Use **Copy as cURL**, or copy the complete `Cookie` request header.
6. In Nocky, open **YouTube Music → Connect account**.
7. Paste the copied request and choose **Import browser session**.

Nocky parses only the request headers needed by `ytmusicapi`. You do not need to provide your Google password.

## Security

The copied request contains sensitive session credentials. Treat it like a password:

- never paste it into a GitHub issue, chat, screenshot or public log;
- do not share `~/.config/nocky/youtube-session.json`;
- disconnect the account in Nocky before sharing a debug archive;
- revoke browser sessions from your Google account when appropriate.

Nocky stores the session in Secret Service/libsecret when available. When Secret Service cannot be used, it stores a fallback file at:

```text
~/.config/nocky/youtube-session.json
```

The fallback file is created with permissions `0600`.

## Disconnect

Use **Disconnect** on the YouTube Music page. Nocky removes both the Secret Service item and the fallback file.

## Stream playback

When you select a song, Nocky asks `yt-dlp` to resolve a temporary audio URL. Deno is used as the JavaScript runtime. The URL and required HTTP headers are sent to Nocky's existing GStreamer `playbin` pipeline. The audio file is not permanently downloaded.

Resolved URLs and their required HTTP headers are cached until shortly before expiry:

```text
~/.cache/nocky/youtube/stream-cache.json
```

The cache keeps the 80 freshest valid entries. When a queue starts, Nocky resolves the next four tracks in the background, so next/previous playback usually starts without waiting for a new `yt-dlp` process.

The synchronized library and any online playlists already opened by the user are cached under:

```text
~/.cache/nocky/youtube/library-cache.json
```

This snapshot is rendered immediately at startup while Nocky refreshes the account library in the background.

Cover images are cached under:

```text
~/.cache/nocky/youtube/covers/
```

Collection cards request a 512 px image and the now-playing view requests a 1200 px image. Cache keys include the final upgraded URL and requested size, preventing an older low-resolution file from being reused after metadata improves.

## Diagnostics

```bash
./scripts/check-youtube.sh
```

The diagnostic checks dependencies and calls only the helper's safe `status` command. It never prints cookies or request headers.

## Troubleshooting

### “YouTube Music dependencies are not installed”

Reinstall with:

```bash
./install.sh --install-youtube
```

### The account session stopped working

Browser sessions can expire. Disconnect, copy a fresh request from `music.youtube.com`, and connect again.

### yt-dlp cannot resolve a stream

Run:

```bash
./scripts/check-youtube.sh
```

Confirm that both yt-dlp and Deno are available. A newer YouTube change may also require updating the pinned yt-dlp release.

### Playback opens but no sound is produced

Run:

```bash
./scripts/check-playback.sh
```

Make sure the distribution's GStreamer base/good/bad/ugly/libav plugin sets are installed.

## Running from source

`cargo run` does not automatically use the Python environment installed beside a system copy of Nocky. Version 0.2.5 searches all known runtimes correctly, and also supports a dedicated project runtime:

```bash
./scripts/setup-youtube-runtime.sh
cargo run
```

The runtime is created in `.nocky-runtime/` and excluded from Git.

## Source mode

The first launch asks whether Nocky should run in **Local library** or **YouTube Music** mode. This is a strict content mode, not only a startup-page preference.

- Local mode remains offline, excludes every YouTube collection from Home and browser routes, and does not automatically contact YouTube.
- YouTube Music mode excludes local tracks and local playlists from the browser and enables account status/synchronization.

Change the saved mode later from **Settings**. Switching modes returns to Home and clears playback state from the previous source. Playlist contents are loaded on demand and then reused from `library-cache.json`.
