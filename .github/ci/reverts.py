"""Stop a pull request from silently putting back an older version of a file.

  reverts.py BASE HEAD     BASE is main as the PR merges into it (HEAD^1 of
                           GitHub's merge commit), HEAD the merge commit. The
                           PR body is read from the PR_BODY variable.

A branch that copies files over from a stale checkout can undo work that
merged since, and the old code still builds and passes its tests. This
lists every file the merge changes whose new content is byte-identical to a
version that file had earlier on main's first-parent line, so the work that
replaced that version would be lost.

A deliberate revert passes when the PR body names it with a line such as
`Reverts #838`. The files are still listed, so the reviewer sees them.
GitHub does not re-run CI when only the body is edited: re-run the job after
adding the line. CI reads the body live from the API for this, because a
re-run reuses the original event and its old body.

Needs main's full history (checkout fetch-depth 0). If git fails, this exits
with an error and the job fails.
"""

import os
import re
import subprocess
import sys

EMPTY_BLOB = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
NAMES_A_REVERT = re.compile(r"\bReverts? #\d+", re.IGNORECASE)


def git(*args):
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True).stdout


def changed(base, head):
    """(path, new blob) for each file the merge adds or changes."""
    out = []
    for line in git("diff-tree", "-r", "--no-renames", base, head).splitlines():
        meta, path = line.split("\t", 1)
        new = meta.split()[3]
        if set(new) != {"0"} and new != EMPTY_BLOB:
            out.append((path, new))
    return out


def earlier_versions(base, paths):
    """{path: {blob: commit}}: every version each path had on main's
    first-parent line before BASE's, with the commit that made it."""
    seen = {}
    if not paths:
        return seen
    commit = None
    log = git("log", "--first-parent", "--raw", "--no-renames", "--no-abbrev",
              "--format=%H", base, "--", *paths)
    for line in log.splitlines():
        if not line.startswith(":"):
            commit = line.strip() or commit
            continue
        meta, path = line.split("\t", 1)
        blob = meta.split()[3]
        seen.setdefault(path, {}).setdefault(blob, commit)
    return seen


def restored(base, head):
    """(path, commit) for each file put back to a version main had at commit."""
    files = changed(base, head)
    seen = earlier_versions(base, [path for path, _ in files])
    hits = []
    for path, new in files:
        commit = seen.get(path, {}).get(new)
        if commit:
            hits.append((path, commit))
    return hits


def main(argv):
    base, head = argv[1], argv[2]
    hits = restored(base, head)
    if not hits:
        print("No file is put back to an older version on main.")
        return 0
    for path, commit in hits:
        subject = git("log", "-1", "--format=%h %s", commit).strip()
        print(f"{path}: identical to main at {subject}; "
              f"the work main merged on this file since then would be lost.")
    if NAMES_A_REVERT.search(os.environ.get("PR_BODY", "")):
        print("The PR body names a revert, so this passes; reviewer, check the files above.")
        return 0
    print("If this is deliberate, add a line `Reverts #N` to the PR body and re-run "
          "this job. If not, the branch copied old files back: restore main's version.")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
