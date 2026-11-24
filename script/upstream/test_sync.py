import copy
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import sync
from registry import validate_state


def sha(tag):
    return hashlib.sha1(tag.encode()).hexdigest()


def release(tag, **kwargs):
    return dict(tag_name=tag, draft=False, prerelease=False,
                html_url="https://github.com/desktop/desktop/releases/tag/" + tag,
                published_at="2026-01-01T00:00:00Z", **kwargs)


def initial():
    return dict(schema_version=1, repository="desktop/desktop", releases=[], changes=[],
                detected=None, analyzed=None, parity=None)


def features():
    return {"schema_version": 1, "features": [
        {"id": "git", "status": "prototype"}, {"id": "future", "status": "planned"}]}


class DiscoveryTests(unittest.TestCase):
    def test_bootstrap_latest_stable_without_claiming_parity(self):
        state, added = sync.propose(initial(), [release("v1.0.0"), release("v1.1.0")], sha)
        self.assertEqual([r["tag"] for r in added], ["v1.1.0"])
        self.assertIsNone(state["parity"])
        self.assertIsNone(state["analyzed"])
        validate_state(state, features())

    def test_catch_up_and_idempotence(self):
        state, _ = sync.propose(initial(), [release("v1.0.0")], sha)
        releases = [release("v1.2.0"), release("v1.0.0"), release("v1.1.0")]
        updated, added = sync.propose(state, releases, sha)
        self.assertEqual(len(added), 2)
        self.assertEqual(len(state["releases"]), 1)
        self.assertEqual(sync.propose(updated, releases, sha), (updated, []))
        self.assertEqual(added[1]["base"]["tag"], "v1.1.0")

    def test_bootstrap_does_not_reimport_older_publications(self):
        old = release("v1.0.0")
        new = dict(release("v1.1.0"), published_at="2026-02-01T00:00:00Z")
        state, _ = sync.propose(initial(), [old, new], sha)
        self.assertEqual(sync.propose(state, [old, new], sha), (state, []))

    def test_equal_publication_times_do_not_reimport_pre_baseline_releases(self):
        releases = [release("v1.0.0"), release("v1.1.0")]
        state, _ = sync.propose(initial(), releases, sha)
        self.assertEqual(sync.propose(state, releases, sha), (state, []))

    def test_official_release_tag_format(self):
        state, _ = sync.propose(initial(), [release("release-3.6.11")], sha)
        self.assertEqual(state["detected"]["tag"], "release-3.6.11")

    def test_moved_or_disappeared_tag_fails_without_mutation(self):
        state, _ = sync.propose(initial(), [release("v1.0.0")], sha)
        original = copy.deepcopy(state)
        with self.assertRaises(ValueError):
            sync.propose(state, [release("v1.0.0")], lambda t: "0" * 40)
        with self.assertRaises(ValueError):
            sync.propose(state, [release("v1.1.0")], sha)
        self.assertEqual(state, original)

    def test_ignore_prerelease_and_draft(self):
        releases = [release("v1.0.0"), dict(release("v2.0.0"), prerelease=True),
                    dict(release("v3.0.0"), draft=True)]
        state, _ = sync.propose(initial(), releases, sha)
        self.assertEqual(state["detected"]["tag"], "v1.0.0")

    def test_new_backport_compares_to_its_release_line(self):
        old = release("v1.0.0")
        main = dict(release("v2.0.0"), published_at="2026-02-01T00:00:00Z")
        backport = dict(release("v1.0.1"), published_at="2026-03-01T00:00:00Z")
        state, _ = sync.propose(initial(), [old, main], sha)
        updated, added = sync.propose(state, [old, main, backport], sha)
        self.assertEqual(added[0]["base"]["tag"], "v1.0.0")
        self.assertEqual(updated["detected"]["tag"], "v1.0.1")
        validate_state(updated, features())

    def test_empty_duplicate_or_malformed_release_response_fails(self):
        for releases in ([], [release("v1.0.0")] * 2, [{}]):
            with self.subTest(releases=releases), self.assertRaises(ValueError):
                sync.propose(initial(), releases, sha)

    def test_api_pagination_and_errors(self):
        with patch("sync.api", side_effect=[[release("v1.0.0")], [release("v1.1.0")], []]) as api:
            self.assertEqual(len(sync.discover()), 2)
            self.assertIn("page=3", api.call_args.args[0])
        with patch("sync.api", side_effect=OSError("API unavailable")):
            with self.assertRaises(OSError):
                sync.discover()
        with patch("sync.api", return_value={"message": "rate limit"}):
            with self.assertRaises(ValueError):
                sync.discover()


