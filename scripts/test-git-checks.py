#!/usr/bin/env python3
"""Offline fixtures for the executable pre-commit and existing CI path scopes."""
import fnmatch
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class HookTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="homie hook ")
        self.addCleanup(self.temp.cleanup)
        self.repo = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "user.email", "fixture@example.invalid")
        self.git("config", "user.name", "Fixture")
        (self.repo / "README.md").write_text("fixture\n")
        self.git("add", "README.md")
        self.git("commit", "-qm", "fixture")
        self.bin = self.repo / "bin"
        self.bin.mkdir()
        cargo = self.bin / "cargo"
        cargo.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$CALLS"\n'
                         'if [ "$1" = "$FAIL_COMMAND" ]; then exit 23; fi\n')
        cargo.chmod(0o755)
        self.calls = self.repo / "calls"

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.repo, text=True).strip()

    def stage(self, path):
        file = self.repo / path
        file.parent.mkdir(parents=True, exist_ok=True)
        file.write_text("fixture\n")
        self.git("add", "--", path)

    def hook(self, fail=""):
        self.calls.write_text("")
        result = subprocess.run([str(ROOT / ".githooks/pre-commit")], cwd=self.repo,
                                env={**os.environ, "PATH": f"{self.bin}:{os.environ['PATH']}",
                                     "CALLS": str(self.calls), "FAIL_COMMAND": fail},
                                capture_output=True, text=True)
        return result, self.calls.read_text().splitlines()

    def test_workspace_shared_and_gitlink_inputs(self):
        for path in ["src/gateway/src/path with spaces.rs", "Cargo.toml", "Cargo.lock",
                     ".gitmodules", ".cargo/config.toml", "rust-toolchain.toml"]:
            with self.subTest(path=path):
                self.git("reset", "-q")
                self.stage(path)
                result, calls = self.hook()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(calls, ["fmt --all -- --check", "clippy --locked -p homie-core --all-targets --all-features -- -D warnings"])
        self.git("reset", "-q")
        self.git("update-index", "--add", "--cacheinfo", f"160000,{self.git('rev-parse', 'HEAD')},src/infra/roci")
        self.assertEqual(len(self.hook()[1]), 2)

    def test_deleted_and_renamed_rust_inputs(self):
        self.stage("src/gateway/deleted.rs")
        self.git("commit", "-qm", "source")
        self.git("rm", "src/gateway/deleted.rs")
        self.assertEqual(len(self.hook()[1]), 2)
        self.git("reset", "--hard", "-q")
        self.git("mv", "src/gateway/deleted.rs", "renamed.md")
        self.assertEqual(len(self.hook()[1]), 2)

    def test_embedded_markdown_modification_and_deletion_check_core(self):
        self.stage('src/core/system_prompt.md')
        self.git('commit', '-qm', 'embedded input')
        self.stage('src/core/system_prompt.md')
        (self.repo / 'src/core/system_prompt.md').write_text('changed input\n')
        self.git('add', 'src/core/system_prompt.md')
        self.assertEqual(len(self.hook()[1]), 2)
        self.git('reset', '--hard', '-q')
        self.git('rm', 'src/core/system_prompt.md')
        self.assertEqual(len(self.hook()[1]), 2)

    def test_nonzero_commands_stop_gate(self):
        self.stage("Cargo.lock")
        for command in ["fmt", "clippy"]:
            with self.subTest(command=command):
                self.assertEqual(self.hook(command)[0].returncode, 23)

    def test_empty_and_docs_skip(self):
        self.assertEqual(self.hook()[1], [])
        self.stage("README.md")
        self.assertEqual(self.hook()[1], [])

    def test_existing_ci_triggers_cover_full_workspace(self):
        text = (ROOT / ".github/workflows/homie-core-lint.yml").read_text()
        for event in ["push:", "pull_request:"]:
            block = text.split("  " + event, 1)[1].split("  workflow_dispatch:", 1)[0]
            if event == "push:":
                block = block.split("  pull_request:", 1)[0]
            patterns = [line.strip()[2:] for line in block.splitlines() if line.strip().startswith("- ")]
            for path in ["src/gateway/src/lib.rs", "src/core/src/lib.rs", "Cargo.toml", "Cargo.lock",
                         ".gitmodules", "src/infra/roci", "src/infra/homie-protocol/Cargo.toml",
                         ".cargo/config.toml", "rust-toolchain.toml"]:
                self.assertTrue(any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns), path)


if __name__ == "__main__":
    unittest.main()
