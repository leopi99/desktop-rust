#!/usr/bin/env python3
"""Prepare isolated offline repositories for manual Rust UI acceptance checks."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

SCENARIOS = ("changes", "branches-stash", "merge-conflict", "rebase-conflict")
ROOT = Path(__file__).resolve().parents[1]


def environment():
    env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    env.update(GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1",
               GIT_EDITOR="true", GIT_SEQUENCE_EDITOR="true", GIT_TERMINAL_PROMPT="0")
    return env


def git(repo, *args, expected=0):
    result = subprocess.run(
        ["git", "--literal-pathspecs", "--no-optional-locks", "-C", str(repo), *args],
        env=environment(), stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30,
        check=False,
    )
    if result.returncode != expected:
        raise RuntimeError(f"Git {args[0]} exited {result.returncode}: "
                           + result.stderr.decode("utf-8", errors="replace"))
    return result.stdout


def write(repo, name, text):
    (repo / name).write_text(text, encoding="utf-8")


def commit(repo, message):
    git(repo, "add", "--all")
    git(repo, "commit", "-m", message)


def lines(edits=()):
    return "".join(f"line {n:03}" + (" edited" if n in edits else "") + "\n"
                   for n in range(50))


def create_repository(repo):
    repo.mkdir()
    git(repo, "init", "--template=", "--initial-branch=main")
    for key, value in [
        ("user.name", "Rust UI Fixture"), ("user.email", "fixture@example.invalid"),
        ("commit.gpgsign", "false"), ("core.hooksPath", os.devnull),
        ("core.attributesFile", os.devnull), ("core.autocrlf", "false"),
        ("status.renames", "true"), ("rebase.backend", "merge"),
    ]:
        git(repo, "config", key, value)
    write(repo, "base.txt", "baseline\n")
    write(repo, "keep.txt", "Preserve this tracked file.\n")
    commit(repo, "Fixture baseline")


def snapshot(repo):
    files = {}
    for path in sorted(repo.rglob("*")):
        if path.is_file() and not path.is_relative_to(repo / ".git"):
            files[str(path.relative_to(repo))] = hashlib.sha256(path.read_bytes()).hexdigest()
    return {
        "head": git(repo, "rev-parse", "HEAD").decode().strip(),
        "refs": git(repo, "show-ref").decode().splitlines(),
        "status": git(repo, "status", "--porcelain=v1").decode().splitlines(),
        "unmerged": git(repo, "ls-files", "--unmerged").decode().splitlines(),
        "index_sha256": hashlib.sha256((repo / ".git/index").read_bytes()).hexdigest(),
        "config_sha256": hashlib.sha256((repo / ".git/config").read_bytes()).hexdigest(),
        "files_sha256": files,
    }


def prepare(output):
    if shutil.which("git") is None:
        raise RuntimeError("Git is required before creating fixtures")
    # Reserve a new root before any Git command; never reuse or clean user data.
    output.mkdir(exist_ok=False)
    initial = {}
    for scenario in SCENARIOS:
        repo = output / scenario
        create_repository(repo)
        before_operation = None
        if scenario == "changes":
            write(repo, "tracked.txt", lines())
            write(repo, "move-source.txt", lines())
            write(repo, "delete.txt", "first\nsecond\nthird\n")
            commit(repo, "Text fixtures")
            git(repo, "mv", "--", "move-source.txt", "renamed.txt")
            write(repo, "tracked.txt", lines((10, 35)))
            write(repo, "renamed.txt", lines((10, 35)))
            write(repo, "new.txt", "first\nsecond\nthird\n")
            (repo / "delete.txt").unlink()
        elif scenario == "branches-stash":
            git(repo, "switch", "-c", "feature")
            write(repo, "feature.txt", "Feature branch content.\n")
            commit(repo, "Feature commit")
            git(repo, "switch", "main")
            write(repo, "base.txt", "Saved tracked changes.\n")
            git(repo, "stash", "push", "-m", "Fixture saved changes")
        else:
            target = "feature" if scenario == "merge-conflict" else "upstream"
            git(repo, "switch", "-c", target)
            write(repo, "base.txt", "Target branch side.\n")
            commit(repo, "Target conflict change")
            git(repo, "switch", "main")
            write(repo, "base.txt", "Local branch side.\n")
            commit(repo, "Local conflict change")
            before_operation = snapshot(repo)
            if scenario == "merge-conflict":
                git(repo, "merge", "--no-edit", target, expected=1)
                git(repo, "rev-parse", "--verify", "MERGE_HEAD")
            else:
                git(repo, "rebase", target, expected=1)
                if not (repo / ".git/rebase-merge").is_dir():
                    raise RuntimeError("Expected a paused merge-backend rebase")
            if not git(repo, "ls-files", "--unmerged"):
                raise RuntimeError("Expected unmerged index entries")
        write(repo, "keep-untracked.txt", "Preserve this untracked user file.\n")
        initial[scenario] = {"initial": snapshot(repo), "before_operation": before_operation}
    manifest = {
        "schema": 1, "builder_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "interactive_checks": "not-run", "scenarios": initial,
    }
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Prepared {len(SCENARIOS)} offline fixtures in {output}; interactive checks not run.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True, help="Fresh root for prepare; existing root for inspect/launch")
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--inspect", choices=SCENARIOS, help="Print current repository evidence without a pass/fail verdict")
    mode.add_argument("--launch", choices=SCENARIOS, help="Open a fixture interactively; close the app to return")
    parser.add_argument("--binary", type=Path, default=ROOT / "target/debug/desktop-rust")
    args = parser.parse_args()
    # Keep an existing symlink visible to mkdir(exist_ok=False), including dangling ones.
    output = args.output.absolute()
    if args.inspect or args.launch:
        manifest = json.loads((output / "manifest.json").read_text())
        if manifest.get("schema") != 1:
            raise ValueError("Unsupported fixture manifest")
        repo = output / (args.inspect or args.launch)
        if args.inspect:
            print(json.dumps(snapshot(repo), indent=2))
        else:
            subprocess.run([str(args.binary.resolve(strict=True)), "--repository", str(repo)],
                           env=environment(), check=True)
    else:
        prepare(output)


if __name__ == "__main__":
    main()
