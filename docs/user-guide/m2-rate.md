# M2. Rate

## What It Is

The **rate** is how many seconds a day the watch would gain (+) or lose
(−) if it kept running exactly as it does over the reading.

## How It Is Worked Out

The app fits one straight line through the good beats' times against
their numbers:

```text
time of beat k = t0 + P × k ± e/2
```

- `P` is the watch's beat period, the time from one beat to the next.
- `e/2` is added to Ticks and taken from Tocks: half the beat error, so
  the two sides' uneven timing doesn't tilt the line (M5).
- The line is fitted by least squares, then fitted twice more without the
  beats over 5 robust standard deviations from it.

The rate compares `P` with the beat period the movement is made for,
3,600 s over the beat rate:

```text
rate = (nominal period ÷ P − 1) × 86,400 s/d
```

**An example.** At 28,800 bph the nominal period is 125 ms. A watch
gaining 10 s/d beats every 124.9855 ms: each beat comes about
14.5 microseconds sooner than the one before would suggest. Over 10 s,
80 beats, that adds up to 1.2 ms, which the fit can see.

**Jitter**, in the Rate card's hover, is the robust standard deviation of
the beats about the line, in microseconds: the spread of the dots across
the strip.

**Over several stretches,** separated by breaks, each stretch is fitted on
its own, and the rates are averaged weighted by the beats each used. A
stretch whose beats span under 10 s gives no rate.

## Why This Way

- **A line through every beat** uses all of them. Timing one beat to the
  next would leave each rate at the mercy of two beats' jitter.
- **The beat error in the line** stops it from reading as a rate. This is
  the approach Witschi describes for its timegraphers.
- **Dropping beats far from the line** keeps a knock or a mis-found beat
  from pulling the rate.
- **No line across a break:** after a pause, a jump or a turn of the
  watch, the beats' count and phase don't carry on, and the watch may
  have changed.

## When You'd Use It

- **Regulating:** read the rate in each position you regulate in, with
  Average Over long enough to steady the last digit.
- **Comparing positions:** the Positions Test reads one rate per position
  (chapter 20).
- **Following a change:** a short Average Over shows the rate move as you
  turn the regulator.

## What It Helps Find

- **Positional error:** rates that differ between positions.
- **A rate that drifts** as the mainspring runs down, or swings as a wheel
  turns (the Steadiness panes).
- **Jitter** much above the take's usual shows a poor sound, a stand
  noise, or a watch whose timing is genuinely uneven.

## What It Can't Tell You

- **The rate on the wrist.** It changes with position, wind and
  temperature. A reading is one position, at one state of wind.
- **The sound card's error.** The rate is measured on the sound card's
  clock, which is commonly 1 to 4 s/d off. See [M3](m3-the-sound-cards-clock.md).
- **A rate between beats.** The fit can't see changes faster than its
  span, the Average Over time.

---

Previous: [M1. Beats](m1-beats.md) ·
Next: [M3. The Sound Card's Clock](m3-the-sound-cards-clock.md)
