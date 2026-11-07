# M5. Beat Error

## What It Is

The **beat error** is how unevenly the two halves of the balance's swing
are timed: half the difference between the Tick-to-Tock and Tock-to-Tick
intervals, in milliseconds.

If one interval is 124.7 ms and the next 125.3 ms, the beat error is
half of 0.6 ms: **0.30 ms**.

It is signed: **+ when the Tick comes late**. The cards and charts show
its size; the sign is in the command line's JSON.

## How It Is Worked Out

There are two figures, timed from two sounds.

**From the unlock** (the big figure):

1. In each 2 s window, the Ticks' sounds are averaged into one shape and
   the Tocks' into another (as for amplitude, M4).
2. The unlock is found on each shape, and timed against the side's beat
   times.
3. The window's beat error is the drop's beat error (below), less the
   Tick's unlock-to-drop time, plus the Tock's.
4. The reading is the **median** of its windows.

**From the drop** (in the Beat Error card's hover):

- The rate fit (M2) puts a line through every beat, with Ticks offset
  `+e/2` and Tocks `−e/2`. `e` is the beat error. A beat's time sits near
  its drop, so this is the beat error timed from the drop.

**Which is shown:** the unlock's, where the unlock was found; else the
drop's, with the chip **Beat Error From the Drop**.

**Which side is Tick:** the sound can't tell which pallet stone is which.
Each take starts positive: the side that comes late over the first few
seconds is called Tick. With a beat error nearer zero than 0.1 ms, the
first beat heard is Tick. A beat error that reads negative later has turned
over during the take.

## Why This Way

- **From the unlock:** the open tg program times the beat error from the
  unlock, and the app is checked against tg in its regression tests, so
  the two can be compared.
- **The drop's figure in the hover:** the drop is louder, so its timing is
  steadier. When the two figures differ, the two sides' unlock-to-drop
  times differ.
- **Signed, but shown as a size:** which side is "Tick" is the app's label,
  so the sign alone says nothing about the watch. A change of sign within
  a take means the two sides' timing turned over, or that Tick and Tock
  swapped names across a break.

## When You'd Use It

- **Setting the beat:** as you adjust it, watch the figure fall. A short
  Average Over follows it quickly.
- **Across positions:** a beat error that changes a lot between positions
  is worth a note.
- **Over a take:** the Steadiness panes show whether it holds.

## What It Helps Find

The beat error shows whether the balance's rest point sits centred in
the escapement, so that the impulse pin is at the fork's centre when the
hairspring is at rest.

## What It Can't Tell You

- **How the watch will keep time.** The rate fit takes the beat error
  out of the rate, so the two are read apart.
- **Through a poor unlock.** Like amplitude, the unlock's figure rests on
  a quiet sound. A muffled or clipped sound moves it.
- **Across a break, a sign.** If the sign differs either side of a cut,
  the whole take gives a size and says so: averaging + and − would read
  near zero.
- **Which pallet stone is late.** Only that the two sides differ.

---

Previous: [M4. Amplitude and Lift Angle](m4-amplitude-and-lift-angle.md) ·
Next: [M6. Averaging, Windows and Cuts](m6-averaging-windows-and-cuts.md)
