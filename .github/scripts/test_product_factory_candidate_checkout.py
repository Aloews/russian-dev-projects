from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / ".github" / "scripts" / "checkout_candidate_base.sh"


def git(*args: str, cwd: Path | None = None) -> str:
    result = subprocess.run(
        ["git", *args], cwd=cwd, check=True, text=True,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    return result.stdout.strip()


class CandidateBaseCheckoutTests(unittest.TestCase):
    def test_fetches_exact_candidate_commit_missing_from_shallow_base_clone(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            remote = root / "remote.git"
            source = root / "source"
            target = root / "target"
            git("init", "--bare", "--initial-branch=main", str(remote))
            git("init", "--initial-branch=main", str(source))
            git("config", "user.name", "CI Test", cwd=source)
            git("config", "user.email", "ci-test@example.invalid", cwd=source)
            (source / "base.txt").write_text("base\\n", encoding="utf-8")
            git("add", "base.txt", cwd=source)
            git("commit", "-m", "base", cwd=source)
            git("remote", "add", "origin", str(remote), cwd=source)
            git("push", "origin", "main", cwd=source)
            base_sha = git("rev-parse", "HEAD", cwd=source)
            git("checkout", "-b", "candidate", cwd=source)
            (source / "candidate.txt").write_text("candidate\\n", encoding="utf-8")
            git("add", "candidate.txt", cwd=source)
            git("commit", "-m", "candidate", cwd=source)
            candidate_sha = git("rev-parse", "HEAD", cwd=source)
            git("push", "origin", "candidate", cwd=source)

            git("clone", "--depth=1", "--single-branch", "--branch", "main", f"file://{remote}", str(target))
            self.assertEqual(git("rev-parse", "HEAD", cwd=target), base_sha)
            with self.assertRaises(subprocess.CalledProcessError):
                git("cat-file", "-e", f"{candidate_sha}^{{commit}}", cwd=target)

            env = os.environ.copy()
            env["PRODUCT_FACTORY_GITHUB_TOKEN"] = "test-only-token"
            result = subprocess.run(
                ["bash", str(SCRIPT), str(target), candidate_sha],
                check=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                env=env,
            )
            self.assertEqual(git("rev-parse", "HEAD", cwd=target), candidate_sha)
            self.assertEqual((target / "candidate.txt").read_text(encoding="utf-8"), "candidate\\n")
            self.assertNotIn("extraheader", git("config", "--local", "--list", cwd=target))
            self.assertIn(candidate_sha, result.stdout + result.stderr)

    def test_reuses_base_commit_already_present_in_shallow_checkout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            remote = root / "remote.git"
            source = root / "source"
            target = root / "target"
            wrapper_bin = root / "bin"
            wrapper_bin.mkdir()
            git("init", "--bare", "--initial-branch=main", str(remote))
            git("init", "--initial-branch=main", str(source))
            git("config", "user.name", "CI Test", cwd=source)
            git("config", "user.email", "ci-test@example.invalid", cwd=source)
            (source / "base.txt").write_text("base", encoding="utf-8")
            git("add", "base.txt", cwd=source)
            git("commit", "-m", "base", cwd=source)
            git("remote", "add", "origin", str(remote), cwd=source)
            git("push", "origin", "main", cwd=source)
            base_sha = git("rev-parse", "HEAD", cwd=source)
            git("clone", "--depth=1", "--single-branch", "--branch", "main", f"file://{remote}", str(target))

            real_git = subprocess.check_output(["which", "git"], text=True).strip()
            git_wrapper = wrapper_bin / "git"
            git_wrapper.write_text(
                f"""#!/usr/bin/env bash
if [[ "$1" == "-C" && "$3" == "fetch" ]]; then
  for arg in "$@"; do
    if [[ "$arg" == "{base_sha}" ]]; then
      echo 'unexpected redundant fetch-by-SHA' >&2
      exit 97
    fi
  done
fi
exec "{real_git}" "$@"
""",
                encoding="utf-8",
            )
            git_wrapper.chmod(0o755)
            env = os.environ.copy()
            env["PATH"] = str(wrapper_bin) + os.pathsep + env["PATH"]
            result = subprocess.run(
                ["bash", str(SCRIPT), str(target), base_sha],
                check=True, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                env=env,
            )
            self.assertEqual(git("rev-parse", "HEAD", cwd=target), base_sha)
            self.assertIn(base_sha, result.stdout + result.stderr)
            self.assertNotIn("unexpected redundant fetch-by-SHA", result.stderr)

    def test_rejects_malformed_sha_without_changing_checkout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            remote = root / "remote.git"
            source = root / "source"
            target = root / "target"
            git("init", "--bare", "--initial-branch=main", str(remote))
            git("init", "--initial-branch=main", str(source))
            git("config", "user.name", "CI Test", cwd=source)
            git("config", "user.email", "ci-test@example.invalid", cwd=source)
            (source / "base.txt").write_text("base\\n", encoding="utf-8")
            git("add", "base.txt", cwd=source)
            git("commit", "-m", "base", cwd=source)
            git("remote", "add", "origin", str(remote), cwd=source)
            git("push", "origin", "main", cwd=source)
            git("clone", "--depth=1", "--single-branch", "--branch", "main", f"file://{remote}", str(target))
            original = git("rev-parse", "HEAD", cwd=target)
            result = subprocess.run(
                ["bash", str(SCRIPT), str(target), "not-a-sha"],
                text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(git("rev-parse", "HEAD", cwd=target), original)


if __name__ == "__main__":
    unittest.main()