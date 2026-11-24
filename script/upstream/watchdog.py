"""Read-only Actions watchdog. Failure is surfaced as a workflow failure."""
import argparse
from datetime import datetime, timedelta, timezone
import os
import re
from urllib.parse import urlencode

from registry import require, timestamp
from sync import request_json


def check_freshness(runs, now):
    require(isinstance(runs, list), "invalid Actions runs response")
    successes = [r for r in runs if r.get("status") == "completed" and r.get("conclusion") == "success"]
    require(bool(successes), "no successful upstream discovery run on the default branch")
    latest = max(timestamp(r["created_at"]) for r in successes)
    require(latest <= now + timedelta(minutes=5), "Actions run timestamp is in the future")
    require(now - latest <= timedelta(days=8), "last successful upstream discovery is older than eight days")
    return latest


def inspect(repository, branch, token=None, now=None):
    require(isinstance(repository, str) and re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository), "invalid repository")
    require(isinstance(branch, str) and bool(branch), "missing default branch")
    root = f"https://api.github.com/repos/{repository}/actions/workflows/upstream-stable.yml"
    workflow = request_json(root, token)
    require(workflow.get("state") == "active", "upstream discovery workflow is disabled")
    query = urlencode({"branch": branch, "status": "success", "per_page": 100})
    data = request_json(root + "/runs?" + query, token)
    return check_freshness(data.get("workflow_runs"), now or datetime.now(timezone.utc))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY"))
    parser.add_argument("--branch", default=os.environ.get("DEFAULT_BRANCH"))
    args = parser.parse_args()
    latest = inspect(args.repository, args.branch, os.environ.get("GH_TOKEN"))
    print("Last successful upstream discovery:", latest.isoformat())


if __name__ == "__main__":
    main()
