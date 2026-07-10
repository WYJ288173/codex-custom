<p align="center"><strong>Codex CLI</strong> is a coding agent from OpenAI that runs locally on your computer.
<p align="center">
  <img src="https://github.com/openai/codex/blob/main/.github/codex-cli-splash.png" alt="Codex CLI splash" width="80%" />
</p>
</br>
If you want Codex in your code editor (VS Code, Cursor, Windsurf), <a href="https://developers.openai.com/codex/ide">install in your IDE.</a>
</br>If you want the desktop app experience, run <code>codex app</code> or visit <a href="https://chatgpt.com/codex?app-landing-page=true">the Codex App page</a>.
</br>If you are looking for the <em>cloud-based agent</em> from OpenAI, <strong>Codex Web</strong>, go to <a href="https://chatgpt.com/codex">chatgpt.com/codex</a>.</p>

---

## About this fork: codex-custom

`codex-custom` is a personal/team fork of the official OpenAI Codex CLI. It is not an official OpenAI distribution.

The purpose of this fork is narrow: keep Codex behavior aligned with upstream while improving the terminal status/footer area for daily CLI use. The official Codex CLI already supports a footer/status line, but the default display is intentionally compact and conservative. For my workflow, that area does not expose enough at-a-glance information, especially when switching models, watching context pressure, and tracking token/usage state during long coding sessions.

This fork keeps a Claude Code-style status line focused on visibility:

- clearer model display in the bottom status/footer area;
- context-window and token usage visibility;
- Today / Week / Month / Total usage-oriented counters where session and rollout data are available;
- status-line layout options that are easier to scan during long-running agent sessions;
- a separate `codex-custom` binary so the official `codex` installation can remain untouched.

The intended operating model is:

```text
official OpenAI Codex
  -> keep following upstream
  -> rebase codex-custom patches
  -> publish codex-custom releases
  -> install as ~/.local/bin/codex-custom
```

Official Codex and this fork should coexist:

- use `codex` or `codex-official` when you want the official distribution;
- use `codex-custom` when you want the custom status/footer experience;
- do not rely on official `codex update` to preserve this fork's source changes.

The fork is intentionally scoped to UI/status-line ergonomics and release/install automation. It should not change the core agent model, authentication model, API semantics, or OpenAI service behavior beyond what is necessary to keep the custom TUI surface working with upstream.

For update and install details:

- [codex-custom update flow](./docs/codex-custom-update.md)
- [codex-custom release install guide](./docs/codex-custom-release-install.md)

---

## Quickstart

### Installing and running Codex CLI

Run the following on Mac or Linux to install Codex CLI:

```shell
curl -fsSL https://chatgpt.com/codex/install.sh | sh
```

Run the following on Windows to install Codex CLI:

```shell
powershell -ExecutionPolicy ByPass -c "irm https://chatgpt.com/codex/install.ps1 | iex"
```

Codex CLI can also be installed via the following package managers:

```shell
# Install using npm
npm install -g @openai/codex
```

```shell
# Install using Homebrew
brew install --cask codex
```

Then simply run `codex` to get started.

<details>
<summary>You can also go to the <a href="https://github.com/openai/codex/releases/latest">latest GitHub Release</a> and download the appropriate binary for your platform.</summary>

Each GitHub Release contains many executables, but in practice, you likely want one of these:

- macOS
  - Apple Silicon/arm64: `codex-aarch64-apple-darwin.tar.gz`
  - x86_64 (older Mac hardware): `codex-x86_64-apple-darwin.tar.gz`
- Linux
  - x86_64: `codex-x86_64-unknown-linux-musl.tar.gz`
  - arm64: `codex-aarch64-unknown-linux-musl.tar.gz`

Each archive contains a single entry with the platform baked into the name (e.g., `codex-x86_64-unknown-linux-musl`), so you likely want to rename it to `codex` after extracting it.

</details>

### Using Codex with your ChatGPT plan

Run `codex` and select **Sign in with ChatGPT**. We recommend signing into your ChatGPT account to use Codex as part of your Plus, Pro, Business, Edu, or Enterprise plan. [Learn more about what's included in your ChatGPT plan](https://help.openai.com/en/articles/11369540-codex-in-chatgpt).

You can also use Codex with an API key, but this requires [additional setup](https://developers.openai.com/codex/auth#sign-in-with-an-api-key).

## Docs

- [**Codex Documentation**](https://developers.openai.com/codex)
- [**Contributing**](./docs/contributing.md)
- [**Installing & building**](./docs/install.md)
- [**codex-custom update flow**](./docs/codex-custom-update.md)
- [**codex-custom release install guide**](./docs/codex-custom-release-install.md)
- [**Open source fund**](./docs/open-source-fund.md)

This repository is licensed under the [Apache-2.0 License](LICENSE).
