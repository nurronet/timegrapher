# 19. Isochronism

![Isochronism, the Curve view, over a whole run-down from full wind to a stop, marks 1 to 7](images/isochronism.png)

**What it shows:** the rate at each amplitude. As the mainspring lets
down, the balance swings less, and a watch's rate moves with it. The pane
sorts the take's 10 s rate readings by the amplitude of the same reading,
2° at a time, and shows the rate in each step and the curve through
them.

**Why you'd use it:** a balance that kept the same rate at every
amplitude would be **isochronous**. None is quite, and how far the rate
moves as the amplitude falls is what decides how the watch keeps time
through a day's wind. Use it on a run-down, or on any take that covers a
wide range of amplitude, to see how much the rate moves per degree and
at which amplitude it changes least. After an adjustment to the
hairspring, the balance or the escapement, compare it with a take from
before.

The picture is a whole take of a Dandong 3235, fully wound and left dial
up until it stopped 59.8 hours later, opened in the app. Its amplitude
fell from 255° to under 130°, so the pane sees the rate across more than
a hundred degrees.

| Mark | Part | What it does or shows |
| --- | --- | --- |
| 1 | **View** | **Curve** (this view) or **Walk** (below). Kept between runs. |
| 2 | **Export…** | Saves the pane as a picture, PNG or SVG. |
| 3 | The slope | "Between 145° and 251° the rate goes with the amplitude by −0.23 ± 0.01 s/d per degree": a straight line through every reading. |
| 4 | The ends | The median rate in the lowest and the highest 2° step. |
| 5 | Where it changes least | The amplitude where the curve turns, when that is inside the range. A vertical line marks it. |
| 6 | A dot and its bar | The median rate in one 2° step of amplitude, and a bar over the middle half of its readings. |
| 7 | The curve | A quadratic through the dots, each weighted by its readings. |

**In the picture,** from 251° down to about 211° the dots fall in a
smooth line, from −58 to about −105 s/d: this watch loses about 1.2 s/d
more for each degree of amplitude it loses. That is its isochronism over
the first 40 hours or so of the wind, and the figure an adjustment of
the hairspring or the escapement would aim to flatten. Below 211° the
dots break from the curve: at about 207° and again at about 175° the
rate jumps faster by 15 to 30 s/d within a few degrees, and below 173°
it stays near −36 s/d. The single slope, −0.23 s/d per degree, and the
quadratic both sum that up poorly. The ends, −36.4 s/d at 145° and
−57.6 s/d at 251°, and the turn at 213° are worked out as below; the
Walk view shows when the jumps came.

## The Walk View

![Isochronism, the Walk view, over the same take, marks 1 to 4](images/isochronism-walk.png)

**Walk** keeps the time the curve leaves out: one dot for each 5 minutes
(once a take has more than 250 such 5 minutes, about 20.8 hours
without gaps, neighbours are joined, 10, 15 … minutes a dot, so there
are at most 250), at its median rate and its median amplitude (on a long take,
the mean of the joined 5 minutes' medians), in time order, joined by a
trail. Each dot is coloured by the hours since the take's start, with a
key. A ring marks the start, a large dot the end with its hour, and a
ring each 12 hours (each 24 hours on a take over 96 hours). The trail
breaks where a stretch was left out or the take broke. The pointer on a
dot gives its time and figures.

| Mark | Part | What it shows |
| --- | --- | --- |
| 1 | The start | A ring at the first 5 minutes, labelled "Start". |
| 2 | The end | A large dot at the last, with its hour: "58 h". |
| 3 | The trail | Dot to dot in time order, shading from each dot's colour to the next's. |
| 4 | The key | The colours from 0 h to the take's end, "58 h since start". |

**In the picture,** the watch starts at 255° and −40 s/d, top right,
and for the first 44 hours walks down and to the left: less amplitude,
a slower rate, the smooth part of the curve, with the 12 h, 24 h and
36 h rings along it. Then the walk turns twice almost straight up: near
207° (at about 44 hours) and near 174° (at about 51 hours) the rate
gains 15 to 30 s/d while the amplitude hardly moves. Those jumps don't
follow the amplitude: something else changed the rate at those
moments, and the Curve view, which has no time in it, can't tell them
apart from it. From about 52 hours the rate holds near −36 s/d down to
140°, and the walk ends at 124° at 58 hours. Charts (chapter 8) and the
Paper Strip show what happened at those hours.

Use it to see the path the watch took: a steady walk down in amplitude,
a step after a knock, or a return to where it was. Both views span the
same amplitudes, so a degree stays put when you switch.

## How It Is Worked Out

- **Rate against amplitude, reading by reading.** Each 10 s rate reading
  is paired with the amplitude of its own 2 s windows.
- **Steps of 2°,** each needing at least 10 readings, between the 2nd and
  98th percentile of the amplitudes, so a few stray readings don't add
  steps of their own. At least 3 steps are needed.
- **The slope** is a straight line through every reading, none smoothed
  (a reading over 6σ from its neighbours' median is left out first),
  corrected for the noise in the amplitude readings, with a ± that allows
  for correlated readings. It is withheld when that noise is half or more
  of their variance. M12 gives the method.
- **The turn** is where the quadratic is flattest, given when it falls in
  the middle 80% of the range.
- **The walk** takes the median rate of the 10 s readings and the median
  amplitude of the 2 s windows in each 5 minutes; a 5 minutes with under
  half of either is left out, never filled. Past 250 of them, neighbouring
  5 minutes are joined, 10, 15 … minutes a dot, so
  there are at most 250.

## What to Look For

| What you see | What it can mean |
| --- | --- |
| A flat curve | The rate barely moves with the amplitude over this range. |
| The rate rising as the amplitude falls | The watch gains at low amplitude. |
| The rate falling as the amplitude falls | The watch loses at low amplitude. |
| A turn inside the range | The amplitude where the rate is least sensitive. |
| A walk that doubles back | The amplitude rose again: a wind, or a change in the stand or the watch. |

## What It Can't Tell You

- **The cause.** The slope says the rate and the amplitude went together.
  Anything else that changed over the take, the temperature or the
  position, moves with them.
- **Much on a short take.** Over a few degrees, the curve is mostly
  noise. It is made for a run-down of many hours.
- **A mix of positions.** A take in several positions mixes them, and the
  pane says so.
- **The wheel cycle.** A cycle of a few minutes that moves the rate and
  amplitude together is in the Steadiness views (chapters 11 and 12), not
  here.

Until there are 5 minutes of beats in one position, the pane says so.
The **?** in its toolbar gives what it needs.

---

Previous: [Wavelet Map](18-wavelet-map.md) ·
Next: [Positions Test](20-positions-test.md)
