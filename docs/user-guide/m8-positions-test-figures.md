# M8. Positions Test Figures

## What It Is

A watch runs at a slightly different rate in each position, because
gravity acts on the balance differently in each. A **positions test**
measures it in several positions and sums the differences in a few
figures. The Positions Test pane (chapter 20) gives them, and so does
`timegrapher session`. They follow the figures Witschi's timegraphers
use and the Swiss standard NIHS 93-10 (the project's
[math standards](../math-standards.md) list them).

## The Positions

| Common | Witschi | Group |
| --- | --- | --- |
| DU, dial up | CH | horizontal |
| DD, dial down | CB | horizontal |
| CD, crown down | 9H | vertical |
| CL, crown left | 6H | vertical |
| CU, crown up | 3H | vertical |
| CR, crown right | 12H | vertical |

Witschi's codes count round the dial with the crown at 3 o'clock, so crown
up is 3H and crown right 12H.

## How It Is Worked Out

**Each position** is read as `timegrapher session` reads a stretch: its
measured seconds, after its settle, with any masked time left out. A
position measured more than once is read together, or averaged first in
the indices. A position needs one whole 10 s reading to count.

**In one state of wind** (all positions at the same hours since full
wind), with *r* each position's rate and *a* its amplitude:

```text
X     = mean of r over the positions read            the Average row
XH    = mean of r over the horizontal positions
XV    = mean of r over the vertical positions
D     = largest r − smallest r                       the Largest Difference row
DV,DH = the same among the vertical or horizontal positions
DVH   = XV − XH, and the same for amplitude          vertical less horizontal
Di    = r(6H) − r(CH)                                crown left less dial up
```

The Largest Difference row gives D for the rate, and the same for the
amplitude and the beat error (as a size).

**Across two states of wind,** full wind and 24 hours after, with each
position's change *Δr = r(24 h) − r(full)*:

```text
Im    = the Δr largest in size, crown right (12H) left out, signed
Im*   = the same over all positions             command line only
Ie    = | X(24 h) − X(full) |, each X over the positions read in both
```

## Why This Way

- **Means over positions:** a watch on the wrist turns through many
  positions, so the mean rate X is the nearest single figure to its rate
  worn.
- **The largest difference D:** how much the rate depends on position at
  all. A poised balance with a well-set hairspring gives a small D.
- **Vertical against horizontal, DVH:** in the horizontal positions the
  balance pivots rest on their ends; in the vertical ones on their sides,
  with more friction and a lower amplitude. DVH sums that split.
- **Crown right left out of Im:** as NIHS 93-10 takes it. The
  Positions Test pane shows Im and Ie; the command line's `session` gives
  Im* as well.
- **The same positions in both states for Ie,** so a position measured
  only once can't move the change in X.

## When You'd Use This

- **Regulating:** X says where the watch runs on average; D and DVH say
  how much the positions differ before you adjust the rate.
- **After service:** compare the six positions and D with the figures
  from before.
- **Full wind against 24 hours:** Im and Ie say how much the rate moves
  as the mainspring lets down over a day, in each position and on
  average.

## What It Helps Find

- **A poise error:** it can show as one vertical position far from the
  others.
- **A large horizontal-vertical split** (DVH): the balance pivots or
  jewels.
- **A rate that runs away at 24 hours** (large Im or Ie): isochronism
  (M12).

## What It Can't Tell You

- **Whether the figures pass.** The app gives no tolerance, verdict or
  pass and fail. Compare with the maker's or the standard's limits
  yourself.
- **The cause.** A large D or DVH says the positions differ, not why.
- **Precision beyond the readings.** Each position is one reading, with
  its own ± (M7); a short measurement in a noisy position gives a wide one.

---

Previous: [M7. The ± and Correlated Readings](m7-the-plus-minus.md) ·
Next: [M9. Steadiness Statistics](m9-steadiness-statistics.md)
