# Project standards for outside agents

This file is for any coding agent that works on this repository from outside the
maintainer's own team, such as a Codex task. It sets out how the project works,
what a pull request must contain, and how review happens. Codex reads this file
on its own; other agents should read it before starting.

The full guides are in the repository, and they win over this summary:

- `docs/architecture.md`: where each decision lives, from the sound to the screen.
  Section 11 is the checklist for any change to a figure. Section 12 is code shape.
- `docs/math-standards.md`: one definition of each statistic. Section 14 is the
  checklist for any change to how a figure is computed.
- `docs/agent-interface.md`: the CLI's JSON contract.
- `docs/design/design-guide.md`: how the desktop app looks and words things.

## How work reaches you, and how review works

- **Issues labelled `for Astra` are yours to pick up.** Work only on open issues
  with that label. Skip one that already has an open PR that names it (`Closes #N`
  or `Part of #N`), and skip one whose newest comment says to wait. The label is
  added only once an issue has a scope comment and no collision with other work;
  never add or remove it yourself.
- **You work from a GitHub issue.** Read the whole issue, including every comment,
  before you start. The newest comment wins over the issue body; the scope comment
  from the maintainer's team sets the files and tests for the task.
- **One issue, one branch, one pull request.** Branch from current `main` as
  `codex/<issue number>-<short-name>` (for example `codex/638-saved-gap-cuts`). Open the PR ready for review, not as a draft.
- **Stay inside the files the issue or your brief names.** If you truly need another
  file, say which and why in the PR body under **Shared files**. Other agents are
  working on the repository at the same time, so an unexpected file can collide
  with their open PRs.
- **Review happens in PR comments.** Reviewers post change requests as comments on
  your PR. Fix each one with a new commit, then reply to the comment saying what you
  changed and at which commit. Do not resolve a reviewer's thread yourself.
- **If a comment is wrong,** reply with the evidence (a test, a command and its
  output). Do not change code you believe is right just to end a thread.
- **If you are blocked** on a decision the issue doesn't settle, stop and ask in a
  comment on the issue or the PR. Do not guess at a behaviour the maintainer has
  not chosen.
- **You never merge.** Only the maintainer merges. Do not approve PRs, close issues
  by hand, rebase or force-push a branch that already has review comments, or push
  to a branch you did not open. Reviewers may press GitHub's "Update branch" on your
  PR (a merge commit); pull it before your next commit.

## What the software is

An open-source timegrapher: it listens to a mechanical watch through a microphone
and measures rate, amplitude and beat error. It is a Rust workspace:

- `timegrapher-core` does every calculation.
- `timegrapher-cli` (the command line, which is also the interface for AI agents),
  `timegrapher-app` (the egui desktop app) and `timegrapher-mcp` (an MCP server)
  only call core and draw what it returns.

## The rules that matter most

1. **Core decides; surfaces draw.** Any number, threshold, window, selection rule or
   sentence of fact lives in core. The app, CLI and MCP never compare a reading with
   a number, pick a clock, cut a window or word a fact.
2. **One rule, one place.** Logic written twice is a defect, even when the copies
   agree today. Reuse the existing helper (make it `pub(crate)` if needed). Each
   limit or threshold is a named constant in core with a doc comment saying why it
   has that value.
3. **Facts only.** The software states what it measured. No grades, verdicts,
   fault calls or advice in its voice. Caveats are factual flags in `caveats[]`.
4. **Cull, never fill.** Missing sound is never interpolated. No reading straddles a
   gap, mask or break.
5. **One clock per take,** chosen by core, used by every pane and surface.
6. **The JSON contract is additive.** New fields are fine. Removing or changing a
   field needs a new schema version and is not something to do inside an issue fix.
   The CLI's JSON shape tests (`crates/timegrapher-cli/tests/json_shapes/`) record
   every field; update them only for a field you add, and say so.
7. **Conventions:** Tick and Tock in all user-facing text (JSON keeps `a`/`b`).
   Positions are DU, DD, CU, CD, CL, CR. Beat error is signed in JSON, and its size
   is shown in text. Rates are in s/d. A "±" takes a space after a value
   (`+11.6 ± 2.2 s/d`).

## Changes to a figure or to the math

A change to how a figure is computed (an average, fit, window, gate, test, ± or
rounding) must pass the checklist in section 14 of `docs/math-standards.md`. In
the PR body:

- name the rule from the guide that the change follows;
- give every number that moves, old and new, on a stated input;
- for any new or changed ± or p-value, add a seeded coverage test, run on null data
  and on data with gaps, and report the coverage.

If the corpus regression (`regress/baseline.json`) moves, list each value that
moves with its old value, new value and reason. Nothing else in the baseline may
move.

## Tests

- **Every test for a fix must fail on `main`.** Check that it does, and say so in the PR.
  A test that pins existing behaviour (for example on a change that only moves code)
  passes on `main` by design; say which tests are of that kind.
- **Every test for a fix must also fail if you remove the line that does the work**, not only
  an error message after it. Try it and report it.
- **Never remove, skip, loosen or edit an existing test** to get green. A test may
  go only with the feature it covers, and the PR body names both.
- Never push an empty commit, or close and reopen a PR, to re-run CI.

## Code shape (enforced in CI)

Functions at most 100 lines, cyclomatic complexity at most 25, files at most 3000
lines. A file already over a cap may only shrink.

## Before you push

All of these must pass:

```
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --release --locked --workspace
```

Building the desktop app on Linux needs `libasound2-dev`. CI runs seven required
checks (tests on Linux, macOS and Windows, the MCP renderer and three corpus
regression checks) and takes about 15 minutes. If one fails, fix the cause.

Do not change `Cargo.toml`, `Cargo.lock`, `.github/`, any `main.rs` or `lib.rs`, or
`docs/agent-interface.md` unless your brief lists them. If you must, explain why
under **Shared files**.

## The pull request body

Use these sections, each a bold lead-in followed by full sentences, with exact
names in backticks:

- **Before:** what happens on `main`, and why it matters.
- **After:** what happens now.
- **How:** the change, function by function, and where each rule or limit lives.
- **What moves:** every output, figure or message that changes, and what stays the
  same. Write "Nothing a user sees changes" if that is true.
- **Tests:** each new test by name, what it proves, and how you confirmed it fails on
  `main` and without the fix (or that it pins existing behaviour). State that no
  existing test was changed.
- **Shared files:** any file outside the task's list, and why. "None" if none.
- **Checks:** the commands you ran, at which commit, and their results.
- `Closes #N` on its own line, or `Part of #N` when the issue needs more than one PR.
- Last line: `— <your agent name> (<work area>)`.

Every sentence in the body must be true at the PR's head. Update the body when a
later commit changes what it says.

## Privacy

No personal names, emails, home-folder paths or AI model names in code, comments,
commit messages or the PR body. Recordings other than the public corpus
(`timegrapher-corpus`) are private; do not ask for them or refer to their contents.

## Issues

Open a new issue only for a real, confirmed problem you found that your PR does not
fix and that would otherwise be lost. Say how to reproduce it. Do not open issues for
plans, questions or notes; ask those in a comment on your PR instead.
