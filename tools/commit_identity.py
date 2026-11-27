"""Who GitHub may credit for a CIVVIS commit.

GitHub credits a commit to whichever account owns the address on it: the
author, the committer, and every ``Co-authored-by:`` trailer. A squash merge
copies the authors and trailers of every commit in the pull request onto the
one commit that lands on ``main``, and the pull-request body *is* that commit's
message, so a trailer typed into the body lands on ``main`` too. Every account
named that way shows on the repository's public contributor list.

#5753 is why this exists. A draft commit made under a placeholder identity,
``draft <d@d>``, rode into the squash merge as a co-author; ``d@d`` belongs to
an unrelated 2013 GitHub account, so a stranger appeared as a CIVVIS
contributor. Nothing about the placeholder looked dangerous: any address
nobody here owns is one somebody else can own.

So the rule is an allowlist, checked against GitHub's own resolution rather
than against names a commit can claim:

* the author of every commit must be an account GitHub resolves to one of
  ``ALLOWED_ACCOUNTS``; an address GitHub resolves to nobody fails too;
* the committer must be one of those, or ``web-flow`` (GitHub's own merges and
  web edits);
* every ``Co-authored-by:`` address, in a commit message or the PR body, must
  be an allowed account's noreply address, or an address this same PR shows
  GitHub resolving to an allowed account.
"""

from __future__ import annotations

import re
from typing import Any, Dict, Iterable, List, Mapping, Optional, Sequence, Set, Tuple

ALLOWED_ACCOUNTS: Tuple[str, ...] = ("MartinHalvorson", "martbot-1", "claude")

# GitHub's committer for squash merges and web edits. It is never an author.
MERGE_COMMITTER = "web-flow"

# Addresses that resolve to an allowed account wherever they appear. These are
# the ones to put in a trailer; personal addresses are deliberately not listed
# in a public file, and are accepted only where the PR itself shows GitHub
# resolving them (see ``resolved_addresses``).
ACCOUNT_NOREPLY: Dict[str, str] = {
    "12651154+martinhalvorson@users.noreply.github.com": "MartinHalvorson",
    "285244636+martbot-1@users.noreply.github.com": "martbot-1",
    "noreply@anthropic.com": "claude",
}

# The identity to use when a tool has to write a commit with no configured
# identity (a launchd job, a fresh runner). It is martbot-1's noreply address,
# so the commit is credited to the fleet's own account if it ever reaches a PR.
FLEET_IDENTITY: Dict[str, str] = {
    "GIT_AUTHOR_NAME": "martbot-1",
    "GIT_AUTHOR_EMAIL": "285244636+martbot-1@users.noreply.github.com",
    "GIT_COMMITTER_NAME": "martbot-1",
    "GIT_COMMITTER_EMAIL": "285244636+martbot-1@users.noreply.github.com",
}

TRAILER_RE = re.compile(
    r"^[ \t]*co-authored-by:[ \t]*(?P<name>[^<\n]*?)[ \t]*<(?P<email>[^<>\n]*)>[ \t]*$",
    re.IGNORECASE | re.MULTILINE,
)

HOW_TO_FIX = (
    "Every author, committer and Co-authored-by on a CIVVIS pull request must "
    "be an account GitHub credits to martbot-1, MartinHalvorson or claude: the "
    "squash merge copies them onto main and GitHub lists each one as a "
    "contributor. Never commit under a made-up identity (`-c user.email=...`, "
    "`GIT_AUTHOR_EMAIL=...`); tools that need a fallback use "
    "`commit_identity.FLEET_IDENTITY`. A trailer in the PR body: edit the body. "
    "A commit not yet pushed: `git commit --amend --no-edit --reset-author` or "
    "delete the trailer. A commit already pushed (branches are never "
    "force-pushed): cherry-pick your own commits onto a fresh task branch from "
    "current origin/main and close this PR. If the named commit is not yours "
    "but a main commit your branch merged in, main has since been rewritten "
    "and this branch still carries the old history: rebuild it the same way."
)


