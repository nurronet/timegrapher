# M9. Steadiness Statistics

## What It Is

Steadiness (chapters 9 to 18) runs a family of tests on the readings of
each series in time order, and names what passes. This note gives the
statistics behind the Info panel's figures and the views: the scatter,
the outlier rule, the Allan deviation, the trend, the CUSUM, the change
finder and Holm's correction. The period search has its own note (M10),
and the autocorrelation is in chapter 13.

## The Figures in the Info Panel

With *y₁ … yₙ* the readings in time order and *ȳ* their mean:

```text
median          the middle reading
sd              √( Σ (yᵢ − ȳ)² ÷ (n − 1) )
reading to      √( ½ × mean of (yᵢ₊₁ − yᵢ)² )
  reading
lag-1 r         how much each reading resembles the next (chapter 13)
```

The **scatter from one reading to the next** uses only neighbours, so a
slow change (a drift, a step) hardly moves it, where it widens the sd.
When the sd is far above it, something slow is going on.

**Outliers.** A reading more than **6 σ** from the 5-minute running
median of the readings is left out, σ the larger of the scatter between
neighbours and the spread about that median. If that would take out more
than 3% of the readings, none is left out: that many are part of what the
watch does, a cycle or a second state, not strays.

## The Allan Deviation

For an averaging time of *m* readings, with *ȳₖ* the mean of the *m*
readings starting at reading *k*:

```text
σ(τ) = √( ½ × mean over k of (ȳₖ₊ₘ − ȳₖ)² )      τ = m × 10 s
white(τ) = σ(10 s) ÷ √m
```

*k* runs over every reading (the overlapping Allan deviation), and an
average counts only when all its *m* readings are there. *m* doubles from
1 while (pairs ÷ *m*) is at least 3. The least scatter is picked among the
times with at least 8; the readings count as wandering where σ(τ) is at
least twice white(τ) at any of them.

**Why this way:** the ordinary sd of a series that wanders depends on how
long the take is, and never settles. The Allan deviation compares each
average with the next, so it settles for wander and drift as well, and
says at what averaging time each kind of noise takes over. It is the
standard measure of frequency stability for clocks and oscillators (IEEE
Std 1139). For independent readings it falls as 1/√τ, the dashed line; a
cycle of period P raises it most near τ ≈ 0.37 P.

## The Trend

A straight line and a quadratic are fitted to the readings against time.
The quadratic also covers a rise and then a fall.

```text
ρ     = lag-1 correlation of the readings about their line
n_eff = n × (1 − ρ) ÷ (1 + ρ)
p     = (1 − R²) ^ ((n_eff − 3) ÷ 2)
```

*R²* is the share of the readings' variation the quadratic explains, and
*p* is its F test against a flat mean, on *n_eff* readings in place of
*n*. Readings that follow each other wander far enough for a curve to fit
a good share of them by chance; counting them as fewer allows for that.
A **slide** is named when the trend explains at least 20% and *p* passes
Holm's correction (below).

## The CUSUM

```text
σ_LR  = the long-run scatter: the reading-to-reading scatter
        × √(1 + ρ) ÷ (1 − ρ), ρ the lag-1 correlation read with the
        largest step out
Sₖ    = Σᵢ₌₁ᵏ (yᵢ − ȳ) ÷ (σ_LR × √n)
D     = max |Sₖ|
p     = 2 × Σⱼ₌₁^∞ (−1)ʲ⁻¹ × e^(−2 j² D²)
```

With no change of level, *Sₖ* behaves like a **Brownian bridge**, a random
walk tied to 0 at both ends, and *D* follows the **Kolmogorov
distribution**: *p* is the chance of an excursion as large as *D*.
*D* = 1.36 gives *p* = 0.05, the dashed lines.

**Why the long-run scatter:** with the ordinary sd, a step would widen the
scale and hide itself. With the reading-to-reading scatter alone,
correlated readings would cross the lines far too often. For readings
that each follow the last by ρ, the neighbour scatter is σ√(1 − ρ), so
the factor makes it σ√((1 + ρ) ÷ (1 − ρ)): how far the mean of *n* such
readings scatters, times √*n*. The correlation
is read once the largest step is out (Page 1954 for the CUSUM; the
correlation allowance as in M7).

## The Change Finder

The readings are cut into 30 s blocks and each block's median taken. The
finder splits the blocks where splitting most reduces the squared
distances from the stretches' means (**binary segmentation**), then
splits each part again:

```text
gain   = reduction in Σ (block − its stretch's mean)² ÷ σ_b²
keep a split when  gain > max(3 × ln(blocks), 15)
and each stretch is at least 5 minutes
```

*σ_b* is the block medians' long-run scatter, allowing for their
correlation as the CUSUM's does. The
penalty only places the cuts; whether the level changed is the CUSUM's
test.

## Holm's Correction

Each take runs many tests: for each of three series, the period search's
lines, the Ljung–Box test, the trend's F test and the CUSUM. At 0.05
each, one of them would pass by chance on many steady takes. **Holm's
correction** holds the whole family to 0.05:

```text
sort the m p-values: p₍₁₎ ≤ p₍₂₎ ≤ … ≤ p₍ₘ₎
keep p₍ᵢ₎ while p₍ᵢ₎ ≤ 0.05 ÷ (m − i + 1); stop at the first that fails
```

A finding is named only when its own test and Holm's both keep it (Holm
1979). It controls the chance of any false finding at 0.05, and is never
less powerful than dividing by *m* throughout. On the project's seeded
steady 2-hour takes, the family names nothing on 192 takes in 200.

## When You'd Use This

- **Reading the Info panel's figures** behind a headline.
- **Judging a finding near the edge:** a p just past its threshold, a
  trend explaining 21%.
- **Comparing two takes** with the same statistics.

## What It Can't Tell You

- **Whether a finding matters for the watch.** The thresholds are for
  detection: how sure the tests must be. None is a tolerance.
- **A guarantee.** Each test rests on a model of the noise; readings that
  break it (bursts, a changing sound) can pass or fail wrongly.

---

Previous: [M7. The ± and Correlated Readings](m7-the-plus-minus.md) ·
Next: [M10. Finding Cycles](m10-finding-cycles.md)