class FakeSource:
    def __init__(self, directory):
        pass

    def resolve(self, tag):
        return sha(tag)

    def git(self, *args):
        return b"source evidence\n"


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.state = self.root / "state.json"
        self.features = self.root / "features.json"
        self.state.write_text(json.dumps(initial()))
        self.features.write_text(json.dumps(features()))
        self.output = self.root / "report"

    def test_success_publishes_complete_proposal_without_changing_ledger(self):
        before = self.state.read_bytes()
        with patch("sync.discover", return_value=[release("v1.0.0")]):
            sync.run(self.state, self.features, self.output, FakeSource)
        self.assertEqual(self.state.read_bytes(), before)
        self.assertTrue((self.output / "report.md").is_file())
        self.assertTrue((self.output / "state.json").is_file())
        self.assertTrue((self.output / "check.json").is_file())

    def test_api_failure_leaves_no_output_or_checkpoint_change(self):
        before = self.state.read_bytes()
        with patch("sync.discover", side_effect=OSError("network failure")):
            with self.assertRaises(OSError):
                sync.run(self.state, self.features, self.output, FakeSource)
        self.assertFalse(self.output.exists())
        self.assertEqual(self.state.read_bytes(), before)

    def test_git_diff_failure_does_not_publish_partial_state(self):
        baseline, _ = sync.propose(initial(), [release("v1.0.0")], sha)
        self.state.write_text(json.dumps(baseline))
        with patch("sync.discover", return_value=[release("v1.0.0"), release("v1.1.0")]), \
             patch.object(FakeSource, "git", side_effect=subprocess.CalledProcessError(1, "git")):
            with self.assertRaises(subprocess.CalledProcessError):
                sync.run(self.state, self.features, self.output, FakeSource)
        self.assertFalse(self.output.exists())
        self.assertEqual(json.loads(self.state.read_text()), baseline)

    def test_git_resolution_failure_leaves_no_partial_output(self):
        with patch("sync.discover", return_value=[release("v1.0.0")]), \
             patch.object(FakeSource, "resolve", side_effect=subprocess.TimeoutExpired("git", 120)):
            with self.assertRaises(subprocess.TimeoutExpired):
                sync.run(self.state, self.features, self.output, FakeSource)
        self.assertFalse(self.output.exists())
        self.assertIsNone(json.loads(self.state.read_text())["detected"])

    def test_no_new_release_still_produces_a_success_record(self):
        releases = [release("v1.0.0")]
        baseline, _ = sync.propose(initial(), releases, sha)
        self.state.write_text(json.dumps(baseline))
        with patch("sync.discover", return_value=releases):
            sync.run(self.state, self.features, self.output, FakeSource)
        self.assertEqual(json.loads((self.output / "state.json").read_text()), baseline)
        self.assertEqual(json.loads((self.output / "check.json").read_text())["new_releases"], 0)

    def test_existing_output_is_never_overwritten(self):
        self.output.mkdir()
        sentinel = self.output / "report.md"
        sentinel.write_text("previous report")
        with self.assertRaises(ValueError):
            sync.run(self.state, self.features, self.output, FakeSource)
        self.assertEqual(sentinel.read_text(), "previous report")

    def test_real_git_patch_and_commit_list_cover_the_release_interval(self):
        upstream = self.root / "upstream"
        upstream.mkdir()
        def git(*args):
            return subprocess.check_output(["git", "-C", str(upstream), *args], stderr=subprocess.PIPE)
        git("init", "--quiet")
        git("config", "user.email", "test@example.invalid")
        git("config", "user.name", "Test")
        git("config", "commit.gpgsign", "false")
        git("config", "core.hooksPath", "/dev/null")
        (upstream / "file.txt").write_text("before\n")
        git("add", "file.txt")
        git("commit", "--quiet", "-m", "baseline")
        git("tag", "v1.0.0")
        (upstream / "file.txt").write_text("after\n")
        git("commit", "--quiet", "-am", "behavior fix")
        git("tag", "v1.1.0")
        def resolve(tag):
            return git("rev-parse", tag + "^{commit}").decode().strip()
        baseline, _ = sync.propose(initial(), [release("v1.0.0")], resolve)
        self.state.write_text(json.dumps(baseline))
        class LocalSource(sync.GitSource):
            def resolve(self, tag):
                self.git("fetch", "--quiet", "--no-tags", str(upstream), "refs/tags/" + tag)
                return self.git("rev-parse", "FETCH_HEAD^{commit}").decode().strip()
        with patch("sync.discover", return_value=[release("v1.0.0"), release("v1.1.0")]):
            sync.run(self.state, self.features, self.output, LocalSource)
        self.assertIn("+after", (self.output / "v1.1.0.patch").read_text())
        self.assertIn("-before", (self.output / "v1.1.0.patch").read_text())
        commits = (self.output / "v1.1.0-commits.txt").read_text()
        self.assertIn("behavior fix", commits)
        self.assertNotIn("baseline", commits)

    def test_real_git_source_uses_an_isolated_bare_database(self):
        source = sync.GitSource(self.root / "objects")
        self.assertEqual(source.git("rev-parse", "--is-bare-repository").strip(), b"true")
        self.assertFalse((self.root / "FETCH_HEAD").exists())
        with self.assertRaises(ValueError):
            source.resolve("--malicious")


if __name__ == "__main__":
    unittest.main()
