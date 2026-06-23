#!/usr/bin/env bash
set -Eeuo pipefail

APP_NAME="Nocky"
APP_ID="io.github.maylton.Nocky"
BIN_NAME="nocky"
DENO_VERSION="2.8.3"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERSION="$(awk -F'"' '/^version = / { print $2; exit }' "$ROOT_DIR/Cargo.toml")"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]] || {
  echo "Could not determine Nocky version from Cargo.toml." >&2
  exit 1
}
MODE="user"
PREFIX=""
INSTALL_DEPS=false
INSTALL_YOUTUBE=true
BUILD_ONLY=false
ASSUME_YES=false

usage() {
  cat <<'EOF'
Nocky universal source installer

Usage: ./install.sh [OPTIONS]

Options:
  --install-deps      Install common GTK/GStreamer build dependencies
  --install-youtube   Install the isolated YouTube Music runtime (default)
  --without-youtube   Skip the optional YouTube Music runtime
  --user              Install for the current user (default: ~/.local)
  --system            Install system-wide under /usr/local (requires sudo)
  --prefix PATH       Install under a custom prefix
  --build-only        Build without copying application files
  -y, --yes           Use non-interactive package-manager confirmation
  --version           Show the Nocky version
  -h, --help          Show this help

Supported package-manager families:
  apt, dnf, yum, zypper, pacman

Recommended complete installation:
  ./install.sh --install-deps

Local-only installation:
  ./install.sh --install-deps --without-youtube
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install-deps) INSTALL_DEPS=true ;;
    --install-youtube) INSTALL_YOUTUBE=true ;;
    --without-youtube) INSTALL_YOUTUBE=false ;;
    --user) MODE="user" ;;
    --system) MODE="system" ;;
    --prefix)
      shift
      [[ $# -gt 0 ]] || { echo "--prefix requires a path" >&2; exit 2; }
      MODE="custom"
      PREFIX="$1"
      ;;
    --build-only) BUILD_ONLY=true ;;
    -y|--yes) ASSUME_YES=true ;;
    --version) printf "%s %s\n" "$APP_NAME" "$VERSION"; exit 0 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

case "$MODE" in
  user) PREFIX="${HOME}/.local" ;;
  system) PREFIX="/usr/local" ;;
  custom) : ;;
esac

run_root() {
  if [[ ${EUID} -eq 0 ]]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    echo "This operation requires root privileges, but sudo is unavailable." >&2
    exit 1
  fi
}

