"""Run the required Rust policy, lint, test and rustdoc gates from any directory."""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import tomllib

from check_rust_policy import cargo_binary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true", help="Use already cached Cargo dependencies")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    offline = ["--offline"] if args.offline else []
    channel = tomllib.loads((root / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    cargo = [cargo_binary(), "+" + channel]
    commands = [
        [sys.executable, "script/check_rust_policy.py", *offline],
        [sys.executable, "-m", "unittest", "discover", "-s", "script", "-p", "test_rust_policy.py", "-v"],
        [*cargo, "fmt", "--all", "--", "--check"],
        [*cargo, "clippy", "--locked", *offline, "--workspace", "--all-targets", "--all-features", "--", "-D", "warnings"],
        [*cargo, "test", "--locked", *offline, "--workspace", "--all-features"],
        [*cargo, "doc", "--locked", *offline, "--workspace", "--all-features", "--no-deps"],
    ]
    env = {**os.environ, "RUSTDOCFLAGS": "-D warnings"}
    for command in commands:
        print("Running: " + " ".join(command), flush=True)
        subprocess.run(command, cwd=root, env=env, check=True)


if __name__ == "__main__":
    main()
