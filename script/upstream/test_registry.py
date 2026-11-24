import copy
import unittest

from registry import validate_state
from sync import propose
from test_sync import features, initial, release, sha


def reviewed():
    state, _ = propose(initial(), [release("v1.0.0")], sha)
    state["releases"][0].update(status="analyzed", assessment="Reviewed source, tests and dependencies")
    state["analyzed"] = state["detected"].copy()
    return state


def change(**kwargs):
    result = dict(id="fix-status", release="v1.0.0", feature="git", status="pending-implementation",
                  upstream=["https://github.com/desktop/desktop/pull/123"])
    result.update(kwargs)
    return result


class RegistryTests(unittest.TestCase):
    def test_empty_ledger_is_valid(self):
        validate_state(initial(), features())

    def test_missing_evidence_or_rust_pr_blocks_implemented_state(self):
        for extra in ({}, {"rust_pr": "https://github.com/leopi99/desktop-rust/pull/1"},
                      {"evidence": ["cargo test"]}):
            state = reviewed()
            state["changes"] = [change(status="implemented", **extra)]
            with self.subTest(extra=extra), self.assertRaises(ValueError):
                validate_state(state, features())

    def test_parity_requires_analysis_and_resolution_of_active_features(self):
        state = reviewed()
        state["changes"] = [change()]
        state["parity"] = state["detected"].copy()
        with self.assertRaises(ValueError):
            validate_state(state, features())
        state["changes"][0].update(status="implemented", rust_pr="https://github.com/leopi99/desktop-rust/pull/1", evidence=["cargo test status_case"])
        validate_state(state, features())
        state["analyzed"] = None
        with self.assertRaises(ValueError):
            validate_state(state, features())

    def test_deferred_planned_feature_does_not_claim_full_application_parity(self):
        state = reviewed()
        state["changes"] = [change(feature="future", status="deferred", reason="Not yet implemented")]
        state["parity"] = state["detected"].copy()
        validate_state(state, features())

    def test_reason_required_for_deferral_and_non_applicability(self):
        for status in ("deferred", "not-applicable"):
            state = reviewed()
            state["changes"] = [change(status=status)]
            with self.assertRaises(ValueError):
                validate_state(state, features())
            state["changes"][0]["reason"] = "Electron-only update"
            validate_state(state, features())

    def test_analysis_cannot_skip_unclassified_release_or_change(self):
        state = reviewed()
        state["changes"] = [change(status="pending-analysis")]
        with self.assertRaises(ValueError):
            validate_state(state, features())
        state["changes"] = []
        state["releases"][0]["status"] = "pending-analysis"
        with self.assertRaises(ValueError):
            validate_state(state, features())

    def test_analyzed_release_requires_human_assessment(self):
        state = reviewed()
        del state["releases"][0]["assessment"]
        with self.assertRaises(ValueError):
            validate_state(state, features())

    def test_duplicate_ids_unknown_features_and_wrong_sha_are_rejected(self):
        for corrupt in (
            lambda s: s.update(changes=[change(), change()]),
            lambda s: s.update(changes=[change(feature="missing")]),
            lambda s: s["detected"].update(sha="0" * 40),
            lambda s: s["releases"].append(copy.deepcopy(s["releases"][0])),
            lambda s: s.update(repository="other/repo"),
            lambda s: s.update(schema_version=True),
        ):
            state = reviewed()
            corrupt(state)
            with self.subTest(corrupt=corrupt), self.assertRaises(ValueError):
                validate_state(state, features())


if __name__ == "__main__":
    unittest.main()
