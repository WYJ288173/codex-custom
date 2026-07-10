#!/usr/bin/env bash
set -euo pipefail

repo="${CODEX_CUSTOM_REPO:-WYJ288173/codex-custom}"
target="${CODEX_CUSTOM_TARGET:-aarch64-apple-darwin}"
install_dir="${CODEX_CUSTOM_BIN_DIR:-$HOME/.local/bin}"
install_path="${install_dir}/codex-custom"
release="${CODEX_CUSTOM_RELEASE:-latest}"

usage() {
  cat <<'EOF'
Usage: install-codex-custom.sh [--repo owner/name] [--release latest|tag] [--target triple] [--install-dir dir]

Installs or updates the custom Codex CLI binary as codex-custom.

Environment overrides:
  CODEX_CUSTOM_REPO       GitHub repo, default WYJ288173/codex-custom
  CODEX_CUSTOM_RELEASE    Release tag or latest, default latest
  CODEX_CUSTOM_TARGET     Asset target, default aarch64-apple-darwin
  CODEX_CUSTOM_BIN_DIR    Install dir, default ~/.local/bin
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo)
      repo="${2:?--repo requires a value}"
      shift 2
      ;;
    --release)
      release="${2:?--release requires a value}"
      shift 2
      ;;
    --target)
      target="${2:?--target requires a value}"
      shift 2
      ;;
    --install-dir)
      install_dir="${2:?--install-dir requires a value}"
      install_path="${install_dir}/codex-custom"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unexpected argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

for tool in curl python3 tar shasum; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "$tool is required." >&2
    exit 2
  }
done

api_url="https://api.github.com/repos/${repo}/releases"
if [[ "$release" == "latest" ]]; then
  api_url="${api_url}/latest"
else
  api_url="${api_url}/tags/${release}"
fi

tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/codex-custom-install.XXXXXX")"
trap 'rm -rf "$tmp_dir"' EXIT

metadata="${tmp_dir}/release.json"
curl -fsSL "$api_url" -o "$metadata"

asset_urls="$(
  python3 - "$metadata" "$target" <<'PY'
import json
import sys

metadata_path, target = sys.argv[1], sys.argv[2]
data = json.load(open(metadata_path, encoding="utf-8"))
archive = checksum = None
for asset in data.get("assets", []):
    name = asset.get("name", "")
    url = asset.get("browser_download_url", "")
    if name.endswith(f"-{target}.tar.gz"):
        archive = url
    if name.endswith(f"-{target}.tar.gz.sha256"):
        checksum = url
if not archive or not checksum:
    raise SystemExit(f"release does not contain codex-custom assets for {target}")
print(archive)
print(checksum)
PY
)"

archive_url="$(printf '%s\n' "$asset_urls" | sed -n '1p')"
checksum_url="$(printf '%s\n' "$asset_urls" | sed -n '2p')"
archive="${tmp_dir}/codex-custom.tar.gz"
checksum="${tmp_dir}/codex-custom.tar.gz.sha256"

curl -fsSL "$archive_url" -o "$archive"
curl -fsSL "$checksum_url" -o "$checksum"

expected="$(awk '{print $1}' "$checksum")"
actual="$(shasum -a 256 "$archive" | awk '{print $1}')"
if [[ "$expected" != "$actual" ]]; then
  echo "Checksum mismatch: expected $expected, got $actual" >&2
  exit 1
fi

extract_dir="${tmp_dir}/extract"
mkdir -p "$extract_dir" "$install_dir"
tar -xzf "$archive" -C "$extract_dir"

if [[ ! -x "${extract_dir}/codex-custom" ]]; then
  echo "Archive does not contain executable codex-custom." >&2
  exit 1
fi

install -m 0755 "${extract_dir}/codex-custom" "$install_path"
echo "Installed ${install_path}"
"$install_path" --version
