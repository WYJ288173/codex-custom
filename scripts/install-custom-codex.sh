#!/bin/sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cargo_bin="${CARGO:-cargo}"
if ! command -v "$cargo_bin" >/dev/null 2>&1 \
    && [ -x /opt/homebrew/opt/rustup/bin/cargo ]; then
    cargo_bin=/opt/homebrew/opt/rustup/bin/cargo
fi

(
    cd "$repo_root/codex-rs"
    "$cargo_bin" build --release -p codex-cli
)

source_binary="$repo_root/codex-rs/target/release/codex"
install_dir="${HOME}/.local/bin"
temporary_binary="$install_dir/codex.new"
mkdir -p "$install_dir"
trap 'rm -f "$temporary_binary"' EXIT HUP INT TERM

install -m 0755 "$source_binary" "$temporary_binary"
"$temporary_binary" --version | grep -q "custom"
mv -f "$temporary_binary" "$install_dir/codex"
install -m 0755 "$repo_root/scripts/codex-official" "$install_dir/codex-official"
trap - EXIT HUP INT TERM

"$install_dir/codex" --version
"$install_dir/codex-official" --version
