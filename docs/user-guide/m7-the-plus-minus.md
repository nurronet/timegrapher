# M7. The ± and Correlated Readings

## What It Is

The **±** beside a figure is its **standard error**: an estimate, from the
take itself, of how far the figure would likely move if the same stretch
were heard again under the same conditions.

It is **precision, not accuracy**. A wrong lift angle or an uncorrected
sound card clock moves the figure without widening the ±.

## How It Is Worked Out

The ± comes from the short readings inside the span, in time order: the
2 s windows for amplitude and beat error, and every other 10 s rate
window for the rate.

```text
d     = each reading less the one before it
s     = 1.4826 × median distance of d from its median ÷ √2
ρ     = lag-one correlation of the readings, after a straight line is
        taken off, held between 0 and 0.99
n_eff = n × (1 − ρ) ÷ (1 + ρ)
±     = 1.2533 × s ÷ √(1 − ρ) ÷ √n_eff
```

In words:

1. **The scatter, `s`,** comes from the differences between neighbouring
   readings, and a median rather than a mean, so one wild window doesn't
   inflate it. The constants turn it into a standard deviation for
   normal noise.
2. **The correlation, `ρ`,** is how much each reading resembles the next.
   Watch readings do: on the project's test recordings, neighbouring 2 s
   amplitude windows correlate by anywhere from 0.01 to 0.90 (the
   [Math Standards Guide](../math-standards.md#12-method-audit),
   section 12).
3. **The effective count, `n_eff`,** counts correlated readings as fewer
   independent ones. At ρ = 0.5, 30 readings are worth 10.
4. **The ±** is the standard error of a median of `n_eff` readings.

**Over several stretches** between cuts (positions, breaks, masks), the
differences `d` are taken within each stretch only, never across a cut.
For amplitude and beat error, a second term is added: how far the
stretches' own medians spread, past what their noise explains.

```text
τ²   = largest of 0 and (variance of the stretches' medians
       − mean of their ±²)
term = 1.2533 × τ ÷ √k     (k: the stretches of 3 readings or more)
±    = √(within² + term²)
```

The live **rate's ±** is read from the 10 s windows since the last break,
up to five minutes, then scaled to the Average Over time. It needs about
30 s of unbroken beats. With fewer than 3 readings there is no ±.

## Why This Way

- **Neighbour differences** ignore a slow, steady drift: a watch winding
  down by a steady 1° a minute doesn't widen the amplitude's ±.
- **Allowing for correlation:** treating correlated readings as
  independent gives a ± too small. On a seeded test at ρ 0, 0.5 and 0.9,
  the share of repeats within ±1 standard error stays between 55% and 85%
  (68% expected).
- **The spread between stretches:** a watch reading 270° in one stretch
  and 250° in the next leaves the take's median uncertain by more than
  either stretch's noise. Stretches at one level add nothing on average.
- **One function for every figure,** in the app, the command line and the
  MCP server, so the same take gives the same ± everywhere.

## When You'd Use It

- **Comparing two readings:** two figures whose ± ranges overlap widely
  aren't shown to differ. Positions, before and after a change, two takes.
- **Choosing Average Over:** a longer span gives a smaller ±.
- **Deciding how many decimals matter:** a rate of +4.2 ± 0.8 s/d isn't
  known to the tenth.

## What It Helps Find

- **A noisy setup:** a ± much wider than usual for the same watch and
  span points to the sound, the contact or the stand.
- **A watch that wanders:** a slow swing or cycle raises ρ and so widens
  the ±.

## What It Can't Tell You

- **Accuracy.** The lift angle and the clock are caveats, not part of the
  ±.
- **The next take.** A watch can read differently after rewinding, a
  knock or an hour. The ± is about this stretch only.
- **Memory of another shape.** The model allows for correlation from one
  reading to the next. A cycle or a level shift is allowed for only as far
  as it shows in that one value, so the ± can still be too small.
- **A guarantee.** It is an estimate under a model of the noise, not a
  bound on the error.

The project's [precision notes](../measurement-precision.md) give the
model, its sources and the tests behind it.

---

Previous: [M6. Averaging, Windows and Cuts](m6-averaging-windows-and-cuts.md) ·
Next: [M8. Positions Test Figures](m8-positions-test-figures.md)
