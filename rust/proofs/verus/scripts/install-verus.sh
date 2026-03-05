#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERUS_VERSION:-0.2026.03.01.25809cb}"
INSTALL_BIN_DIR="${VERUS_INSTALL_BIN_DIR:-$HOME/.local/bin}"
INSTALL_ROOT_DIR="${VERUS_INSTALL_ROOT_DIR:-$HOME/.local/opt}"

uname_s="$(uname -s)"
uname_m="$(uname -m)"

asset_suffix=""
case "${uname_s}:${uname_m}" in
  Darwin:arm64)
    asset_suffix="arm64-macos"
    ;;
  Darwin:x86_64)
    asset_suffix="x86-macos"
    ;;
  Linux:x86_64)
    asset_suffix="x86-linux"
    ;;
  *)
    echo "unsupported platform: ${uname_s}/${uname_m}" >&2
    exit 2
    ;;
esac

archive_name="verus-${VERSION}-${asset_suffix}.zip"
archive_url="https://github.com/verus-lang/verus/releases/download/release/${VERSION}/${archive_name}"
zip_path="/tmp/${archive_name}"
install_dir="${INSTALL_ROOT_DIR}/verus-${VERSION}"

mkdir -p "$INSTALL_BIN_DIR" "$INSTALL_ROOT_DIR"

if [[ ! -x "${install_dir}/verus" ]]; then
  rm -f "$zip_path"
  curl -fL "$archive_url" -o "$zip_path"

  extract_dir="$(mktemp -d /tmp/verus-install-XXXXXX)"
  unzip -q "$zip_path" -d "$extract_dir"
  extracted="$(find "$extract_dir" -mindepth 1 -maxdepth 1 -type d | head -n 1)"
  if [[ -z "$extracted" ]]; then
    echo "failed to locate extracted Verus directory" >&2
    exit 1
  fi

  rm -rf "$install_dir"
  mv "$extracted" "$install_dir"
fi

ln -sfn "${install_dir}/verus" "${INSTALL_BIN_DIR}/verus"
if [[ -f "${install_dir}/cargo-verus" ]]; then
  ln -sfn "${install_dir}/cargo-verus" "${INSTALL_BIN_DIR}/cargo-verus"
fi

if [[ "${uname_s}" == "Darwin" && -f "${install_dir}/macos_allow_gatekeeper.sh" ]]; then
  bash "${install_dir}/macos_allow_gatekeeper.sh" >/dev/null 2>&1 || true
fi

echo "installed verus ${VERSION} -> ${install_dir}"
"${INSTALL_BIN_DIR}/verus" --version
