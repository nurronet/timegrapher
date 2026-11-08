# Historical DD3285 replay

These scripts reproduce a single-specimen experiment. They are not an engine
estimator or an automated accuracy gate. `marker.py` and `edges.py` preserve the
numerical kernels used before the matching audio was disclosed. `geometry.json`
and `selection-fits.json` are frozen pre-audio priors; replay does not refit those
priors. The exploratory global optimizer and rest-position tracker are not
packaged as general-purpose tools.

Install `requirements.txt` in a Python environment and provide `ffmpeg` on PATH.
From this directory:

```sh
python reproduce.py
```

Without raw media, this verifies all 19 frozen optical means against retained
endpoint measurements, the blind-result hashes, native crop hashes and pair-mean
arithmetic. It does not validate optical accuracy or uncertainty coverage.

For a contributor/reviewer with the originals, replay one take at a time:

```sh
python reproduce.py --take final \
  --video /path/to/final-video.mov \
  --audio /path/to/final-audio.wav \
  --cli /path/to/timegrapher-at-8002cdc \
  --output /tmp/dd3285-final-replay
```

The take names are `high`, `low`, `intermediate`, `final`. Sources are authenticated
against the manifest before decoding. Video-only and audio-only replay are also
supported by omitting the other source. Outputs must be outside this evidence
folder; a mismatch raises an error rather than changing the committed results.

The optical path decodes each two-second excerpt with its original crop and checks
the individual endpoint-derived amplitudes, not only the rounded published mean.
The audio path trims to the retained sync offset plus a zero/one-second shift to
align the CLI's two-second windows, then explicitly supplies 28,800 bph and each
of 55°, 58°, 59°. It checks the CSV's one-decimal Tick/Tock amplitudes. A newer CLI
may not reproduce the historical results and is not interchangeable with `8002cdc`.

`matched-windows.csv` includes both beat groups, their arithmetic mean, the frozen
optical value and allowance, source-relative times, and unlock/drop times relative
to the analyzer reference. The descriptive mean is not the app's pooled statistic.
The high take's 58° replay was added during documentation preparation, not during
its original blind comparison. Its original comparison JSON and figure retain
the original 52°/55°/59° candidates.

To check the retained tap offsets, run `inspect_sync.py --take final --video
/path/to/final-video.mov --audio /path/to/final-audio.wav` (or another take).
It searches near the retained offsets for the selected tap neighborhoods. This
checks alignment reproducibility, not independent image/audio latency or a blind
global synchronization algorithm. In particular, earlier microphone transients
in the intermediate take are unrelated to the video taps.

Native PNG examples contain no source MOV metadata. Their excerpt frame indices,
crop geometry and hashes are in `manifest.json`. The rest study is an auxiliary
observation, not part of the 19-window numerical replay. Its results and figure
are retained with their original limitations.

Raw source media are not publicly archived yet: null archive URLs mean unavailable,
not missing hash verification. Original session files are hashed but not copied;
`session-metadata.json` is a deliberately limited nonidentifying subset.
