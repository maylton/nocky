#!/usr/bin/env bash
set -Eeuo pipefail

APP_NAME="Nocky"
APP_ID="io.github.maylton.Nocky"
BIN_NAME="nocky"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MODE="user"
PREFIX=""
INSTALL_DEPS=false
BUILD_ONLY=false
ASSUME_YES=false

usage() {
  cat <<'EOF'
Nocky universal source installer

Usage: ./install.sh [OPTIONS]

Options:
  --install-deps   Install common build/runtime dependencies using the detected package manager
  --user           Install for the current user (default: ~/.local)
  --system         Install system-wide under /usr/local (requires sudo)
  --prefix PATH    Install under a custom prefix
  --build-only     Install/check dependencies and build, but do not copy files
  -y, --yes        Pass non-interactive confirmation flags to the package manager
  -h, --help       Show this help

Supported package-manager families:
  apt, dnf, yum, zypper, pacman
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install-deps) INSTALL_DEPS=true ;;
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
    local yes_flag=""
    $ASSUME_YES && yes_flag="-y"
    run_root apt-get update
    run_root apt-get install ${yes_flag:+$yes_flag} \
      build-essential pkg-config cargo rustc \
      libgtk-4-dev libadwaita-1-dev \
      libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
      gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
      gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav \
      desktop-file-utils hicolor-icon-theme libglib2.0-bin
    return
  fi

  if command -v dnf >/dev/null 2>&1; then
    echo "Detected Fedora/RHEL family (dnf)."
    local yes_flag=""
    $ASSUME_YES && yes_flag="-y"
    run_root dnf install ${yes_flag:+$yes_flag} \
      gcc gcc-c++ make pkgconf-pkg-config rust cargo \
      gtk4-devel libadwaita-devel \
      gstreamer1-devel gstreamer1-plugins-base-devel \
      gstreamer1-plugins-base gstreamer1-plugins-good gstreamer1-plugins-bad-free \
      desktop-file-utils hicolor-icon-theme
    return
  fi

  if command -v yum >/dev/null 2>&1; then
    echo "Detected RPM family (yum)."
    local yes_flag=""
    $ASSUME_YES && yes_flag="-y"
    run_root yum install ${yes_flag:+$yes_flag} \
      gcc gcc-c++ make pkgconfig rust cargo \
      gtk4-devel libadwaita-devel \
      gstreamer1-devel gstreamer1-plugins-base-devel \
      gstreamer1-plugins-base gstreamer1-plugins-good \
      desktop-file-utils hicolor-icon-theme
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
    echo "Run with --install-deps or install the matching development package manually." >&2
    exit 1
  fi
done

if command -v gst-inspect-1.0 >/dev/null 2>&1; then
  gst-inspect-1.0 playbin >/dev/null 2>&1 || {
    echo "GStreamer playbin is missing. Install the base plugins package." >&2
    exit 1
  }
  gst-inspect-1.0 spectrum >/dev/null 2>&1 || {
    echo "Warning: the GStreamer spectrum plugin is missing; install the good plugins package for the visualizer." >&2
  }
fi

cd "$ROOT_DIR"
echo "Building ${APP_NAME} 0.1.0 in release mode..."
cargo build --release

if $BUILD_ONLY; then
  echo "Build completed: $ROOT_DIR/target/release/$BIN_NAME"
  exit 0
fi

BIN_DIR="${PREFIX}/bin"
DATA_DIR="${PREFIX}/share"
APP_DIR="${DATA_DIR}/applications"
ICON_DIR="${DATA_DIR}/icons/hicolor"
METAINFO_DIR="${DATA_DIR}/metainfo"

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

install_file 0755 "target/release/${BIN_NAME}" "${BIN_DIR}/${BIN_NAME}"

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

if ((icon_count == 0)); then
  echo "No application icons were found in data/icons/hicolor." >&2
  exit 1
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$ICON_DIR" 2>/dev/null || true
fi

cat <<EOF

${APP_NAME} 0.1.0 installed successfully.
Executable: ${BIN_DIR}/${BIN_NAME}
Desktop entry: ${APP_DIR}/${APP_ID}.desktop
Icons installed: ${icon_count}

If the launcher was already open, close and reopen it once so its application cache refreshes.
EOF
