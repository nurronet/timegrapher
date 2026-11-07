# M3. The Sound Card's Clock

## What It Is

The sound card counts samples by its own quartz crystal. If the crystal
runs fast, the card's second is short, and every beat seems to come a
little late: the watch reads slow. The error is given in **parts per
million (ppm)**, + when the card runs fast.

| Card error | Rate error |
| --- | --- |
| 1 ppm | 0.09 s/d |
| 10 ppm | 0.86 s/d |
| 20 ppm | 1.73 s/d |
| 50 ppm | 4.32 s/d |

A sound card's crystal is commonly 10 to 50 ppm off, and holds that error
steadily. Two sound cards can read the same watch a few s/d apart.

## How It Is Worked Out

A take's readings are made on one clock, picked once from what the take
has, in this order:

| | Source | What it gives |
| --- | --- | --- |
| 1 | **Clock log rated by USB lines** | The log's pauses and steps, with the USB lines' steady error. |
| 2 | **Clock log** | Pairs of sound card time and computer time, written while recording. A fitted line follows drift and steps. |
| 3 | **USB lines** | A USB microphone picks up its computer's bus, which sends a packet every millisecond. Faint lines at exact multiples of 1 kHz land in the sound, timed by the computer. Where they fall on the card's clock gives the card's error. |
| 4 | **Known correction** | One set for the take, or the one kept for that input by Quartz Correction. |
| 5 | **None** | The readings stay on the card's clock. |

- **Quartz Correction** (Settings > Microphone, chapter 3) times the card
  against the computer's clock while you listen, and offers to keep the
  error once it is known to half a ppm, after 20 minutes or more.
- **The correction:** a card `ppm` fast makes rates read slow by about
  `ppm × 0.0864` s/d. The app corrects exactly:
  `(1 + rate/86,400) ÷ (1 − ppm × 10⁻⁶)`, less 1, times 86,400.
- **True time** is the card's time mapped through the take's clock, with
  each pause's length added. Every reading, window and hour is placed on
  it.

## Why This Way

- **The computer's clock is kept right by internet time (NTP),** so over
  minutes it is a far better reference than the card's crystal.
- **USB lines need no log,** so a recording made in another app can still
  be corrected, if it came through a USB microphone.
- **The Rate card's big figure stays on the card's clock** while you
  listen, the clock other timegraphers and `timegrapher analyze` read on,
  so figures can be compared with theirs. The corrected rate is in the
  hover, as **True Clock**.

## When You'd Use It

- **Comparing takes from different microphones** or computers: read the
  True Clock figures, or the error between them can be the cards'.
- **Long takes and Power Reserve tests,** where hours are counted: they
  are counted on true time.
- **Checking a watch against a reference:** only a corrected rate is
  comparable with a time signal or a certified figure.

## What It Helps Find

- **A rate difference that is the card's, not the watch's,** between two
  setups.
- **Pauses and sleeps** in a long take, from the clock log.
- **Disagreement** between the USB lines and the clock log, by more than
  5 ppm and 2 of the log's standard errors, or between the USB lines and
  a kept correction by more than 5 ppm: the app says so in a caveat,
  since one of them is wrong.

## What It Can't Tell You

- **Without a log, lines or a kept correction,** nothing: the readings stay
  on the card's clock, and the Rate card's ? says the input's clock isn't
  measured.
- **A busy computer** delivers sound less evenly, so Quartz Correction's
  offer can come later.
- **The computer's own error.** On the stored takes, the USB lines and
  the internet-timed clock logs agree to about 1 ppm (0.1 s/d). A
  computer clock not kept by internet time carries its own crystal's
  error into the log.

---

Previous: [M2. Rate](m2-rate.md) ·
Next: [M4. Amplitude and Lift Angle](m4-amplitude-and-lift-angle.md)
