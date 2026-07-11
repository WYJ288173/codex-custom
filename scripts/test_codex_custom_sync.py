#!/usr/bin/env python3

import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


class CodexCustomSyncWorkflowTest(unittest.TestCase):
    def test_source_version_stays_upstream_compatible(self) -> None:
        cargo_toml = REPO_ROOT / "codex-rs" / "Cargo.toml"
        text = cargo_toml.read_text()
        cargo_lock = (REPO_ROOT / "codex-rs" / "Cargo.lock").read_text()

        match = re.search(r'(?m)^\[workspace\.package\]\nversion = "([^"]+)"', text)

        self.assertIsNotNone(match)
        self.assertEqual("0.0.0", match.group(1))
        self.assertNotIn("-custom.", cargo_lock)

    def test_workflow_builds_custom_version_from_latest_upstream_release(self) -> None:
        workflow = (REPO_ROOT / ".github/workflows/codex-custom-sync.yml").read_text()

        self.assertIn("repos/openai/codex/releases/latest", workflow)
        self.assertIn('custom_version="${upstream_version}-custom.1"', workflow)
        self.assertIn("cargo update --workspace", workflow)
        self.assertIn("steps.custom_version.outputs.custom_version", workflow)


if __name__ == "__main__":
    unittest.main()
