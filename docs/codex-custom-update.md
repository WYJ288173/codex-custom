# codex-custom Update Flow

`codex-custom` keeps the footer/status-line patch in a fork. Official `codex update` installs the upstream package and will not carry these source changes, so updates must come from this fork's own build.

## Update Model

```text
openai/codex upstream/main
  -> rebase feature/claude-style-statusline
  -> validate focused TUI/CLI tests
  -> build macOS arm64 codex-custom artifact
  -> install ~/.local/bin/codex-custom
```

## GitHub Action

Workflow: `.github/workflows/codex-custom-sync.yml`.

Manual run is the normal path:

1. Open GitHub Actions.
2. Run `codex-custom-sync` on `feature/claude-style-statusline`.
3. Keep `publish_release=false` to only test and download the workflow artifact.
4. Use `publish_release=true` to push the rebased custom branch and create a prerelease.

The scheduled run performs the same rebase, validation, build, and artifact upload without publishing a release.

If rebase fails, the upstream TUI/footer surface changed and needs manual conflict resolution.

## Local Install Or Update

After a published prerelease exists:

```bash
bash scripts/install-codex-custom.sh
```

The script downloads the latest `codex-custom-*-aarch64-apple-darwin.tar.gz` release asset, verifies the `.sha256`, and installs:

```text
~/.local/bin/codex-custom
```

Official Codex remains separate:

```bash
codex-official
```

Custom Codex runs as:

```bash
codex-custom
```

For installing a specific release tag, rolling back to an older version, or adding more release targets, see:

```text
docs/codex-custom-release-install.md
```

## Local Fallback

When GitHub Actions is unavailable, build locally:

```bash
cd /Users/huayang/developer/codex-custom/.worktrees/claude-style-statusline
git fetch upstream main
git rebase upstream/main
cd codex-rs
cargo fmt -- --config imports_granularity=Item --check
cargo test -p codex-tui
cargo test -p codex-cli
cargo build --release --bin codex
ln -sf /Users/huayang/developer/codex-custom/.worktrees/claude-style-statusline/codex-rs/target/release/codex ~/.local/bin/codex-custom
```

## Boundary

This makes updates repeatable, not maintenance-free. Any upstream change touching `bottom_pane`, `StatusLineItem`, footer layout, token usage, or config parsing can still require manual patch repair.
