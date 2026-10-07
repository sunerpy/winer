#!/usr/bin/env python3
"""Decide whether a CI run is a release-please version bump that may skip the build and test jobs.

A release PR, and the squash commit it lands as on main, changes only what release-please owns: a
new CHANGELOG.md section, the manifest's version and package.json's version. The code it carries is
the commit CI has just built and tested on main, and the release workflow builds the tagged commit
again. Such a run skips the Rust and frontend jobs; anything else runs every job.

The run is fast only when every check below holds (github-project-scaffold's ci.md, "Use release
PR fast lanes only with a delta proof"):

- pull_request: the author is release-please's bot, the head repository is this repository and the
  title is `chore: release X.Y.Z`. push to main: the head commit's author is that bot and its
  subject is `chore: release X.Y.Z (#N)`. merge_group and every other event run in full. The bot is
  github-actions[bot], or the release app's bot the repository names with both RELEASE_BOT_LOGIN
  (`<slug>[bot]`) and RELEASE_BOT_EMAIL (`<id>+<slug>[bot]@users.noreply.github.com`).
- check-release-delta.py, read from the base commit rather than the head, proves that the head sits
  directly on the base and changes CHANGELOG.md, .release-please-manifest.json and nothing of
  package.json but its version.
- the title's version, package.json's and the manifest's agree, and are greater than the base's.
- CHANGELOG.md keeps its first line and everything after it, adding exactly one `## [` section.

Writes `fast=true|false` and `reason=...` to $GITHUB_OUTPUT and exits 0 either way: a doubt is a
full run, never a failure. `--self-test` runs the checks against throwaway repositories.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

BOT_LOGIN = "github-actions[bot]"
BOT_EMAIL = "41898282+github-actions[bot]@users.noreply.github.com"
PR_TITLE = re.compile(r"^chore: release (\d+)\.(\d+)\.(\d+)$")
COMMIT_SUBJECT = re.compile(r"^chore: release (\d+)\.(\d+)\.(\d+) \(#\d+\)$")
SHA = re.compile(r"^[0-9a-f]{40}$")
DELTA_SCRIPT = ".github/scripts/check-release-delta.py"
ALLOW_GLOBS = "CHANGELOG.md .release-please-manifest.json"
VERSION_ONLY_GLOBS = "package.json"


def git(repo: Path, *args: str) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), *args], check=True, capture_output=True, text=True
    ).stdout


def version_of(text: str) -> tuple[int, int, int]:
    match = re.fullmatch(r"(\d+)\.(\d+)\.(\d+)", text)
    if not match:
        raise ValueError(f"not a plain X.Y.Z version: {text!r}")
    return tuple(int(part) for part in match.groups())  # type: ignore[return-value]


def release_bots(event: dict[str, str]) -> tuple[set[str], set[str]]:
    """The logins and commit emails release-please writes as: github-actions[bot], and the release
    app's bot where the repository names both, the email the bot's own no-reply address."""
    logins, emails = {BOT_LOGIN}, {BOT_EMAIL}
    login, email = event.get("bot_login", ""), event.get("bot_email", "")
    if (
        re.fullmatch(r"[a-z0-9-]+\[bot\]", login)
        and re.fullmatch(rf"\d+\+{re.escape(login)}@users\.noreply\.github\.com", email)
    ):
        logins.add(login)
        emails.add(email)
    return logins, emails


def decide(repo: Path, event: dict[str, str]) -> tuple[bool, str]:
    """(fast, reason) for one run. Any exception means a full run."""
    name = event.get("event_name", "")
    base, head = event.get("base", ""), event.get("head", "")
    logins, emails = release_bots(event)
    if name == "pull_request":
        if event.get("author") not in logins:
            return False, "not a pull request from release-please's bot"
        if event.get("head_repo") != event.get("repo"):
            return False, "the head is in another repository"
        title = PR_TITLE.fullmatch(event.get("title", ""))
        if not title:
            return False, "the title is not `chore: release X.Y.Z`"
    elif name == "push":
        if event.get("ref") != "refs/heads/main":
            return False, "not a push to main"
        if not (SHA.fullmatch(head) and SHA.fullmatch(base)):
            return False, "the push has no single previous commit"
        author = git(repo, "log", "-1", "--format=%ae", head).strip()
        if author not in emails:
            return False, "the head commit is not release-please's bot's"
        title = COMMIT_SUBJECT.fullmatch(git(repo, "log", "-1", "--format=%s", head).strip())
        if not title:
            return False, "the head commit is not `chore: release X.Y.Z (#N)`"
    else:
        return False, f"{name or 'this event'} always runs in full"
    if not (SHA.fullmatch(base) and SHA.fullmatch(head)):
        return False, "base or head is not a full commit id"

    # The proof's own code comes from the base: the head cannot vouch for itself.
    try:
        script = git(repo, "show", f"{base}:{DELTA_SCRIPT}")
    except subprocess.CalledProcessError:
        return False, "the base has no check-release-delta.py"
    with tempfile.NamedTemporaryFile("w", suffix=".py", delete=False) as handle:
        handle.write(script)
        checker = handle.name
    try:
        proof = subprocess.run(
            [
                sys.executable,
                checker,
                "--repo",
                str(repo),
                "--base",
                base,
                "--head",
                head,
                "--allow-globs",
                ALLOW_GLOBS,
                "--version-only-globs",
                VERSION_ONLY_GLOBS,
            ],
            capture_output=True,
            text=True,
        )
    finally:
        os.unlink(checker)
    if proof.returncode != 0:
        detail = proof.stderr.strip().splitlines()
        return False, "the delta is not release-please's: " + (detail[-1] if detail else "?")

    wanted = tuple(int(part) for part in title.groups())
    package = json.loads(git(repo, "show", f"{head}:package.json"))
    before = json.loads(git(repo, "show", f"{base}:package.json"))
    manifest = json.loads(git(repo, "show", f"{head}:.release-please-manifest.json"))
    versions = {version_of(package["version"]), version_of(manifest["."])}
    if versions != {wanted}:
        return False, "the title, package.json and the manifest name different versions"
    if not wanted > version_of(before["version"]):
        return False, "the version does not go up"

    old = git(repo, "show", f"{base}:CHANGELOG.md")
    new = git(repo, "show", f"{head}:CHANGELOG.md")
    old_head, _, old_rest = old.partition("\n")
    new_head, _, new_rest = new.partition("\n")
    if new_head != old_head or not new_rest.endswith(old_rest):
        return False, "CHANGELOG.md rewrites what it had"
    added = new_rest[: len(new_rest) - len(old_rest)]
    if sum(1 for line in added.splitlines() if line.startswith("## [")) != 1:
        return False, "CHANGELOG.md does not add exactly one version section"
    return True, f"release-please's version bump to {'.'.join(map(str, wanted))}"


def from_environment() -> dict[str, str]:
    return {
        "event_name": os.environ.get("EVENT_NAME", ""),
        "author": os.environ.get("PR_AUTHOR", ""),
        "head_repo": os.environ.get("PR_HEAD_REPO", ""),
        "repo": os.environ.get("REPO", ""),
        "title": os.environ.get("PR_TITLE", ""),
        "ref": os.environ.get("RUN_REF", ""),
        "base": os.environ.get("BASE_SHA", ""),
        "head": os.environ.get("HEAD_SHA", ""),
        "bot_login": os.environ.get("RELEASE_BOT_LOGIN", ""),
        "bot_email": os.environ.get("RELEASE_BOT_EMAIL", ""),
    }


def run(repo: Path) -> None:
    try:
        fast, reason = decide(repo, from_environment())
    except Exception as error:  # noqa: BLE001 - any doubt is a full run
        fast, reason = False, f"could not decide: {error}"
    print(f"fast={str(fast).lower()}: {reason}")
    output = os.environ.get("GITHUB_OUTPUT")
    if output:
        with open(output, "a", encoding="utf-8") as handle:
            handle.write(f"fast={str(fast).lower()}\n")
            handle.write(f"reason={reason}\n")


def self_test() -> None:
    """Every rule against a throwaway repository: one fast case, each departure from it full."""
    checker = Path(__file__).with_name("check-release-delta.py").read_text(encoding="utf-8")
    changelog = "# Changelog\n\n## [0.0.5](u) (2026-10-06)\n\n* earlier\n"
    package = '{\n  "name": "w",\n  "version": "0.0.5",\n  "private": true\n}\n'
    scratch = Path(tempfile.mkdtemp(prefix="release-fast-lane-"))
    try:
        cases = run_cases(scratch, checker, changelog, package)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
    failed = [
        f"{label}: fast={fast} ({reason}), expected {expected}"
        for label, (fast, reason), expected in cases
        if fast != expected
    ]
    for line in failed:
        print(f"FAIL {line}", file=sys.stderr)
    if failed:
        raise SystemExit(1)
    print(f"release-fast-lane self-test: {len(cases)} cases pass")


def run_cases(scratch: Path, checker: str, changelog: str, package: str) -> list:
    """The self-test's cases, each in its own repository under `scratch`."""

    def repo_with(edit, email: str = BOT_EMAIL) -> tuple[Path, str, str]:
        root = Path(tempfile.mkdtemp(dir=scratch))
        git(root, "init", "-q", "-b", "main")
        git(root, "config", "user.name", "t")
        git(root, "config", "user.email", "t@example.com")
        (root / ".github/scripts").mkdir(parents=True)
        (root / DELTA_SCRIPT).write_text(checker, encoding="utf-8")
        (root / "CHANGELOG.md").write_text(changelog, encoding="utf-8")
        (root / "package.json").write_text(package, encoding="utf-8")
        (root / ".release-please-manifest.json").write_text('{\n  ".": "0.0.5"\n}\n')
        (root / "src.rs").write_text("fn main() {}\n")
        git(root, "add", "-A")
        git(root, "commit", "-q", "-m", "feat: base")
        base = git(root, "rev-parse", "HEAD").strip()
        files = {
            "CHANGELOG.md": changelog.replace(
                "\n\n## [0.0.5]", "\n\n## [0.0.6](u) (2026-10-07)\n\n* new\n\n## [0.0.5]", 1
            ),
            "package.json": package.replace('"0.0.5"', '"0.0.6"'),
            ".release-please-manifest.json": '{\n  ".": "0.0.6"\n}\n',
        }
        edit(files)
        for path, text in files.items():
            (root / path).write_text(text, encoding="utf-8")
        git(root, "add", "-A")
        git(
            root,
            "-c",
            "user.name=github-actions[bot]",
            "-c",
            f"user.email={email}",
            "commit",
            "-q",
            "-m",
            "chore: release 0.0.6 (#12)",
        )
        return root, base, git(root, "rev-parse", "HEAD").strip()

    def pr(base: str, head: str, **change: str) -> dict[str, str]:
        event = {
            "event_name": "pull_request",
            "author": BOT_LOGIN,
            "head_repo": "o/r",
            "repo": "o/r",
            "title": "chore: release 0.0.6",
            "base": base,
            "head": head,
        }
        event.update(change)
        return event

    def push(base: str, head: str) -> dict[str, str]:
        return {"event_name": "push", "ref": "refs/heads/main", "base": base, "head": head}

    cases = []
    root, base, head = repo_with(lambda files: None)
    cases += [
        ("a release PR", decide(root, pr(base, head)), True),
        ("its squash commit on main", decide(root, push(base, head)), True),
        ("a human's PR", decide(root, pr(base, head, author="someone")), False),
        ("a PR from a fork", decide(root, pr(base, head, head_repo="x/r")), False),
        ("another title", decide(root, pr(base, head, title="chore: release 0.0.7")), False),
        ("a merge queue", decide(root, {**pr(base, head), "event_name": "merge_group"}), False),
        ("a push elsewhere", decide(root, {**push(base, head), "ref": "refs/heads/x"}), False),
    ]

    # The release app's bot, once the repository names it.
    app, app_email = "winer-release[bot]", "123+winer-release[bot]@users.noreply.github.com"
    named = {"bot_login": app, "bot_email": app_email}
    cases += [
        ("the app's release PR", decide(root, pr(base, head, author=app, **named)), True),
        ("the app's PR, the app not named", decide(root, pr(base, head, author=app)), False),
        (
            "a human named as the bot",
            decide(root, pr(base, head, author="someone", bot_login="someone", bot_email=app_email)),
            False,
        ),
    ]
    root, base, head = repo_with(lambda files: None, email=app_email)
    cases += [
        ("the app's squash commit", decide(root, {**push(base, head), **named}), True),
        ("the app's commit, the app not named", decide(root, push(base, head)), False),
    ]

    def extra_file(files):
        files["src.rs"] = "fn main() { println!(); }\n"

    def other_field(files):
        files["package.json"] = files["package.json"].replace('"private": true', '"private": false')

    def rewritten(files):
        files["CHANGELOG.md"] = files["CHANGELOG.md"].replace("* earlier", "* rewritten")

    def two_sections(files):
        files["CHANGELOG.md"] = files["CHANGELOG.md"].replace(
            "\n\n## [0.0.6]", "\n\n## [0.0.7](u)\n\n## [0.0.6]", 1
        )

    def disagreeing(files):
        files[".release-please-manifest.json"] = '{\n  ".": "0.0.7"\n}\n'

    for label, edit in [
        ("a code change", extra_file),
        ("another package.json field", other_field),
        ("a rewritten changelog", rewritten),
        ("two changelog sections", two_sections),
        ("disagreeing versions", disagreeing),
    ]:
        root, base, head = repo_with(edit)
        cases.append((label, decide(root, pr(base, head)), False))
    return cases


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        self_test()
    else:
        run(args.repo.resolve())


if __name__ == "__main__":
    main()