install_dependencies() {
  echo "Detecting package manager..."

  if command -v apt-get >/dev/null 2>&1; then
    echo "Detected Debian/Ubuntu family (apt)."
    local args=()
    $ASSUME_YES && args+=(-y)
    run_root apt-get update
    run_root apt-get install "${args[@]}" \
      build-essential pkg-config cargo rustc \
      libgtk-4-dev libadwaita-1-dev \
      libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
      gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
      gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav \
      desktop-file-utils hicolor-icon-theme libglib2.0-bin
    if $INSTALL_YOUTUBE; then
      run_root apt-get install "${args[@]}" \
        python3 python3-venv python3-pip python3-gi \
        gir1.2-secret-1 libsecret-1-0 curl unzip
    fi
    return
  fi

  if command -v dnf >/dev/null 2>&1; then
    echo "Detected Fedora/RHEL family (dnf)."
    local args=()
    $ASSUME_YES && args+=(-y)
    run_root dnf install "${args[@]}" \
      gcc gcc-c++ make pkgconf-pkg-config rust cargo \
      gtk4-devel libadwaita-devel \
      gstreamer1-devel gstreamer1-plugins-base-devel \
      gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-bad-free \
      desktop-file-utils hicolor-icon-theme
    if $INSTALL_YOUTUBE; then
      run_root dnf install "${args[@]}" \
        python3 python3-pip python3-gobject libsecret curl unzip
    fi
    return
  fi

  if command -v yum >/dev/null 2>&1; then
    echo "Detected RPM family (yum)."
    local args=()
    $ASSUME_YES && args+=(-y)
    run_root yum install "${args[@]}" \
      gcc gcc-c++ make pkgconfig rust cargo \
      gtk4-devel libadwaita-devel \
      gstreamer1-devel gstreamer1-plugins-base-devel \
      gstreamer1-plugins-base gstreamer1-plugins-good \
      desktop-file-utils hicolor-icon-theme
    if $INSTALL_YOUTUBE; then
      run_root yum install "${args[@]}" \
        python3 python3-pip python3-gobject libsecret curl unzip
    fi
    return
  fi

  if command -v zypper >/dev/null 2>&1; then
    echo "Detected openSUSE family (zypper)."
    run_root zypper --non-interactive install \
      gcc gcc-c++ make pkg-config rust cargo \
      gtk4-devel libadwaita-devel \
      gstreamer-devel gstreamer-plugins-base-devel \
      gstreamer-plugins-base gstreamer-plugins-good \
      desktop-file-utils hicolor-icon-theme
    if $INSTALL_YOUTUBE; then
      run_root zypper --non-interactive install \
        python3 python3-pip python3-gobject libsecret-1-0 curl unzip
    fi
    return
  fi

  if command -v pacman >/dev/null 2>&1; then
    echo "Detected Arch family (pacman)."
    local args=(-S --needed)
    $ASSUME_YES && args+=(--noconfirm)
    run_root pacman "${args[@]}" \
      base-devel pkgconf rust gtk4 libadwaita gstreamer \
      gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav \
      desktop-file-utils hicolor-icon-theme
    if $INSTALL_YOUTUBE; then
      run_root pacman "${args[@]}" \
        python python-pip python-gobject libsecret curl unzip
    fi
    return
  fi

  echo "Unsupported package manager. Install Rust/Cargo, GTK4, libadwaita and GStreamer development packages manually." >&2
  exit 1
}

if $INSTALL_DEPS; then
  install_dependencies
fi

missing=()
for command_name in cargo rustc pkg-config; do
  command -v "$command_name" >/dev/null 2>&1 || missing+=("$command_name")
