# M6. Averaging, Windows and Cuts

## What It Is

A reading is never one beat. It is built in layers, each made only from
the one above it:

```text
Sound
  ↓ beats found (M1)
Beats: time, number, quality
  ↓ placed on true time (M3); cuts and masks applied
Rate fit: one line through every good beat in the span (M2)
10 s rate windows: a rate fit every 5 s, overlapping by half
2 s windows: each side's beats averaged into one shape (M4, M5)
  ↓
Reading: rate as fitted; amplitude and beat error as the median of
the 2 s windows; each with its ± (M7) and chips
```

## How It Is Worked Out

**The span.** While listening, a reading covers the last **Average Over**
seconds (10 s unless changed). For a stopped or opened take, the whole
take's reading covers each position's measured stretch.

| Window | Length | Gives |
| --- | --- | --- |
| **2 s window** | 2 s of beats, laid end to end | Amplitude, Tick and Tock; the beat error from the unlock |
| **10 s rate window** | 10 s, one every 5 s | The rate's ±, the 5th to 95th percentile of rates, the rate chart |

**Cuts.** A cut is any place no reading may cross:

| Cut | What makes one |
| --- | --- |
| **Pause** | Listening stopped and started again, or the computer slept. |
| **Jump** | A sudden step in the beats' timing: a knock, lost sound. |
| **Position change** | The watch turned to another position. |
| **Mask** | A stretch you masked by hand on the Paper Strip. |
| **Pause between beats** | More than 2 s with no beat. |
| **Clock step** | A step of 0.5 s or more in the clock log that no listed pause explains. |

Two rules cover them all:

- **Nothing is fitted, windowed or averaged across a cut.** Each stretch
  between cuts is read on its own. The rate and the drop's beat error
  combine the stretches, weighted by the beats each used; amplitude and
  the beat error from the unlock are the median over every 2 s window of
  every stretch.
- **Nothing is filled in.** A window that reaches into a cut is left out.
  A window short of a beat or two is made from the beats it has.

**Settling.** After each change of position, the first seconds aren't
read while the balance settles: 20 s by default, but never more than half
the position's time. A settling time you set is used as it is.

**Short stretches.** A stretch between cuts whose beats span under 10 s
gives no rate: a few seconds of beats give a rate several s/d out.

## Why This Way

- **Medians of windows** for amplitude and beat error: a window with an
  edge marked on the wrong sound moves one window, not the reading.
- **A fit for the rate:** it uses every beat, and needs no windows.
- **No reading across a cut:** after a knock or a turn, the beats' count
  and the watch itself may have changed. A reading across it would mix
  two states and call it one.
- **No filling:** an invented value looks like a measured one. A gap
  shows as a gap.

## When You'd Use It

- **Choosing Average Over** (chapter 5): a short span follows a change, a
  long one steadies the figure.
- **Masking** a knock or a handling stretch on the Paper Strip, to keep it
  out of every reading.
- **Reading a whole take** in the cards' hover, settled and in one
  position.

## What It Helps Find

- **A knock or lost sound,** as a jump the readings skip.
- **A reading resting on little,** from its chips: **6 s of Beats**,
  **540 of 600 Beats**, **Read from 4 of 15 Windows**.

## What It Can't Tell You

- **What happened inside a cut.** Masked or lost time is not read.
- **A change faster than the span.** A 10 s reading averages anything
  quicker than 10 s.
- **Whether the windows are independent.** Neighbouring readings often
  resemble each other; the ± allows for that (M7).

---

Previous: [M5. Beat Error](m5-beat-error.md) ·
Next: [M7. The ± and Correlated Readings](m7-the-plus-minus.md)
