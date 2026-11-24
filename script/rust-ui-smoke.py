#!/usr/bin/env python3
"""Capture each Rust UI panel using a disposable, offline Git repository."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/desktop-rust"))
    parser.add_argument("--output", type=Path, required=True, help="Fresh output directory")
    parser.add_argument("--size", default="1200x800")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    env["GIT_CONFIG_NOSYSTEM"] = "1"
    env["GIT_CONFIG_GLOBAL"] = os.devnull
    env.setdefault("ICED_BACKEND", "tiny-skia")
    with tempfile.TemporaryDirectory(prefix="desktop-rust-ui-") as temporary:
        repo = Path(temporary) / "sample-project"
        repo.mkdir()

        def git(*arguments):
            return subprocess.check_output(
                ["git", "--no-optional-locks", "-C", str(repo), *arguments],
                env=env, stderr=subprocess.STDOUT,
            )

        git("init", "--initial-branch=main")
        git("config", "user.name", "UI Fixture")
        git("config", "user.email", "fixture@example.invalid")
        git("config", "commit.gpgsign", "false")
        git("config", "core.hooksPath", os.devnull)
        (repo / "README.md").write_text("# Sample project\n\nA local Git fixture for UI checks.\n")
        (repo / "src").mkdir()
        source = repo / "src" / "main.rs"
        source.write_text('fn main() {\n    println!("Hello, world!");\n}\n')
        git("add", "--all")
        git("commit", "-m", "Create sample project")
        git("branch", "feature/navigation")
        source.write_text('fn main() {\n    println!("Hello, Rust!");\n}\n')
        git("commit", "--all", "-m", "Update greeting")
        (repo / "README.md").write_text("# Sample project\n\nSaved documentation draft.\n")
        git("stash", "push", "-m", "Documentation draft")
        git("remote", "add", "origin", "https://example.invalid/sample/project.git")
        git("config", "branch.main.remote", "origin")
        git("config", "branch.main.merge", "refs/heads/main")
        source.write_text('fn main() {\n    println!("Hello from Desktop Rust!");\n}\n')
        (repo / "notes.txt").write_text("Review the navigation layout.\n")
        def signature():
            files = {str(path.relative_to(repo)): path.read_bytes()
                     for path in repo.rglob("*")
                     if path.is_file() and not path.is_relative_to(repo / ".git")}
            return (git("status", "--porcelain=v1"), git("show-ref"),
                    (repo / ".git/index").read_bytes(), (repo / ".git/config").read_bytes(), files)

        before = signature()
        panels = ["changes", "history", "branches", "remotes", "stash", "repository", "git", "branch-delete-review"]
        for panel in panels:
            subprocess.run(
                [str(binary), "--repository", str(repo), "--panel", panel,
                 "--size", args.size, "--capture", str(output / f"{panel}.png")],
                env=env, check=True, timeout=40,
            )
        # With no repository open, exercise the missing-Git diagnostic in isolation.
        subprocess.run(
            [str(binary), "--panel", "git", "--size", args.size,
             "--capture", str(output / "git-missing.png")],
            env={**env, "PATH": str(output / "missing-tools")}, check=True, timeout=40,
        )
        after = signature()
        if before != after:
            raise RuntimeError("UI capture changed fixture files, configuration, status, refs, or index")
    (output / "report.json").write_text(json.dumps({
        "size": args.size, "panels": panels, "diagnostic_cases": ["git-missing"],
        "fixture_unchanged": True,
        "note": "Rendered captures require human visual review; this is not an interaction or accessibility test.",
    }, indent=2) + "\n")
    print(f"Captured {len(panels)} panels plus missing-Git diagnostics in {output}; fixture unchanged")


if __name__ == "__main__":
    main()
