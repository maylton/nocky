#!/usr/bin/env bash
set -euo pipefail

command -v gst-inspect-1.0 >/dev/null || {
  echo "gst-inspect-1.0 was not found. Install the gstreamer package." >&2
  exit 1
}

gst-inspect-1.0 playbin >/dev/null || {
  echo "The GStreamer playbin element is unavailable. Install gst-plugins-base." >&2
  exit 1
}

gst-inspect-1.0 spectrum >/dev/null || {
  echo "The GStreamer spectrum element is unavailable. Install gst-plugins-good." >&2
  exit 1
}

echo "GStreamer playbin: available"
echo "GStreamer spectrum analyzer: available"
echo "Running cargo check..."
cargo check
