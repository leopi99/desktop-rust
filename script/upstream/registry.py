"""Validate the stable-release ledger and its human-reviewed checkpoints."""
import argparse
from datetime import datetime
import json
from pathlib import Path
import re

TAG = re.compile(r"(?:release-|v)?(\d+)\.(\d+)\.(\d+)")
SHA = re.compile(r"[0-9a-f]{40}")
STATUSES = {"pending-analysis", "pending-implementation", "deferred", "implemented", "not-applicable"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def nonempty(value):
    return isinstance(value, str) and bool(value.strip())


def timestamp(value):
    require(isinstance(value, str), "timestamp must be an ISO 8601 string")
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    require(parsed.tzinfo is not None, "timestamp must include a timezone")
    return parsed


def version(tag):
    require(isinstance(tag, str) and TAG.fullmatch(tag), "invalid stable release tag")
    return tuple(map(int, TAG.fullmatch(tag).groups()))


def release_key(release):
    return timestamp(release["published_at"]), version(release["tag"])


def validate_features(data):
    require(isinstance(data, dict) and type(data.get("schema_version")) is int and data.get("schema_version") == 1, "unsupported feature schema")
    require(isinstance(data.get("features"), list), "features must be an array")
    features = {}
    for feature in data["features"]:
        require(isinstance(feature, dict) and nonempty(feature.get("id")), "feature requires an id")
        require(feature["id"] not in features, "duplicate feature id")
        require(isinstance(feature.get("status"), str) and feature.get("status") in {"planned", "prototype", "supported"}, "invalid feature status")
        features[feature["id"]] = feature["status"]
    return features


def validate_state(state, feature_data):
    features = validate_features(feature_data)
    require(isinstance(state, dict) and type(state.get("schema_version")) is int and state.get("schema_version") == 1, "unsupported ledger schema")
    require(state.get("repository") == "desktop/desktop", "unexpected upstream repository")
    require(isinstance(state.get("releases"), list), "releases must be an array")
    require(isinstance(state.get("changes"), list), "changes must be an array")
    releases = {}
    for release in state["releases"]:
        require(isinstance(release, dict), "release must be an object")
        version(release.get("tag"))
        tag = release["tag"]
        require(tag not in releases, "duplicate release tag")
        require(isinstance(release.get("sha"), str) and SHA.fullmatch(release["sha"]), "invalid release SHA")
        timestamp(release.get("published_at"))
        require(release.get("url") == "https://github.com/desktop/desktop/releases/tag/" + tag, "invalid release URL")
        require(isinstance(release.get("status"), str) and release.get("status") in {"pending-analysis", "analyzed"}, "invalid release status")
        if release["status"] == "analyzed":
            require(nonempty(release.get("assessment")), "analyzed release requires an assessment, even with no changes")
        if "base" in release:
            base = release["base"]
            require(isinstance(base, dict), "comparison base must be an object")
            version(base.get("tag"))
            require(isinstance(base.get("sha"), str) and SHA.fullmatch(base["sha"]), "invalid comparison SHA")
        releases[tag] = release

    checkpoints = {}
    for name in ("detected", "analyzed", "parity"):
        require(name in state, "missing checkpoint: " + name)
        ref = state[name]
        if ref is None:
            checkpoints[name] = None
            continue
        require(isinstance(ref, dict) and isinstance(ref.get("tag"), str) and ref.get("tag") in releases, "unknown checkpoint release: " + name)
        release = releases[ref["tag"]]
        require(ref.get("sha") == release["sha"], "checkpoint SHA mismatch: " + name)
        checkpoints[name] = release
    if releases:
        require(checkpoints["detected"] == max(releases.values(), key=release_key), "detected must reference the last published registered release")
    else:
        require(all(ref is None for ref in checkpoints.values()), "empty ledger cannot have checkpoints")
    if checkpoints["analyzed"]:
        require(release_key(checkpoints["analyzed"]) <= release_key(checkpoints["detected"]), "analysis exceeds detection")
    if checkpoints["parity"]:
        require(checkpoints["analyzed"] is not None and release_key(checkpoints["parity"]) <= release_key(checkpoints["analyzed"]), "parity exceeds analysis")

    seen = set()
    for change in state["changes"]:
        require(isinstance(change, dict) and nonempty(change.get("id")), "change requires an id")
        require(change["id"] not in seen, "duplicate change id")
        seen.add(change["id"])
        require(isinstance(change.get("release"), str) and change.get("release") in releases, "change references an unknown release")
        require(isinstance(change.get("feature"), str) and change.get("feature") in features, "change references an unknown feature")
        require(isinstance(change.get("status"), str) and change.get("status") in STATUSES, "invalid change status")
        sources = change.get("upstream")
        require(isinstance(sources, list) and bool(sources), "change requires upstream references")
        require(all(isinstance(source, str) and (SHA.fullmatch(source) or re.fullmatch(
            r"https://github.com/desktop/desktop/(?:pull/\d+|commit/[0-9a-f]{40})", source)) for source in sources), "invalid upstream reference")
        if change["status"] == "implemented":
            require(isinstance(change.get("rust_pr"), str) and re.fullmatch(
                r"https://github.com/leopi99/desktop-rust/pull/\d+", change["rust_pr"]), "implemented change requires a Rust PR")
            evidence = change.get("evidence")
            require(isinstance(evidence, list) and bool(evidence) and all(nonempty(item) for item in evidence), "implemented change requires test evidence")
        if change["status"] in {"not-applicable", "deferred"}:
            require(nonempty(change.get("reason")), "deferred/non-applicable change requires a reason")
        release = releases[change["release"]]
        if release["status"] == "analyzed":
            require(change["status"] != "pending-analysis", "analyzed release still has unclassified changes")
        parity = checkpoints["parity"]
        if parity and release_key(release) <= release_key(parity) and features[change["feature"]] != "planned":
            require(change["status"] in {"implemented", "not-applicable"}, "parity includes unresolved changes for an active feature")
    for release in releases.values():
        if checkpoints["analyzed"] and release_key(release) <= release_key(checkpoints["analyzed"]):
            require(release["status"] == "analyzed", "analysis checkpoint skips an unclassified release")
    return state


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--state", type=Path, default=Path("upstream/state.json"))
    parser.add_argument("--features", type=Path, default=Path("upstream/features.json"))
    args = parser.parse_args()
    validate_state(json.loads(args.state.read_text()), json.loads(args.features.read_text()))
    print("Upstream ledger and checkpoints are valid")


if __name__ == "__main__":
    main()
