# M13. Dynamic Poise

## What It Is

The Dynamic Poise pane (chapter 20b) makes two fits over the upright
positions, and keeps them apart:

- **The measured wave**, a fact: how the rate moves once round as the
  watch turns in its own plane, its size and its fastest hour.
- **The heavy spot**, an inference: which way the balance's heavy side
  hangs, worked out from that wave and each position's amplitude.

Both are core's `poise::fit`.

## The Angle

Each upright position is named by the hour mark at the top, and its angle
`α` is measured clockwise from 12H: 12H is 0°, 1H30 45°, 3H 90°, 6H 180°,
9H 270°, and so on. The angle a fit gives is turned back into the nearest
such hour for the words ("nearest 3H at the top").

## The Measured Wave

```text
rate(α) = m + A × cos(α − φ)
```

- `m` is the mean rate, `A` the size of the wave each way from the mean
  (half its peak to peak), and `φ` the angle at which the watch ran
  fastest.
- It is fitted by least squares as `m + a × cos α + b × sin α`, so
  `A = √(a² + b²)` and `φ = atan2(b, a)`. Three unknowns: three
  positions fix it, and the pane draws it from five, leaving two to
  check its shape.
- **Weights:** each position by 1 / SE², its rate's standard error
  squared (M7), so a noisy position counts for less. The ± of `A` comes
  from the fit's covariance. The ± of `φ` is found by test inversion:
  the fit's error across the true direction is the same whatever `A` is,
  so the 95% half-width is `asin(1.96 × σ⊥ ÷ A)`, shown as one standard
  error (divided by 1.96). A small wave gives a wide angle. With any
  position lacking a ±, the fit is unweighted and gives none.
- **Clear of the noise:** the fastest hour is named only when `A` is at
  least 2 standard errors from zero. Otherwise the line says the wave is
  "not clear of the noise", and names no hour.

**The twice-round check.** Beside the wave, the fit adds `cos 2α`, where
the positions leave room for a fourth unknown, and `sin 2α` where they
leave room for a fifth (at 90° steps `sin 2α` is zero at every position,
so only a 45° test gives it). These are no part of the answer. Poise
gives only the once-round wave; pivots and the hairspring can give a
twice-round one, so a large twice-round term says the wave isn't all
poise. Info gives both, in s/d.

## The Heavy Spot

A heavy spot of size `U` (in s/d, as a small swing would show it), lowest
at rest with angle `ψ` at the top, moves the rate by

```text
U × f(θ) × cos(α − ψ),     f(θ) = 2 × J1(θ) ÷ θ
```

where `θ` is that position's amplitude in radians and `J1` is the Bessel
function of the first kind, order one. `f` says how much of a small
swing's poise effect is left at amplitude `θ`:

| Amplitude | f(θ) |
| --- | --- |
| 90° | 0.72 |
| 180° | 0.18 |
| 200° | 0.08 |
| 219.5° | 0 |
| 230° | −0.036 |
| 250° | −0.088 |
| 270° | −0.12 |
| 300° | −0.13 |

So the same heavy spot makes the watch fastest with it at the bottom
below 219.5°, and slowest above it; at 219.5° (3.8317 radians, the first
zero of `J1`) poise doesn't move the rate at all, and near it barely.

- **The fit:** `m + c × f(θᵢ) cos αᵢ + d × f(θᵢ) sin αᵢ`, with each
  position's own amplitude `θᵢ`, weighted as the wave is. Then
  `U = √(c² + d²)` and `ψ = atan2(d, c)`: the line reads "Heavy spot hangs
  lowest with 4H30 at the top (…°)", with the ± of `ψ`.
- **The amplitude's own noise** moves `f(θᵢ)`. Its share,
  `U × f′(θᵢ) × SEθᵢ × cos(αᵢ − ψ)`, is added to each position's rate ±
  from a first fit, and the fit is made once more.
- **It rests on the lift angle.** Amplitude is worked out with the lift
  angle (M4), so a lift angle set wrong moves every `θᵢ`, and `f` with
  them.
- **Stated only when `U` is clear of zero by 2 standard errors.**

## The Band Near 219.5°

The heavy spot isn't stated when any position's amplitude may be near
219.5°: when the range from 2 standard errors below its amplitude to 2
above, widened by 8% of the amplitude either way, holds 219.5°. With no
± that is from about 203° to 239°.

The 8% is for the lift angle. A catalogue lift angle can be several
percent out, and amplitude scales with it: the one lift angle measured
for this project moved 7.3%, from 55° to 59°. Across 219.5° the sign of
`f` flips, so an amplitude carried across it by a wrong lift angle would
turn the heavy spot round.

Inside the band only the measured wave is shown. Run the test where the
amplitude is well away from it: let down below 203°, or fully wound above
239°.

## Amplitude Round the Dial

A heavy spot barely changes the amplitude; friction, which differs by
position, does. If the amplitude itself has a once-round wave, the
watch's isochronism (M12) turns it into a once-round wave in the rate
too, in the same pattern as poise.

So the amplitudes are fitted with the same once-round wave. Where its
size is clear of zero by 2 standard errors, the line says so: "Amplitude
also changes once round, … each way, so part of the wave may be
isochronism." The pane does not take the isochronism off the rates.

## Why This Way

- **Two fits, not one:** the fastest hour needs nothing but the rates,
  so it is a fact. The heavy spot needs the amplitude, the lift angle and
  the sign of `f`, so it is an inference, and is withheld where those
  can't settle it.
- **Each position's own amplitude:** in a long test the amplitude falls,
  and positions differ. One amplitude for all would put the wrong `f`
  on most of them.

## What It Can't Tell You

- **A mass.** `U` is in s/d, as a small swing would show it, not in
  milligrams.
- **The heavy spot near 219.5°,** or where the wave is within its noise.
- **Effects that look like poise:** a once-round wave in the amplitude,
  through isochronism, is named but not taken off.

---

Previous: [M12. Isochronism and Rate against Amplitude](m12-isochronism-and-rate-against-amplitude.md) ·
[Contents](README.md)
