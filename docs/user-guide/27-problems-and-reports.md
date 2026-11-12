# 27. Problems and Reports

**What it covers:** what the app records about itself as it runs, what
it does after it closes unexpectedly, how to make a problem report, and
what to do if the window won't open.

**Why you'd use it:** a reading that looks wrong is far easier to put
right with the sound that gave it. A report holds that sound, the
figures the app showed, the settings and the log in one zip, with your
name and computer's name taken out, so the problem can be heard and
measured again on another computer.

## Report a Problem

**Help > Report a Problem…** opens the form.

| Part | What it does | Why you'd use it |
| --- | --- | --- |
| **What Happened** | What you did, what you saw and what you expected to see. | The one thing only you know. |
| **Kind** | **Bug** (something went wrong in the app), **Reading** (a rate, amplitude or beat error that looks wrong), **Setup** (the microphone, the level or the stand) or **Idea** (something the app could do). | So it reaches the right place. |
| **Include the Sound** | Adds up to five minutes of the sound, cut without changing a sample: the last five minutes of a live session, or five minutes of the recording open, chosen with **Ends At**. | Without the sound a reading can't be checked. |
| The handles | Drag either end of the waveform to keep only the part that shows the problem; **Play** plays it. | A shorter report, and only the part that matters. |
| **I listened to this sound…** | Says you listened and it holds nothing private, such as voices. **Save Report…** waits for this tick when the sound is included. | The sound can hold a room's talk. |
| **Offer it to the public test corpus** | Offers the excerpt to the test recordings anyone may use (CC0, Creative Commons Zero: given to the public domain). | It helps test the app on more watches. Only the person who made the recording should tick it. |
| **Save Report…** | Saves the zip where you pick (it starts at Downloads), then opens its folder and the GitHub form. | Drag the zip into the form. |

A take kept as its analysis alone has no sound to include: its report
carries its beats and amplitude windows instead.

**The app sends nothing.** You attach the zip to the issue yourself,
and can open it first to see what is in it.

## What the Zip Holds

| Part | What it is |
| --- | --- |
| The excerpt | The sound you kept, as FLAC, with the same samples as the take. |
| Its measurement | `timegrapher analyze` of the excerpt, which can be run again on another build (below). |
| The app's readings | What the app showed, and its settings, in `results/app.json`. |
| The take's beats | The whole take's beats and amplitude windows, in `analysis/`, so every reading and chart of it can be read again, not only the excerpt's. About 0.3 MB an hour: the Tick Tock profiles are left out. |
| The log and crash records | From the last week. |
| The clock corrections | Each sound input's stored clock error (M3). |
| `manifest.json` | The version, the system, your note, the kind, each file with its checksum, what you agreed to, and what was taken out. |

**Before anything is written,** your home folder becomes `~`, and your
user name and the computer's name become `<user>` and `<host>`, in the
note, the results and the logs. The manifest counts what was taken out,
so a maintainer knows something was there.

## From the Command Line

`timegrapher report` makes the same zip without the app:

```sh
timegrapher report take.flac --at 120 --span 300 --lift 59 --note "reads 310°, the shop's timegrapher says 265°"
timegrapher report --kind bug --note "the app froze when I unplugged the microphone"
```

`--at` and `--span` choose the excerpt, in seconds; `--no-audio` leaves
it out, and `--no-analysis` leaves out the take's beats, which saves
reading the whole recording again. Give `--bph` and `--lift` as they were set when the problem
showed. `--listened-back` and `--cc0` are the form's two ticks, for you
alone to give. It never opens the microphone.

`timegrapher report --replay <zip>` runs a zip's measurement again with the
build you have, and lists each value that moved, with its value then
and now. It is how a maintainer checks that a fix changed what it
should, and nothing else.

## The Log and Crash Records

The app keeps a log, `logs/timegrapher.log`, in its settings folder:

| System | Settings folder |
| --- | --- |
| Linux | `~/.config/timegrapher` |
| Mac | `~/Library/Application Support/timegrapher` |
| Windows | `%APPDATA%\timegrapher` |

It records the version, the microphone opened, level changes, the beat
found and lost, position changes and every message the app shows. It is
kept under 1 MB, with the two before it.

**If the app closes unexpectedly,** it writes `crash-<time>.txt` in the
settings folder itself (not in `logs/`), with the message, where it happened and the last 200 lines of
the log. The next start shows **Closed Unexpectedly…** in the status
bar, which offers:

| Button | What it does |
| --- | --- |
| **Make a Report…** | Copies the record and opens a new GitHub issue to paste it into. The crash isn't offered again. |
| **Show the Record** | Shows what the app wrote down as it closed, in the message itself. |
| **Dismiss** | Closes the message. The crash isn't offered again; its record stays in the settings folder. |

A test's sound not yet saved when it closed is offered too (chapter 23).

## If the Window Won't Open

The window draws with OpenGL 2.0 or newer. Where it can't, such as in a
virtual machine or remote desktop without 3D graphics, or with a
graphics driver missing, the app tries again with Direct3D 12 on
Windows, Metal on a Mac or Vulkan on Linux. If neither works, the app
says why in the log and, on Windows and a Mac, in a message box. The
command line (chapter 25) needs no window, and can still read and
record.

## What It Can't Tell You

- **Whether the problem is the watch, the stand or the app.** A report
  gives a maintainer what's needed to find out.
- **Anything you leave out.** Without the sound a reading can't be
  measured again; with only the last five minutes, an earlier moment
  is lost.

---

Previous: [Claude Desktop](26-claude-desktop.md) ·
Next: [Glossary](28-glossary.md)
