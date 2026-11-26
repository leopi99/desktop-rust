"""Verify that every Cargo workspace member inherits the Rust quality policy."""
import argparse
import json
import os
import shutil
from pathlib import Path
import re
import subprocess
import tomllib


def cargo_binary():
    """Find Cargo on PATH or in the standard rustup installation directory."""
    found = shutil.which("cargo")
    if found:
        return found
    home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    candidate = home / "bin" / ("cargo.exe" if os.name == "nt" else "cargo")
    if candidate.is_file():
        return str(candidate)
    raise SystemExit("Cargo is unavailable. Install Rust with rustup or add Cargo to PATH.")


def validate_policy(root, metadata):
    """Return policy violations from manifests and Cargo's actual member list."""
    root = Path(root).resolve()
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    toolchain = tomllib.loads((root / "rust-toolchain.toml").read_text())["toolchain"]
    failures = []
    channel = toolchain.get("channel", "")
    if not re.fullmatch(r"\d+\.\d+\.\d+", channel):
        failures.append("rust-toolchain.toml must pin an exact stable release")
    package = manifest.get("workspace", {}).get("package", {})
    minimum = package.get("rust-version", "")
    if not re.fullmatch(r"\d+\.\d+(?:\.\d+)?", minimum):
        failures.append("workspace.package.rust-version must be numeric")
    elif re.fullmatch(r"\d+\.\d+\.\d+", channel):
        if tuple(map(int, (minimum + ".0").split(".")[:3])) != tuple(map(int, channel.split("."))):
            failures.append("the minimum Rust version must match the tested toolchain")
    if not {"rustfmt", "clippy"}.issubset(toolchain.get("components", [])):
        failures.append("the pinned toolchain must include rustfmt and clippy")
    if not {"rust", "clippy"}.issubset(manifest.get("workspace", {}).get("lints", {})):
        failures.append("workspace rust and clippy lint tables are required")
    packages = {item["id"]: item for item in metadata["packages"]}
    registered = {Path(packages[member]["manifest_path"]).resolve() for member in metadata["workspace_members"]}
    for path in (root / "crates").rglob("Cargo.toml"):
        if "target" not in path.relative_to(root).parts and path.resolve() not in registered:
            failures.append(f"{path.relative_to(root)}: crate is not a workspace member")
    for member in metadata["workspace_members"]:
        path = Path(packages[member]["manifest_path"]).resolve()
        if not path.is_relative_to(root):
            failures.append(f"workspace member is outside the repository: {path.name}")
            continue
        local = tomllib.loads(path.read_text())
        label = path.relative_to(root)
        if local.get("lints") != {"workspace": True}:
            failures.append(f"{label}: inherit workspace lints without local overrides")
        for key in ("edition", "license", "version", "rust-version"):
            if local.get("package", {}).get(key) != {"workspace": True}:
                failures.append(f"{label}: package.{key} must inherit the workspace value")
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--offline", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    channel = tomllib.loads((root / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    command = [cargo_binary(), "+" + channel, "metadata", "--locked", "--no-deps", "--format-version", "1"]
    if args.offline:
        command.append("--offline")
    metadata = json.loads(subprocess.check_output(command, cwd=root, text=True))
    failures = validate_policy(root, metadata)
    if failures:
        raise SystemExit("Rust policy violations:\n- " + "\n- ".join(failures))
    print(f"Rust policy inherited by all {len(metadata['workspace_members'])} workspace members")


if __name__ == "__main__":
    main()
