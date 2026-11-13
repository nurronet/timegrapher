# M12. Isochronism and Rate against Amplitude

## What It Is

A balance is **isochronous** when its period doesn't depend on how far it
swings. A real one isn't quite: the hairspring, the escapement's impulse
and the balance's poise all make the period move a little with the
amplitude. As the mainspring lets down, the amplitude falls, so a watch's
rate moves through the day with it.

The Isochronism pane (chapter 19) measures that: the rate at each
amplitude, the slope of rate against amplitude, and where the rate
changes least. The Positions Test's full wind against 24 hours (chapter
20, M8) measures it another way: the change in each position between two
states of wind.

## How It Is Worked Out

**Pairs.** Each 10 s rate reading is paired with an amplitude for the
same span. A rate reading is the slope of a line fitted through its
beats, and such a fit weighs the beats near the middle of the span most:
6u(1 − u) at the fraction u of the way through. So the paired amplitude
is the mean of the reading's 2 s windows, each weighted the same way:

```text
weight of a window from u₀ to u₁ = F(u₁) − F(u₀),   F(u) = 3u² − 2u³
five windows: 0.104, 0.248, 0.296, 0.248, 0.104
```

A short event inside a reading then moves the amplitude it is paired
with as far as it moves the rate.

**Steps and curve.** The pairs are sorted into 2° steps of amplitude
between the 2nd and 98th percentile, each step with at least 10 pairs,
and at least 3 steps. Each step gives its median rate and its quartiles.
A quadratic is fitted through the medians, each weighted by its pairs:

```text
rate ≈ c₀ + c₁ (A − Ā) + c₂ (A − Ā)²
turn at A = Ā − c₁ ÷ (2 c₂)       shown when inside the middle 80%
```

**The slope.** A straight line through every pair, by least squares,
after leaving out any rate or amplitude reading over 6σ from its
5-minute running median (none when more than 3% would go), as the
Steadiness views do. But
the amplitude readings are noisy, and noise in the quantity along the
bottom always pulls a fitted slope towards 0. The slope is corrected for
it (errors in variables, Fuller 1987):

```text
σ_w²   = (1.4826 × MAD of neighbouring window differences ÷ √2)²
noise  = σ_w² × Σ w²                (about 0.232 σ_w² for five windows)
λ      = 1 − noise ÷ variance of the paired amplitudes
slope  = least-squares slope ÷ λ
```

*λ* is the **reliability**: the share of the amplitudes' spread that is
real. The slope is withheld when *λ* is under 0.5, and the pane says the
amplitude readings' own noise is half or more of their variance. The ±
allows for correlated readings, as in M7.

**The walk.** One point per 5 minutes: the median rate of its 10 s
readings and the median amplitude of its 2 s windows, each read on its
own. Each follows the floors every window figure does: read from under
10% of its readings or windows it is withheld, and a 5 minutes with
either withheld is left out; under 60% it stands and is drawn faint, its
hover giving the counts. Only full points set the axes while two stand, so
a faint point off their range is cut at the plot's edge and counted in the
key. Past 250
points (a take over about 20.8 hours) neighbouring 5 minutes are merged,
10, 15 … minutes a point: each series the mean of the joined medians,
weighted by their counts.

## Why This Way

- **Medians in steps:** a few wild readings don't move a step's rate.
- **The slope through every reading:** none is smoothed away, so a short
  swing of amplitude still counts.
- **The noise correction:** without it a noisy amplitude would make any
  watch look more isochronous than it is.
- **Weights matching the rate's fit:** a median or a plain mean of the
  windows moves less than the rate does in a short event, and biases the
  slope by about 10% on a rub of 8 s in 10 (the project's
  [series notes](../reference/series.md)).
- **The walk:** the curve throws time away; the walk keeps it, so a step
  or a return shows.

## When You'd Use This

- **On a run-down:** a watch fully wound and left to run for hours
  covers the amplitudes it meets in a day.
- **After adjusting the hairspring, the balance or the escapement:**
  compare the slope and the turn with a take from before.
- **Judging a positions test:** a rate difference between positions comes
  partly from their different amplitudes.

## What It Helps Find

- **How much the rate moves through a day's wind:** the slope times the
  fall in amplitude.
- **The amplitude where the watch is steadiest:** the turn.
- **A change in the watch** between two takes, with the same slope read
  the same way.

## What It Can't Tell You

- **Cause.** The rate and the amplitude went together; temperature,
  position or anything else that changed over the take moves with them.
- **The shape beyond the range read.** The curve is fitted only over the
  amplitudes the take covered.
- **A guarantee.** The noise correction assumes the window noise is
  independent from window to window.

---

Previous: [M11. Time and Frequency Maps](m11-time-and-frequency-maps.md) ·
[Contents](README.md)
