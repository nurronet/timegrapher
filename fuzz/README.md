# Fuzzing the file readers

Five [`cargo fuzz`](https://rust-fuzz.github.io/book/cargo-fuzz.html)
targets feed made-up files to the readers that take files from outside, and made-up sound to the FLAC writer:

| Target | Reads |
| --- | --- |
| `analysis_take` | a take kept as its analysis alone (`analysis/`), then measures it |
| `session_toml` | a take's `session.toml`, its checks, times, masks and settings |
| `clock_log` | a clock log, then fits it |
| `report_zip` | a report bundle's ZIP file (`archive::read_zip`) |
| `flac_encode` | sound written as FLAC (`flac::encode`): it must decode to the same samples and be no bigger than them plain |

They need a nightly toolchain and `cargo install cargo-fuzz`. They are not
part of the workspace and do not run in CI: a run is long, and is meant to
go overnight. From this folder:

```sh
cargo +nightly run --release --bin seeds   # well-formed inputs to start from
cargo +nightly fuzz run analysis_take -- -max_total_time=3600 -rss_limit_mb=4096
```

A crash is saved under `artifacts/<target>/`. Run it again with
`cargo +nightly fuzz run <target> artifacts/<target>/<file>`; each one is
fixed with a test in the crate that owns the reader, which is the test that
keeps it fixed.
