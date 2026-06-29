# Assisted YouTube Music login — privacy and packaging review

## Status

This document is the mandatory Phase 11 design gate. It does **not** add a web
engine, capture cookies or change authentication behavior.

Implementation must remain in a separate pull request after this review is
accepted.

## Goal

Reduce the friction of copying browser request headers while keeping Nocky a
native GTK music player rather than a general-purpose web wrapper.

The existing manual session import remains supported and is the fallback when
WebKitGTK is unavailable, disabled at build time or rejected by the account.

## Existing security baseline

Nocky currently:

- accepts a browser Cookie/header or copied cURL request;
- rejects sessions without a SAPISID-family cookie;
- persists only an allowlisted set of request headers;
- recomputes `SAPISIDHASH` locally instead of storing a copied authorization
  header as the source of truth;
- prefers Secret Service storage;
- falls back to a permission-restricted `0600` configuration file;
- clears both storage locations on disconnect.

The assisted flow must feed the same normalization and storage functions rather
than creating a second credential format.

## Proposed architecture

### Build boundary

Use an optional Cargo feature named `assisted-login`.

- Default/core builds remain possible without WebKitGTK.
- The feature targets the GTK 4 WebKit API (`webkitgtk-6.0`) through the
  compatible Rust `webkit6` bindings.
- CI must exercise both the default build and the feature-enabled build.
- Runtime UI hides the assisted-login action when the feature is absent.

This keeps the Python helper, headless tests and minimal distribution builds
independent from a browser engine.

### Window and process isolation

The login surface is a dedicated modal window containing one WebView created
with an **ephemeral WebKit context/data manager**.

Required properties:

- no persistent WebKit cookie database;
- no disk cache, IndexedDB, local storage or service-worker data;
- no shared WebView/WebContext with any future web content;
- developer extras disabled;
- no custom JavaScript bridge;
- no DOM queries or form-field inspection;
- no password, username or autofill callbacks exposed to Nocky;
- camera, microphone, geolocation, notifications and screen capture denied;
- file chooser and downloads denied;
- popups either blocked or redirected into the same constrained view after the
  same navigation-policy check.

The context and window are destroyed immediately after success, cancellation or
failure.

### Navigation policy

Only HTTPS top-level navigation is accepted.

The first implementation must use a small audited allowlist covering the Google
Accounts and YouTube Music login/consent flow. The expected initial hosts are:

- `music.youtube.com`;
- `accounts.google.com`;
- `myaccount.google.com`;
- Google/YouTube consent hosts observed in fixture-backed login traces.

Rules:

- exact hosts only; no suffix matching such as `*.google.com`;
- reject IP-address hosts, non-default credentials in URLs and non-HTTPS
  schemes;
- block `file:`, `data:`, `javascript:` and custom schemes;
- block or open unrelated help/legal destinations in the system browser;
- apply the same policy to redirects, new-window requests and back/forward
  navigation;
- keep the allowlist in one tested module, not scattered UI callbacks.

The implementation PR must document every additional host and why the Google
flow requires it.

### Login completion

Nocky does not inspect the page DOM to decide whether login succeeded.

A candidate session is collected only after the top-level page reaches
`https://music.youtube.com/` and loading completes. The WebKit cookie manager is
queried specifically for that URI, not for every domain in the ephemeral
profile.

The candidate is then validated through the existing helper with a lightweight
authenticated YouTube Music request. The session is persisted only when that
request succeeds.

### Session-data boundary

The official browser-authentication model used by `ytmusicapi` relies on the
browser Cookie header, `x-goog-authuser` and the YouTube Music origin. Therefore
the initial assisted implementation may need the full cookie string associated
with `https://music.youtube.com/`; inventing a smaller cookie allowlist without
real-account evidence could create unreliable or account-dependent failures.

Even when the full associated Cookie header is required:

- retrieve cookies only for the YouTube Music URI;
- exclude expired cookies;
- preserve secure/HTTP-only values without exposing them to JavaScript;
- never log cookie names and values together;
- never include values in errors, crash reports, toasts or diagnostics;
- pass the result through the existing minimum-header allowlist;
- store it only through the existing Secret Service / `0600` fallback path;
- discard all temporary cookie objects after validation;
- do not persist the WebKit profile itself.

A later cookie-minimization change requires its own fixture and real-account
validation. It must not be guessed.

### Account selection

