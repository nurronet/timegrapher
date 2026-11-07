# M11. Time and Frequency Maps

## What It Is

Three views break a series into periods: the **FFT** (Fast Fourier Transform, chapter 16) over the
whole take, and the **STFT Map** (Short-Time Fourier Transform) and **Wavelet Map** (chapters 17 and 18)
at each time of it. This note gives how each size is worked out, and
what each method trades.

All three give a period's size as a **sine's size**: half its peak to
peak, in the series' unit. For the rate that is the beat timing, in ms,
except the FFT's default, which converts it to the swing in rate readings
(below).

## The FFT: a Sine Fitted at Each Period

At each period *P*, a sine and a constant are fitted by least squares to
the samples present, each weighted by a **Hann window** over the whole
take:

```text
w(t)   = sin²(π t ÷ T)                   T the take's length
fit    yᵢ ≈ c + a cos(2π tᵢ/P) + b sin(2π tᵢ/P), weighted by w(tᵢ)
size   = √(a² + b²)
```

With every sample present this closely matches the ordinary
Hann-windowed FFT. With
samples missing it is the weighted **Lomb–Scargle** periodogram (its sums
read off transforms, as Press and Rybicki 1989): nothing is filled in,
and a line keeps its size.

**Why the Hann window:** a take is cut off at its ends, and an unwindowed
spectrum spreads a strong line over the whole axis. The window tapers the
ends, so a strong line stays narrow and a weak one beside it still shows.

**The rate's swing in readings.** A timing sine of size *a* ms at period
*P* moves the rate by its slope:

```text
rate swing, peak to peak = 2 × a × 2π/P × 86.4   s/d   (a in ms, P in s)
kept by a 10 s reading   = 3(sin x − x cos x) ÷ x³,  x = π × 10 ÷ P
```

86.4 turns ms per second into s/d. A rate reading is the slope of a line
fitted through its 10 s of beats, and that fit keeps the share above of
a swing at period *P*: nearly all of a slow one, 89% at 30 s, 77% at 20 s. (A
plain 10 s average would keep sin *x* ÷ *x*, 18% less at 20 s.)

## The STFT Map: the Same Window for Every Period

```text
window   20 min of readings, every one present
step     5 min
each     its straight line taken off, a Hann window, its spectrum read
rows     48 periods, even on a log scale, from 4 readings to 400 s
```

Each row reads the strongest frequency within its band (halfway, on the
log scale, to the rows either side), so a line between two rows still
reads within 20% of its size.

**What it trades:** every period is read over the same 20 minutes, so
sizes compare fairly from row to row and window to window. But a short
event is spread over a whole window, and the longest period is a third of
the window.

## The Wavelet Map: a Window to Suit Each Period

The **Morlet wavelet** is a wave inside a bell-shaped envelope:

```text
ψ(t) = π^(−1/4) × e^(i ω₀ t) × e^(−t²/2),   ω₀ = 6
```

Stretched to each scale and slid along the readings, it reads each period
over about the same number of its own turns: short periods over a short
time, long periods over a long one.

**Synchrosqueezing** then moves each value from the period it was read
at to the period its own phase turns at. A line read spread over several
rows is pulled in to near its true period.

- **Each unbroken stretch alone,** of at least 64 readings; nothing is
  read across a gap.
- **The cone of influence:** near a stretch's ends a period's wavelet
  would reach past the readings, so those cells are left empty. Longer
  periods need more room, so the read area is a dome.
- **Periods** from 4 readings up to an hour, or a quarter of the longest
  stretch, whichever is shorter. A stretch of over 65,536 readings is
  averaged down in whole blocks first, and the map is at most 400 columns.

**What it trades:** it places a fast swing in time to within a few of its
turns, and reaches long periods. But under noise, synchrosqueezing spreads
a line over neighbouring rows, so one row reads under the line's size.
Read the size from the STFT Map, and where and when from the Wavelet Map.

## The Colours

Both maps colour from 0 (dark purple) up to the size 99.5% of the cells
read stay under (yellow), on the **viridis** scale, which reads evenly in
lightness and in greyscale. Taking the 99.5% point keeps one bright cell
from darkening the rest. **dB Scale** colours 20 × log₁₀ of the size
against the brightest, over at most 300 to 1 (about −50 dB).

## When You'd Use This

- **Comparing sizes** across periods or takes: the FFT and the STFT Map.
- **Finding when** a swing changed: the Wavelet Map.
- **Checking the rate's FFT against the timing:** the conversion above
  says how much of a timing swing a rate reading keeps.

## What It Can't Tell You

- **Whether a line is real.** These are sizes, not tests. The period
  search decides (M10).
- **Anything across a gap:** each method leaves it unread rather than
  guess.
- **The cause.** A period's swing says what repeats, not why.

---

Previous: [M10. Finding Cycles](m10-finding-cycles.md) ·
Next: [M12. Isochronism and Rate against Amplitude](m12-isochronism-and-rate-against-amplitude.md)
