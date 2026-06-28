#!/usr/bin/env bash
# zanto one-line installer.
#   curl -fsSL https://raw.githubusercontent.com/satyamyadav/zanto-rust/main/install.sh | bash
#   ... | bash -s -- --system     # install to /usr (needs sudo) instead of ~/.local
#
# Downloads the latest release's portable Linux tarball, extracts it, and runs
# the tarball's own install.sh (the single source of truth for the file copy).
# Uses only curl + tar + grep/sed (no jq). Needs system webkit2gtk-4.1 at runtime.
set -euo pipefail

REPO="satyamyadav/zanto-rust"

arch="$(uname -m)"
if [ "$arch" != "x86_64" ]; then
  echo "Unsupported architecture: $arch (only x86_64 builds are published)." >&2
  exit 1
fi

echo "Finding the latest zanto release…"
# /releases lists newest-first and includes pre-releases. Pull the Linux tarball's
# download URL without jq; the asset name pattern is stable.
api="https://api.github.com/repos/$REPO/releases"
url="$(curl -fsSL "$api" \
  | grep -oE '"browser_download_url": *"[^"]*Linux-x86_64\.tar\.gz"' \
  | head -1 | sed -E 's/.*"(https[^"]+)"/\1/')"

if [ -z "$url" ]; then
  echo "No Linux tarball found in the latest releases of $REPO." >&2
  echo "Grab a .deb/.rpm from https://github.com/$REPO/releases instead." >&2
  exit 1
fi

echo "Downloading $url"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
curl -fsSL "$url" -o "$tmp/zanto.tar.gz"
tar -xzf "$tmp/zanto.tar.gz" -C "$tmp"

dir="$(find "$tmp" -maxdepth 1 -type d -name 'zanto-*-Linux-x86_64' | head -1)"
if [ -z "$dir" ] || [ ! -x "$dir/install.sh" ]; then
  echo "Extracted tarball is missing install.sh." >&2
  exit 1
fi

exec "$dir/install.sh" "$@"
