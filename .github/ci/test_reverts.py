"""Offline tests of the silent-revert check. Builds a small git repo in a
temporary folder; no network."""

import contextlib
import io
import os
import pathlib
import subprocess
import tempfile
import unittest

import reverts


class Restored(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        self.cwd = os.getcwd()
        os.chdir(self.root)
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.email", "ci@example.invalid")
        self.git("config", "user.name", "CI")
        self.git("config", "core.autocrlf", "false")
        self.old = self.commit({"a.rs": "fn a() { old }\n", "b.rs": "fn b() {}\n"}, "first")
        self.new = self.commit({"a.rs": "fn a() { new }\n"}, "Merge pull request #2")

    def tearDown(self):
        os.chdir(self.cwd)
        self.tmp.cleanup()

    def git(self, *args):
        return subprocess.run(["git", *args], capture_output=True, text=True,
                              check=True).stdout.strip()

    def commit(self, files, message):
        for name, text in files.items():
            (self.root / name).write_text(text, encoding="utf-8")
        self.git("add", "-A")
        self.git("commit", "-q", "-m", message)
        return self.git("rev-parse", "HEAD")

    def branch(self, files):
        self.git("checkout", "-q", "-b", "pr")
        head = self.commit(files, "pr")
        self.git("checkout", "-q", "main")
        return head

    def run_main(self, head, body=""):
        os.environ["PR_BODY"] = body
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = reverts.main(["reverts.py", self.new, head])
        return code, out.getvalue()

    def test_file_put_back_to_an_older_version_is_flagged(self):
        head = self.branch({"a.rs": "fn a() { old }\n"})
        self.assertEqual(reverts.restored(self.new, head), [("a.rs", self.old)])
        code, out = self.run_main(head)
        self.assertEqual(code, 1)
        self.assertIn("a.rs: identical to main at", out)

    def test_new_content_passes(self):
        head = self.branch({"a.rs": "fn a() { newer }\n", "c.rs": "fn c() {}\n"})
        self.assertEqual(reverts.restored(self.new, head), [])
        self.assertEqual(self.run_main(head)[0], 0)

    def test_body_naming_the_revert_passes_but_still_lists_the_file(self):
        head = self.branch({"a.rs": "fn a() { old }\n"})
        code, out = self.run_main(head, "Some text.\nReverts #2\n")
        self.assertEqual(code, 0)
        self.assertIn("a.rs", out)

    def test_body_that_only_mentions_a_pr_does_not_pass(self):
        head = self.branch({"a.rs": "fn a() { old }\n"})
        self.assertEqual(self.run_main(head, "Follows #2.")[0], 1)

    def test_emptied_file_is_not_flagged(self):
        self.commit({"e.py": "x = 1\n"}, "fill")
        self.new = self.commit({"e.py": ""}, "empty")
        self.new = self.commit({"e.py": "x = 2\n"}, "refill")
        head = self.branch({"e.py": ""})
        self.assertEqual(reverts.restored(self.new, head), [])


if __name__ == "__main__":
    unittest.main()
