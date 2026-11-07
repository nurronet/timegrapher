# Timegrapher for Linux

This folder holds everything in this release, for 64-bit Intel and AMD PCs
with a distribution from 2022 or later (Ubuntu 22.04, Debian 12, Fedora 36
or newer).

| In this folder | What it is |
|---|---|
| **Timegrapher.AppImage** | The desktop app, in one file |
| **Command Line** | `timegrapher`, the command line tool, and `timegrapher-mcp`, the server that lets AI chat apps use it |
| **User Guide** | The manual, as pages that open in your browser |
| **LICENSE.txt** | The licence (GPL-3.0) |

## Open the App

Double-click **Timegrapher.AppImage**. If your file manager does not run
it, open its **Properties**, then **Permissions**, tick *Allow executing
file as program*, and double-click it again. From a terminal:

```sh
chmod +x Timegrapher.AppImage
./Timegrapher.AppImage
```

If it says FUSE is missing, install `libfuse2` (Ubuntu 22.04:
`sudo apt install libfuse2`; Ubuntu 24.04: `sudo apt install libfuse2t64`)
or run it with `--appimage-extract-and-run`. Sound input goes through ALSA,
which PipeWire and PulseAudio systems provide through their ALSA plugin.

## Use the Command Line Tool

In a terminal in the **Command Line** folder, run `./timegrapher --help`.
Keep `timegrapher-mcp` beside `timegrapher`: the server runs it. The
AppImage carries the tool too: `./Timegrapher.AppImage cli --help`.

## Read the User Guide

The **User Guide** folder holds the manual, as pages that open in your
browser without an internet connection.

- [Contents](../../docs/user-guide/README.md): every button, pane and
  function, what it does, and why you'd use it
- [Your First Take](../../docs/user-guide/02-your-first-take.md): from
  opening the app to a saved take
- [Installing and updating](../../docs/install.md) (on GitHub)
