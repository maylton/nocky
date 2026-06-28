# YouTube stream-client fallback policy

Nocky resolves YouTube Music audio with `yt-dlp + Deno` and keeps GStreamer as
the playback engine. This phase adds an explicit, observable client policy rather
than repeatedly retrying the same YouTube client identity.

## Default order

With a connected browser session:

1. `web_music` (`WEB_REMIX`)
2. `web_creator` (`WEB_CREATOR`)
3. `tv` (`TVHTML5`)
4. `android_vr` (`Android VR`)
5. `web` (`WEB`)

Without a session, clients that require authentication are skipped. `ios` exists
as an opt-in diagnostic profile but remains disabled by default because current
PO-token and throttling behavior is less predictable.

The optional environment variable `NOCKY_YOUTUBE_STREAM_CLIENTS` can override
the order for development, for example:

```text
NOCKY_YOUTUBE_STREAM_CLIENTS=tv,android_vr,web
```

Unknown and duplicate names are ignored.

## Recovery behavior

A successful stream stores its client identity in the temporary stream cache.
If GStreamer later rejects that URL and Nocky forces a refresh, the failed client
moves to the end of the next attempt sequence. This prevents three retries from
repeating the same client strategy.

Nocky does not rotate clients for terminal availability errors such as private,
removed, region-blocked, copyright-blocked, or age-restricted content. It does
not attempt to bypass those restrictions.

## Diagnostics

Each resolved stream records:

- selected client key and label;
- attempted client keys;
- whether a fallback was needed;
- selected format/container/protocol/codec metadata.

URLs and authentication header values are redacted from aggregated resolver
errors before they are returned to the native application.