def co_authors(message: str) -> List[Tuple[str, str]]:
    """Every ``Co-authored-by:`` trailer in ``message`` as (name, address)."""
    return [
        (match.group("name").strip(), match.group("email").strip())
        for match in TRAILER_RE.finditer(message or "")
    ]


def _login(person: Optional[Mapping[str, Any]]) -> Optional[str]:
    if not person:
        return None
    login = person.get("login")
    return str(login) if login else None


def _ident(person: Optional[Mapping[str, Any]]) -> str:
    person = person or {}
    return f"{person.get('name') or '?'} <{person.get('email') or '?'}>"


def resolved_addresses(
    commits: Iterable[Mapping[str, Any]],
    allowed: Sequence[str] = ALLOWED_ACCOUNTS,
) -> Set[str]:
    """Addresses this PR shows GitHub resolving to an allowed account."""
    allowed_set = set(allowed)
    found: Set[str] = set()
    for row in commits:
        commit = row.get("commit") or {}
        for role in ("author", "committer"):
            if _login(row.get(role)) in allowed_set:
                email = str((commit.get(role) or {}).get("email") or "").lower()
                if email:
                    found.add(email)
    return found


def identity_errors(
    commits: Sequence[Mapping[str, Any]],
    body: str = "",
    *,
    expected_commits: Optional[int] = None,
    allowed: Sequence[str] = ALLOWED_ACCOUNTS,
) -> List[str]:
    """Why GitHub would credit someone outside ``allowed`` for this PR.

    ``commits`` are rows of ``GET /repos/{repo}/pulls/{n}/commits``: each has
    ``sha``, top-level ``author``/``committer`` (the GitHub account the address
    resolves to, or null) and ``commit.author``/``commit.committer``/
    ``commit.message`` (what the commit itself says).
    """
    allowed_set = set(allowed)
    errors: List[str] = []
    if expected_commits is not None and expected_commits > len(commits):
        errors.append(
            f"this PR has {expected_commits} commits but only {len(commits)} "
            "could be checked for who GitHub credits; squash it to fewer "
            "commits so every one can be checked"
        )
    trusted = {address.lower() for address in ACCOUNT_NOREPLY}
    trusted |= resolved_addresses(commits, allowed)

    def check_trailers(message: str, where: str) -> None:
        for name, email in co_authors(message):
            if email.lower() not in trusted:
                errors.append(
                    f"{where} has 'Co-authored-by: {name} <{email}>', an address "
                    "not known to belong to martbot-1, MartinHalvorson or "
                    "claude, so GitHub could credit anyone who owns it"
                )

    for row in commits:
        sha = str(row.get("sha") or "")[:9] or "?"
        commit = row.get("commit") or {}
        subject = str(commit.get("message") or "").split("\n", 1)[0][:60]
        where = f"commit {sha} ({subject!r})"
        author = _login(row.get("author"))
        if author is None:
            errors.append(
                f"{where} is authored by '{_ident(commit.get('author'))}', an "
                "address GitHub ties to no allowed account"
            )
        elif author not in allowed_set:
            errors.append(
                f"{where} is credited by GitHub to @{author} "
                f"('{_ident(commit.get('author'))}'), who is not martbot-1, "
                "MartinHalvorson or claude"
            )
        committer = _login(row.get("committer"))
        if committer is None:
            errors.append(
                f"{where} is committed by '{_ident(commit.get('committer'))}', "
                "an address GitHub ties to no allowed account"
            )
        elif committer not in allowed_set and committer != MERGE_COMMITTER:
            errors.append(
                f"{where} is committed by @{committer}, who is not martbot-1, "
                "MartinHalvorson or claude"
            )
        check_trailers(str(commit.get("message") or ""), where)
    # The squash merge uses the PR body as the commit message.
    check_trailers(body or "", "the PR body (it becomes the squash commit's message)")
    if errors:
        errors.append(HOW_TO_FIX)
    return errors
