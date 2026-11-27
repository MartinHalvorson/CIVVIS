from pathlib import Path
import re
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import commit_identity as ci

TOOLS = Path(__file__).resolve().parent
MARTBOT = "285244636+martbot-1@users.noreply.github.com"


def row(sha="a" * 40, *, author="martbot-1", committer="martbot-1",
        author_email=MARTBOT, committer_email=MARTBOT,
        author_name="martbot-1", message="Do a thing"):
    """One row of GET /pulls/{n}/commits: top-level login is GitHub's resolution."""
    return {
        "sha": sha,
        "author": {"login": author} if author else None,
        "committer": {"login": committer} if committer else None,
        "commit": {
            "author": {"name": author_name, "email": author_email},
            "committer": {"name": committer or "?", "email": committer_email},
            "message": message,
        },
    }


class CoAuthorsTest(unittest.TestCase):
    def test_reads_every_trailer_case_insensitively(self):
        message = (
            "Subject\n\nBody\n\n"
            "Co-authored-by: Claude Opus 5.5 <noreply@anthropic.com>\n"
            "co-authored-by:   draft   <d@d>  \n"
        )
        self.assertEqual(
            ci.co_authors(message),
            [("Claude Opus 5.5", "noreply@anthropic.com"), ("draft", "d@d")],
        )

    def test_prose_mentioning_the_word_is_not_a_trailer(self):
        self.assertEqual(ci.co_authors("we co-authored-by hand <x@y> today"), [])


class IdentityErrorsTest(unittest.TestCase):
    def test_fleet_commits_pass(self):
        commits = [
            row(message="Work\n\nCo-authored-by: Claude Opus 5.5 <noreply@anthropic.com>"),
            row("b" * 40, author="MartinHalvorson", committer="MartinHalvorson",
                author_email="someone@example.com", committer_email="someone@example.com"),
            row("c" * 40, author="claude", committer="web-flow",
                author_email="noreply@anthropic.com", committer_email="noreply@github.com"),
        ]
        body = "## What changed\n\nCo-authored-by: martbot-1 <%s>\n" % MARTBOT
        self.assertEqual(ci.identity_errors(commits, body), [])

    def test_the_5753_draft_commit_is_refused(self):
        # GitHub resolved `draft <d@d>` to an unrelated account.
        commits = [row(author="dimas1", author_name="draft", author_email="d@d",
                       message="draft: invasion chart on #5745")]
        errors = ci.identity_errors(commits)
        self.assertIn("@dimas1", errors[0])
        self.assertIn("draft <d@d>", errors[0])
        self.assertEqual(errors[-1], ci.HOW_TO_FIX)

    def test_an_address_github_resolves_to_nobody_is_refused(self):
        errors = ci.identity_errors([row(author=None, author_email="civvis@local")])
        self.assertIn("civvis@local", errors[0])
        self.assertIn("no allowed account", errors[0])

    def test_a_stranger_committer_is_refused_but_web_flow_is_not(self):
        self.assertEqual(ci.identity_errors([row(committer="web-flow")]), [])
        errors = ci.identity_errors([row(committer="someone-else")])
        self.assertIn("committed by @someone-else", errors[0])
        self.assertTrue(ci.identity_errors([row(committer=None)]))

    def test_web_flow_never_counts_as_an_author(self):
        self.assertTrue(ci.identity_errors([row(author="web-flow")]))

    def test_an_unknown_trailer_in_a_commit_is_refused(self):
        # The exact shape that reached main: an allowed author, a bad trailer.
        message = "Squash\n\nCo-authored-by: draft <d@d>\n"
        errors = ci.identity_errors([row(message=message)])
        self.assertIn("'Co-authored-by: draft <d@d>'", errors[0])

    def test_a_trailer_in_the_pr_body_is_refused(self):
        errors = ci.identity_errors([row()], "Body\n\nCo-authored-by: m <a@b>\n")
        self.assertIn("the PR body", errors[0])
        self.assertIn("a@b", errors[0])

    def test_a_personal_address_is_trusted_only_where_github_resolved_it(self):
        mine = "owner@example.com"
        trailer = "x\n\nCo-authored-by: Owner <%s>\n" % mine
        self.assertTrue(ci.identity_errors([row(message=trailer)]))
        resolved = row("b" * 40, author="MartinHalvorson", author_email=mine)
        self.assertEqual(ci.identity_errors([row(message=trailer), resolved]), [])
        # ...and not where GitHub resolved it to someone else.
        stranger = row("b" * 40, author="dimas1", author_email=mine)
        self.assertNotIn(mine, ci.resolved_addresses([stranger]))

    def test_addresses_compare_case_insensitively(self):
        message = "x\n\nCo-authored-by: Claude <NoReply@Anthropic.com>\n"
        self.assertEqual(ci.identity_errors([row(message=message)]), [])

    def test_commits_the_api_did_not_return_fail_closed(self):
        errors = ci.identity_errors([row()], expected_commits=300)
        self.assertIn("300 commits", errors[0])

    def test_noreply_addresses_name_allowed_accounts(self):
        self.assertLessEqual(set(ci.ACCOUNT_NOREPLY.values()), set(ci.ALLOWED_ACCOUNTS))
        for address in ci.ACCOUNT_NOREPLY:
            self.assertEqual(address, address.lower())


class FallbackIdentityTest(unittest.TestCase):
    """Tools that write commits with no configured identity use the fleet's."""

    def test_fleet_identity_is_an_allowed_noreply_address(self):
        for key in ("GIT_AUTHOR_EMAIL", "GIT_COMMITTER_EMAIL"):
            self.assertIn(ci.FLEET_IDENTITY[key], ci.ACCOUNT_NOREPLY)

    def test_tools_set_no_made_up_identity(self):
        assigned = re.compile(
            r"""["'](?:GIT_(?:AUTHOR|COMMITTER)_EMAIL)["']\s*:\s*["']([^"']+)["']"""
            r"""|user\.email=([^\s"',`]+@[^\s"',`]+)"""
        )
        offenders = []
        for path in sorted(TOOLS.glob("*.py")):
            if path.name.startswith("test_"):
                continue  # throwaway repos in tests never reach GitHub
            for match in assigned.finditer(path.read_text(encoding="utf-8")):
                email = match.group(1) or match.group(2)
                if email not in ci.ACCOUNT_NOREPLY:
                    offenders.append(f"{path.name}: {email}")
        self.assertEqual(offenders, [])

    def test_ladder_identity_matches_the_fleet_identity(self):
        import civ6_ladder
        self.assertEqual(civ6_ladder.LEDGER_IDENTITY, ci.FLEET_IDENTITY)


class CheckPrWiringTest(unittest.TestCase):
    def test_check_pr_runs_the_identity_check(self):
        source = (TOOLS / "civvis_collab.py").read_text(encoding="utf-8")
        start = source.index("def check_pr_action(")
        end = source.index("\ndef ", start + 1)
        self.assertIn("commit_identity.identity_errors(", source[start:end])


if __name__ == "__main__":
    unittest.main()
