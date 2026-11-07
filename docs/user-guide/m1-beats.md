# M1. Beats

Every reading starts from a list of beats: a time, a number and a
quality for each tick and tock heard. This page says how the app finds
them in the sound.

## What It Is

A **beat** is one tick or tock found in the sound. Each has:

- **a time:** when its drop was heard, on the sound card's clock;
- **a number:** counted from the take's start, with any missed beats
  counted too, so beat 1,000 is always 1,000 beats after beat 0;
- **a quality:** how well its sound matches the take's typical beat.
  A typical beat scores about 1. Only beats scoring over 0.4, the **good
  beats**, are timed or measured.

Even-numbered beats are **Tick**, odd-numbered beats are **Tock**.

## How It Is Worked Out

1. **The envelope.** The sound is filtered to keep what is above
   1,500 Hz, where the escapement's clicks are and the room's hum isn't.
   Every sample is made positive, and smoothed over 0.2 ms. The result is
   the sound's loudness from moment to moment.
2. **Knocks gated out.** Any short burst with more than twice the energy
   of a typical tick is set to silence (a loud stretch over 0.1 s long is
   kept). A knock on the desk or a bump of the stand would otherwise
   pull the beats towards it.
3. **The beat rate.** With the beat rate on Auto, the app compares the
   envelope with itself shifted by each standard beat period, from
   12,000 to 72,000 bph, and picks the one the sound repeats at best.
4. **A first pass** follows the loudest point of each beat and builds the
   **template**: the median shape of a beat, from 20 ms before the drop to
   8 ms after it, the same span the Tick Tock Profile shows.
5. **A second pass** slides the template along the whole envelope and
   takes each place it fits best as a beat. The template is rebuilt from
   those beats, and the pass run once more.
6. **Numbering.** Each beat is numbered on from the one before by the
   whole number of beat periods between them, so a missed beat leaves a
   gap in the numbers, not a slip.

While listening, the last 4 s of sound go through the same steps every
half second. A beat is added once it is a quarter of a second from the
end of the sound, so each beat is measured once, with the same template.

## Why This Way

- **Matching the whole shape** finds a beat by its unlock, impulse and
  drop together. Picking the loudest point alone would jump to the unlock
  on the odd beat where it is louder than the drop.
- **One template for the whole take** keeps every beat's time on the same
  point of its sound. A template that drifted would read as a rate change.
- **Numbering across missed beats** means a lost beat costs one point,
  not the count. The rate fit (M2) needs the true count.

## When You'd Use It

You don't set any of this. It matters when you read:

- the **Paper Strip**, where each dot is one beat;
- the **signal** figure in the status bar, which compares the ticks'
  peaks with the level between them;
- the **540 of 600 Beats** chip, which counts the beats found against
  those expected.

## What It Helps Find

- **A weak or muffled sound:** many beats under 0.4 quality, or beats
  missing.
- **Knocks and bumps:** the gate removes most; a long knock still shows
  on the strip as a jump.
- **A wrong beat rate:** with the rate set by hand to the wrong standard,
  beats don't line up and **No regular beat found** appears.

## What It Can't Tell You

- **Which pallet stone is which.** Tick and Tock are names given by
  count. The app names them so the first steady stretch reads a positive
  beat error (M5).
- **Whether a sound is the watch.** A regular noise at a beat rate, such
  as another clock, can be tracked as beats. Over 10 s or more, beats
  scattering 2% of a beat or more are taken as no watch.
- **The time of the unlock.** A beat's time is near its drop. The unlock
  is found later, on averaged beats (M4).

---

[Contents](README.md) · Next: [M2. Rate](m2-rate.md)
