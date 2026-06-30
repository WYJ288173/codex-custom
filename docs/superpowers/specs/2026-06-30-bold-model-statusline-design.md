# Bold Model Status-Line Design

## Goal

Make the model version visually prominent in the Claude-style Codex status line without changing
layout, colors, truncation, or any other metric.

## Design

- Render the model name as its own green, bold Ratatui `Span`.
- Render the optional ` · reasoning` suffix as a separate green span with normal weight.
- Keep path, branch, metrics, separators, responsive line counts, and missing-data behavior
  unchanged.

## Verification

- Add a focused style assertion that the model span is bold and the reasoning span is not.
- Keep the existing semantic-color assertions.
- Update the narrow-layout snapshot to cover the split spans without changing visible text.
