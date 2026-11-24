"""Propose stable-release evidence atomically; never advance analysis or parity."""
import argparse
import copy
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import tempfile
import urllib.request

from registry import SHA, TAG, release_key, require, timestamp, validate_state, version

API = "https://api.github.com/repos/desktop/desktop"
UPSTREAM = "https://github.com/desktop/desktop.git"


def request_json(url, token=None):
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "desktop-rust-sync"}
    if token:
        headers["Authorization"] = "Bearer " + token
    with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=30) as response:
        return json.load(response)


def api(route):
    return request_json(API + route, os.environ.get("GH_TOKEN"))


def discover():
    releases = []
    page = 1
    while True:
        batch = api(f"/releases?per_page=100&page={page}")
        require(isinstance(batch, list), "invalid releases API response")
        if not batch:
            break
        releases.extend(batch)
        page += 1
    return releases


def stable(releases):
    candidates = []
    for release in releases:
        require(isinstance(release, dict), "invalid release API item")
        require(isinstance(release.get("draft"), bool) and isinstance(release.get("prerelease"), bool), "invalid release flags")
        tag = release.get("tag_name")
        require(isinstance(tag, str), "missing release tag")
        if not release["draft"] and not release["prerelease"] and TAG.fullmatch(tag):
            timestamp(release.get("published_at"))
            require(release.get("html_url") == "https://github.com/desktop/desktop/releases/tag/" + tag, "invalid upstream release URL")
            candidates.append(release)
    require(bool(candidates), "upstream returned no stable releases; refusing a healthy empty report")
    require(len({r["tag_name"] for r in candidates}) == len(candidates), "duplicate upstream release tag")
    return sorted(candidates, key=lambda r: (timestamp(r["published_at"]), version(r["tag_name"])))


def propose(state, releases, resolve):
    result = copy.deepcopy(state)
    known = {r["tag"]: r for r in result["releases"]}
    candidates = stable(releases)
    by_tag = {r["tag_name"]: r for r in candidates}
    require(set(known).issubset(by_tag), "a registered release disappeared or became a prerelease; investigate manually")
    floor = min((release_key(r) for r in known.values()), default=None)
    selected = ([r for r in candidates if r["tag_name"] in known or (timestamp(r["published_at"]), version(r["tag_name"])) >= floor]
                if known else candidates[-1:])
    added = []
    resolved = {}

    def commit(tag):
        if tag not in resolved:
            resolved[tag] = resolve(tag)
        require(isinstance(resolved[tag], str) and SHA.fullmatch(resolved[tag]), "invalid resolved SHA")
        return resolved[tag]

    for release in selected:
        tag = release["tag_name"]
        sha = commit(tag)
        if tag in known:
            require(known[tag]["sha"] == sha, f"Tag changed SHA: {tag}; manual investigation required")
            continue
        item = {"tag": tag, "sha": sha, "url": release["html_url"],
                "published_at": release["published_at"], "status": "pending-analysis"}
        # Compare backports with their own release line rather than the last report.
        if known:
            previous = [r for r in candidates if version(r["tag_name"]) < version(tag)
                        and timestamp(r["published_at"]) <= timestamp(release["published_at"])]
            same_line = [r for r in previous if version(r["tag_name"])[:2] == version(tag)[:2]]
            if previous:
                base = max(same_line or previous, key=lambda r: version(r["tag_name"]))
                item["base"] = {"tag": base["tag_name"], "sha": commit(base["tag_name"])}
        result["releases"].append(item)
        added.append(item)
    latest = max(result["releases"], key=release_key)
    result["detected"] = {"tag": latest["tag"], "sha": latest["sha"]}
    return result, added


class GitSource:
    def __init__(self, directory):
        self.directory = directory
        self.directory.mkdir(parents=True, exist_ok=True)
        self.git("init", "--bare", "--quiet")

    def git(self, *args):
        env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM="1", GIT_TERMINAL_PROMPT="0")
        return subprocess.check_output(["git", "-C", str(self.directory), "-c", "protocol.ext.allow=never", *args],
                                       env=env, timeout=120)

    def resolve(self, tag):
        require(isinstance(tag, str) and TAG.fullmatch(tag), "invalid tag requested from Git")
        self.git("fetch", "--quiet", "--no-tags", UPSTREAM, "refs/tags/" + tag)
        return self.git("rev-parse", "FETCH_HEAD^{commit}").decode().strip()


def report(added, source):
    files = {}
    lines = ["# Stable upstream discovery", "", "Analysis and Rust parity have NOT advanced.", ""]
    for release in added:
        tag = release["tag"]
        lines += [f"## {tag}", release["url"], f"SHA: {release['sha']}", ""]
        base = release.get("base")
        if base:
            patch = f"{tag}.patch"
            commits = f"{tag}-commits.txt"
            files[patch] = source.git("diff", "--no-ext-diff", "--no-textconv", "--binary", base["sha"], release["sha"], "--")
            files[commits] = source.git("log", "--format=%H %s", base["sha"] + ".." + release["sha"], "--")
            lines += [f"Compared with {base['tag']}; [source diff]({patch}), [commits]({commits}).", ""]
        else:
            lines += ["Initial baseline: inventory this stable release before claiming parity.", ""]
    if not added:
        lines += ["No new stable releases."]
    files["report.md"] = ("\n".join(lines) + "\n").encode()
    return files


def run(state_path, features_path, output, source_factory=GitSource):
    require(not output.exists(), "output directory already exists; use a fresh path")
    state = json.loads(state_path.read_text())
    features = json.loads(features_path.read_text())
    validate_state(state, features)
    releases = discover()
    with tempfile.TemporaryDirectory(prefix="desktop-rust-upstream-git-") as git_dir:
        source = source_factory(Path(git_dir))
        proposed, added = propose(state, releases, source.resolve)
        validate_state(proposed, features)
        files = report(added, source)
    files["state.json"] = (json.dumps(proposed, indent=2) + "\n").encode()
    files["check.json"] = (json.dumps({"checked_at": datetime.now(timezone.utc).isoformat(),
        "new_releases": len(added), "detected": proposed["detected"]}, indent=2) + "\n").encode()
    # Publish only after every API request, Git diff and validation has succeeded.
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".upstream-report-", dir=output.parent) as staging:
        for name, data in files.items():
            (Path(staging) / name).write_bytes(data)
        os.rename(staging, output)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--state", type=Path, default=Path("upstream/state.json"))
    parser.add_argument("--features", type=Path, default=Path("upstream/features.json"))
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    run(args.state, args.features, args.output)


if __name__ == "__main__":
    main()