`x-goog-authuser` cannot always be assumed to be `0` for users signed into
multiple Google accounts.

The implementation should determine the effective account index from the
validated YouTube Music request when possible. If it cannot be determined
reliably, the UI may offer a small account-index fallback only after validation
fails; it must not expose raw headers.

### Cancellation, logout and errors

- Closing the dialog cancels pending cookie reads and validation.
- A failed validation leaves the existing saved session untouched.
- A successful login atomically replaces the old saved session.
- Disconnect continues to clear both Secret Service and protected-file storage.
- Browser data is ephemeral regardless of success.
- Error messages identify the stage (`navigation`, `cookie capture`,
  `validation`, `storage`) without secrets.

## Threat model

| Threat | Required mitigation |
| --- | --- |
| Credential-field interception | No JS bridge, DOM inspection or form callbacks |
| Cookie leakage through logs | Redacted diagnostics; values never formatted |
| Persistent browser profile | Ephemeral context/data manager only |
| Open redirect to hostile content | Exact HTTPS top-level allowlist on every navigation |
| Popup or new-window escape | Reapply policy and keep within the constrained view |
| Arbitrary file access/download | Disable file chooser, downloads and non-HTTPS schemes |
| Session replacement after failed login | Validate first, then atomically store |
| Cross-account confusion | Validate effective account index and expose clear account state |
| Dependency unavailable | Optional build feature and manual-import fallback |
| Flatpak permission expansion | No broad filesystem or unrestricted D-Bus permissions |

## Packaging review

### Native packages

For Fedora-based builds, the feature-enabled build requires the GTK 4 WebKit
package and development metadata providing `webkitgtk-6.0`.

Other distributions must map this to their WebKitGTK 6 / GTK 4 package. The
installer should detect the pkg-config module before enabling the feature and
show an actionable error rather than silently disabling it.

### Flatpak

The Flatpak build must verify that the chosen GNOME runtime supplies the required
WebKitGTK 6 ABI or bundle it as a normal build module.

The assisted flow must not require:

- host-home filesystem access;
- unrestricted session/system bus access;
- browser-profile access;
- access to another browser's cookies;
- new device permissions.

Network access is already fundamental to the YouTube Music integration. Secret
Service access should reuse the existing storage path; if unavailable, the
existing app-private `0600` fallback remains preferable to broadening filesystem
permissions.

### Binary and maintenance cost

WebKitGTK materially increases build time, runtime size and security-update
surface. Keeping it behind a feature allows maintainers and distributions to
choose between:

- `nocky` — native player with manual browser-session import;
- `nocky` with `assisted-login` — adds the isolated login window.

Release notes must disclose which build includes the feature.

## Implementation sequence after approval

1. Add optional `webkit6` dependency and `assisted-login` Cargo feature.
2. Add a pure, fixture-tested navigation-policy module.
3. Add the ephemeral login dialog with all nonessential capabilities denied.
4. Add cookie capture for the YouTube Music URI.
5. Convert captured cookies into the existing helper session contract.
6. Validate before storage and implement atomic replacement/rollback.
7. Add localized UI copy and retain manual import.
8. Test native and Flatpak builds, cancellation, multi-account behavior,
   expired sessions and logout.
9. Perform a real-account privacy review before merging.

## Acceptance gate

The implementation PR may begin only after explicit approval of these points:

- WebKitGTK is optional and isolated behind a build feature;
- the WebView uses an ephemeral profile;
- Nocky never reads passwords or page DOM;
- top-level navigation uses an exact audited HTTPS allowlist;
- only cookies associated with the YouTube Music URI are collected;
- the existing normalized session format and secure-storage path are reused;
- manual import remains available;
- no broad new Flatpak permission is introduced.

## Primary references

- WebKitGTK ephemeral contexts and website-data management:
  <https://webkitgtk.org/reference/webkit2gtk/2.32.3/WebKitWebContext.html>
- WebKitGTK cookie retrieval for a specific URI:
  <https://webkitgtk.org/reference/webkit2gtk/2.25.1/WebKitCookieManager.html>
- Rust GTK 4 WebKit bindings:
  <https://docs.rs/webkit6/latest/webkit6/>
- ytmusicapi browser authentication:
  <https://ytmusicapi.readthedocs.io/en/stable/setup/browser.html>
- Flatpak sandbox permissions:
  <https://docs.flatpak.org/en/latest/sandbox-permissions.html>
