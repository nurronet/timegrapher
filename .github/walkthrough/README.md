# Windows walkthroughs

A real Windows 11 desktop (a GitHub `windows-latest` runner) that an agent
drives live: clicks, keys, typing, and a screenshot after every batch.

## What it is

- **Display:** 1920 x 1080. The runner's virtual display offers nothing larger.
  Smaller sizes are window resizes.
- **No zoom:** walk at the app's normal scale. Don't use Ctrl+minus or Ctrl+plus.
- **No sound input:** there's no microphone, so walk recordings opened from files.
- **Session:** the first answer arrives about 10 minutes after starting, once the
  build is done. After that each step takes about 10 to 15 s. A session lasts up
  to `minutes` (default 120, 340 at most).
- **Cost:** free, because the repository is public.

## Start a session

1. Use your own branch as the channel and check it out locally. Never use a PR
   branch: the session pushes `walk/` files to the channel.
2. Start the `Windows walkthrough` workflow by hand (workflow_dispatch). Inputs:
   - `ref`: the PR head.
   - `base_ref`: optional, the PR base.
   - `channel`: your branch.
   - `minutes`: how long the session lasts.
3. Wait for `walk/out/ready-<run id>.png` on the channel. It shows the head build
   open on a fresh profile.

## Send steps

```
python3 .github/walkthrough/send.py '[["key","win","up"],["wait",2]]'
```

`send.py` pushes the next numbered step file and waits for the answer. It
prints the screenshot's path and the answer text:

- whether the app is running, or its exit code;
- the end of the app's stderr;
- the output of any `shell` step.

Steps are JSON arrays. Coordinates are screen pixels.

| Step | Does |
|---|---|
| `["click", x, y]`, `["click", x, y, "right"]` | Click |
| `["dclick", x, y]` | Double-click |
| `["move", x, y]` | Hover, e.g. for "?" help text |
| `["drag", x1, y1, x2, y2]` | Drag, e.g. a window edge to resize |
| `["scroll", clicks, x, y]` | Wheel; positive scrolls up |
| `["key", "ctrl", "o"]` | Key or chord ("alt","f4"; "win","up") |
| `["type", "text"]` | Type text |
| `["wait", seconds]` | Pause |
| `["launch", "head"]`, `["launch", "base"]` | Start that build fresh; any app running is killed |
| `["kill"]` | Kill the app (to test a crash restart) |
| `["shell", "command"]` | Run a cmd.exe command; its output is in the answer |
| `["stop"]` | End the session |

## Recordings

The repository's fixtures are under `D:\a\timegrapher\timegrapher\head\`.
Public corpus takes can be downloaded with a `shell` step, for example
`curl -sLo D:\t.flac <raw URL>`. Then open them through the app's Open
dialog by typing the full path.

## File-system checks

Run `dir` and `powershell Get-ChildItem` through `shell` before and after you
open, record, save or convert anything. Settings are in `%APPDATA%`, data and
cache in `%LOCALAPPDATA%`, and the library in `%USERPROFILE%\Timegrapher Data`.

## Safety

The code under test is checked out and built without credentials. The
session's write token is used only by the step loop, which starts the app
without it. Start sessions only for branches of this repository, never a
fork's.