done
if ((${#missing[@]})); then
  printf 'Missing required tools: %s\n' "${missing[*]}" >&2
  echo "Run this installer again with --install-deps, or install them manually." >&2
  exit 1
fi

for package in gtk4 libadwaita-1 gstreamer-1.0; do
  if ! pkg-config --exists "$package"; then
    echo "Missing development package detected by pkg-config: $package" >&2
    exit 1
  fi
done

if command -v gst-inspect-1.0 >/dev/null 2>&1; then
  gst-inspect-1.0 playbin >/dev/null 2>&1 || {
    echo "GStreamer playbin is missing. Install the base plugins package." >&2
    exit 1
  }
  gst-inspect-1.0 spectrum >/dev/null 2>&1 || {
    echo "Warning: install the GStreamer good plugins package for the visualizer." >&2
  }
fi

cd "$ROOT_DIR"
echo "Building ${APP_NAME} ${VERSION} in release mode..."
cargo build --release --locked

if $BUILD_ONLY; then
  echo "Build completed: $ROOT_DIR/target/release/$BIN_NAME"
  exit 0
fi

BIN_DIR="${PREFIX}/bin"
DATA_DIR="${PREFIX}/share"
APP_DIR="${DATA_DIR}/applications"
ICON_DIR="${DATA_DIR}/icons/hicolor"
METAINFO_DIR="${DATA_DIR}/metainfo"
NOCKY_DATA_DIR="${DATA_DIR}/nocky"
HELPER_DIR="${NOCKY_DATA_DIR}/helpers"
RUNTIME_DIR="${NOCKY_DATA_DIR}/runtime"

needs_root=false
if [[ "$MODE" == "system" ]]; then
  needs_root=true
elif [[ "$MODE" == "custom" ]] && [[ ${EUID} -ne 0 ]]; then
  parent="$PREFIX"
  while [[ ! -e "$parent" && "$parent" != "/" ]]; do
    parent="$(dirname "$parent")"
  done
  [[ -w "$parent" ]] || needs_root=true
fi

install_file() {
  local mode="$1" source="$2" destination="$3"
  if $needs_root; then
    run_root install -D -m "$mode" "$source" "$destination"
  else
    install -D -m "$mode" "$source" "$destination"
  fi
}

run_install_command() {
  if $needs_root; then
    run_root "$@"
  else
    "$@"
  fi
}

install_file 0755 "target/release/${BIN_NAME}" "${BIN_DIR}/${BIN_NAME}"
install_file 0755 "helpers/nocky_youtube.py" "${HELPER_DIR}/nocky_youtube.py"

tmp_desktop="$(mktemp)"
trap 'rm -f "$tmp_desktop"' EXIT
sed \
  -e "s|^Exec=.*|Exec=${BIN_DIR}/${BIN_NAME}|" \
  -e "s|^TryExec=.*|TryExec=${BIN_DIR}/${BIN_NAME}|" \
  "data/${APP_ID}.desktop" > "$tmp_desktop"
install_file 0644 "$tmp_desktop" "${APP_DIR}/${APP_ID}.desktop"
install_file 0644 "data/${APP_ID}.metainfo.xml" "${METAINFO_DIR}/${APP_ID}.metainfo.xml"

icon_count=0
while IFS= read -r -d '' icon; do
  size_dir="$(basename "$(dirname "$(dirname "$icon")")")"
  install_file 0644 "$icon" "${ICON_DIR}/${size_dir}/apps/${APP_ID}.png"
  icon_count=$((icon_count + 1))
done < <(find "data/icons/hicolor" -type f -path "*/apps/${APP_ID}.png" -print0 | sort -z)

if $INSTALL_YOUTUBE; then
  for command_name in python3 curl unzip; do
    command -v "$command_name" >/dev/null 2>&1 || {
      echo "Missing command required by --install-youtube: $command_name" >&2
      exit 1
    }
  done

  echo "Creating isolated YouTube Music runtime..."
  run_install_command rm -rf "$RUNTIME_DIR"
  run_install_command python3 -m venv --system-site-packages "$RUNTIME_DIR"
  run_install_command "$RUNTIME_DIR/bin/python3" -m pip install --upgrade pip
  run_install_command "$RUNTIME_DIR/bin/python3" -m pip install \
    -r "$ROOT_DIR/requirements-youtube.txt"
  run_install_command "$RUNTIME_DIR/bin/python3" -c \
    "import requests, ytmusicapi, yt_dlp; print('YouTube Music Python runtime verified')"

  if ! command -v deno >/dev/null 2>&1; then
    case "$(uname -m)" in
      x86_64|amd64) deno_arch="x86_64-unknown-linux-gnu" ;;
      aarch64|arm64) deno_arch="aarch64-unknown-linux-gnu" ;;
      *)
        echo "Deno could not be bundled automatically for architecture $(uname -m)." >&2
        echo "Install Deno manually before using YouTube Music." >&2
        deno_arch=""
        ;;
    esac
    if [[ -n "$deno_arch" ]]; then
      temp_dir="$(mktemp -d)"
      curl -fL \
        "https://github.com/denoland/deno/releases/download/v${DENO_VERSION}/deno-${deno_arch}.zip" \
        -o "$temp_dir/deno.zip"
      unzip -q "$temp_dir/deno.zip" -d "$temp_dir"
      install_file 0755 "$temp_dir/deno" "$RUNTIME_DIR/bin/deno"
      rm -rf "$temp_dir"
    fi
  fi

  echo "YouTube Music runtime installed at ${RUNTIME_DIR}."
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
fi

cat <<EOF

${APP_NAME} ${VERSION} installed successfully.
Executable: ${BIN_DIR}/${BIN_NAME}
Desktop entry: ${APP_DIR}/${APP_ID}.desktop
Icons installed: ${icon_count}
YouTube helper: ${HELPER_DIR}/nocky_youtube.py
YouTube runtime: $($INSTALL_YOUTUBE && echo installed || echo not-installed)

If the launcher was already open, close and reopen it once so its application cache refreshes.
EOF
