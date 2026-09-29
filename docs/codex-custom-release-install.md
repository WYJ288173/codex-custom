# codex-custom Release Install Guide

This repository publishes custom Codex builds as GitHub Releases. Each release is a standalone `codex-custom` binary built from the custom branch after rebasing onto `openai/codex` upstream `main`.

## Release Version Model

Release tags use this format:

```text
codex-custom-<codex-version>-<git-short-sha>
```

Example:

```text
codex-custom-0.142.0-custom.1-13b9f5dee1c9
```

The tag contains two useful parts:

- `<codex-version>`: the custom package version from `codex-rs/Cargo.toml`.
- `<git-short-sha>`: the exact source commit used to build the binary.

This means every downloadable release can be traced back to a specific source commit.

## Supported Artifact

Current workflow target:

```text
aarch64-apple-darwin
```

This is for macOS Apple Silicon. The release contains:

```text
codex-custom-<version>-<sha>-aarch64-apple-darwin.tar.gz
codex-custom-<version>-<sha>-aarch64-apple-darwin.tar.gz.sha256
```

The archive contains:

```text
codex-custom
install-codex-custom.sh
README.md
```

The installer verifies the `.sha256` checksum before installing the binary.

## Install Latest Release

From a local checkout:

```bash
bash scripts/install-codex-custom.sh
```

Or directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/WYJ288173/codex-custom/feature/claude-style-statusline/scripts/install-codex-custom.sh | bash
```

Default install path:

```text
~/.local/bin/codex-custom
```

Verify:

```bash
~/.local/bin/codex-custom --version
```

## Install a Specific Release Version

Use `--release <tag>`:

```bash
bash scripts/install-codex-custom.sh \
  --release codex-custom-0.142.0-custom.1-13b9f5dee1c9
```

Equivalent environment-variable form:

```bash
CODEX_CUSTOM_RELEASE=codex-custom-0.142.0-custom.1-13b9f5dee1c9 \
  bash scripts/install-codex-custom.sh
```

This is the recommended rollback path. If a newer release has a regression, install the previous known-good tag.

## Install to a Custom Path

```bash
bash scripts/install-codex-custom.sh \
  --install-dir "$HOME/bin"
```

This installs:

```text
~/bin/codex-custom
```

Equivalent environment-variable form:

```bash
CODEX_CUSTOM_BIN_DIR="$HOME/bin" bash scripts/install-codex-custom.sh
```

## Install for a Different Target

The installer supports `--target`, but the workflow currently only publishes `aarch64-apple-darwin`.

```bash
bash scripts/install-codex-custom.sh \
  --target aarch64-apple-darwin
```

To support more platforms, add a matrix to `.github/workflows/codex-custom-sync.yml` and publish matching assets, for example:

```text
codex-custom-<version>-<sha>-x86_64-apple-darwin.tar.gz
codex-custom-<version>-<sha>-x86_64-unknown-linux-musl.tar.gz
codex-custom-<version>-<sha>-aarch64-unknown-linux-musl.tar.gz
```

Then users can install with:

```bash
bash scripts/install-codex-custom.sh --target x86_64-apple-darwin
```

## List Available Versions

GitHub UI:

```text
https://github.com/WYJ288173/codex-custom/releases
```

GitHub CLI:

```bash
gh release list --repo WYJ288173/codex-custom
```

Show assets for a specific release:

```bash
gh release view <tag> --repo WYJ288173/codex-custom
```

## Publish a New Release

Run the workflow manually:

```text
Actions -> codex-custom-sync -> Run workflow
```

Use:

```text
publish_release = true
```

The workflow will:

1. Check out `feature/claude-style-statusline`.
2. Rebase it onto `openai/codex` `main`.
3. Run focused validation:
   - `cargo fmt -- --config imports_granularity=Item --check`
   - `cargo test -p codex-tui`
   - `cargo test -p codex-cli`
4. Build the release binary.
5. Package the archive and checksum.
6. Push the rebased custom branch.
7. Create a prerelease with the generated tag.

Use `publish_release=false` when you only want to validate and download the workflow artifact without publishing a version.

## Recommended User Commands

Latest:

```bash
curl -fsSL https://raw.githubusercontent.com/WYJ288173/codex-custom/feature/claude-style-statusline/scripts/install-codex-custom.sh | bash
```

Specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/WYJ288173/codex-custom/feature/claude-style-statusline/scripts/install-codex-custom.sh \
  | bash -s -- --release <tag>
```

Verify:

```bash
codex-custom --version
```

If `codex-custom` is not found, add the install directory to `PATH`:

```bash
export PATH="$HOME/.local/bin:$PATH"
```
