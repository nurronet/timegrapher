# Sending a session to the LN ERP

`timegrapher-erp` sends the figures of a `timegrapher session` to the LN ERP: it looks up the watch, signs the operator in with a PIN, signs each position with the station's own key, and sends them as one measurement session. It decides no reading. Every figure it sends is in the session's `summary.json`, taken by core's rules.

```sh
timegrapher session recordings/watch-1/            # writes session_report/summary.json
timegrapher-erp submit recordings/watch-1/ --identity SERIAL --operator OP1
```

## What the ERP administrator provides

One set per station: the ERP's address, an API key and secret, a **Station ID**, and an LN Workstation record in the ERP with that Station ID, Status Active and the key's user as its API User. Each operator needs an Operator ID and a PIN (6 to 8 digits) set in the ERP. The station integration guide in the ERP's repository (`docs/integration/timegrapher_station_integration.md`) describes all of this.

## Setting up

```sh
timegrapher-erp setup --url https://erp.example.com --key API_KEY --station STATION-01
timegrapher-erp test
```

`setup` asks for the API secret (or reads `TIMEGRAPHER_ERP_SECRET`) and saves `erp.toml` in the configuration folder: `%APPDATA%\timegrapher-erp` on Windows, `~/Library/Application Support/timegrapher-erp` on macOS, `~/.config/timegrapher-erp` elsewhere, or the folder named by `TIMEGRAPHER_ERP_DIR`. The secret is stored there as plain text, so keep that folder private. To keep it out of the file, leave it unsaved and set `TIMEGRAPHER_ERP_SECRET` when you run. `TIMEGRAPHER_ERP_URL`, `_KEY` and `_STATION` also override the file.

The address must be `https://`; `http://` is accepted only for this computer (a test ERP), because the key and secret go with every call. The ERP's certificate is checked against the system's certificate store. `--insecure-tls` turns the check off for a test server with a self-signed certificate. Redirects are never followed.

`test` calls the ERP's `ping` and shows the ERP user, its time and its API version.

## Commands

| Command | What it does |
| --- | --- |
| `setup` | Saves the address, key and Station ID |
| `test` | Checks the address, key and secret |
| `search TEXT` | Lists identities that match what is typed or scanned |
| `lookup IDENTITY` | Shows what the ERP knows: part, status, work order, recommended measurement type |
| `register` | Makes the station's signing key if there is none, and registers it |
| `submit SESSION --identity ID` | Sends a session's positions |

`submit` takes a `summary.json`, or the session folder holding `session_report/summary.json`.

## What `submit` does

1. Reads the summary (`timegrapher.session/2`) and lists any reading it leaves out: a position with no regular beat, or one the ERP has no name for.
2. Looks the identity up. If the ERP does not know it, nothing is sent and the nearest matches are shown.
3. Loads the signing key, or makes one. A new key is registered with the ERP (the ERP names it `<Station ID>-workstation` and makes it Active at once for a registered station).
4. With `--operator OP1`, asks for the PIN (typed with no echo, or the first line of piped input) and signs the operator in. A PIN that is not 6 to 8 digits is refused before it is sent, so it never costs one of the operator's five tries.
5. Sends each position as its own signed result, chained under one ERP session, then completes the session. The sign-in token lives in memory for this run only; it is never written to a file, and the PIN is wiped as soon as the call returns.
6. Signs the operator out, and prints what happened to each position.

Add `--dry-run` to see the exact bytes that would be signed, with nothing sent and no key made.

Options: `--work-order`, `--qa-standard` (default `Workshop`; the ERP judges the figures against it), `--temperature` (default `Room / 23C`) and `--measurement-type` (default: the type the ERP recommends for the identity, `Follow-up Certification` where it holds a baseline).

## What a result is

Each position's figures come from the summary and are rounded to six decimals, so the number signed is the number the ERP parses.

| Sent | From |
| --- | --- |
| Position name (`Dial Up`, `Crown Left`, ...) | The reading's position, named by core's `Position` |
| `rate_s_per_day`, `amplitude_deg` | The reading's measurement |
| `beat_error_ms` | Core's `beat_error_size` over the unlock and drop figures: from the unlock where found, as a size |
| `duration_seconds` | Core's `measured_seconds`: the stretch measured, less what was masked |
| `beat_frequency_bph` | The reading's beat rate |
| `lift_angle_deg` | The session's lift angle, when it has one |

A figure the position did not give is left out, not sent as zero. No baseline is sent; the ERP looks up its stored one.

## The operator and the verification level

Without `--operator`, the ERP records every result as **Invalid**: the station's signature is one of two it requires, and the operator's is the other. With a signed-in operator it records them as **Attested**: the ERP vouches for the operator, and the station's signature stays valid. `submit` says which it expects before it sends, and counts the attested results after.

Another result is Invalid if the key is not registered and Active, or the operator's sign-in has ended (8 hours after it began, or 30 minutes without a result). `submit` runs within one sign-in, so this only arises if a run is held up for a long time; the report then says the sign-in was not accepted.

## The signing key

The key is Ed25519, kept as `workstation_signing_key.bin` beside `erp.toml` (the 32-byte seed then the 32-byte public key, the layout the Ln-tg station uses, so its key file can be copied here). It is never written over a file that is there. A marker file beside it records that the key was registered.

If the ERP already holds a key under this Station ID and this station's key is new (for example the old file was lost), `submit` stops and says so: results signed with the new key would not verify. An administrator must revoke the old key in the ERP.

## In the desktop app

The app uses the same settings, key and calls. **Settings > ERP** holds the address, the API key and secret, the Station ID, the station's signing key and the operator's sign-in. The **ERP** view (after Bench and Watches, `Ctrl+3`) searches the ERP, shows the identity and sends the Positions Test with a note.

When the ERP is set up **and answering**, the Bench gets an ERP card across its top: a quick search, the identity picked, the Positions Test table, and Send. The app asks the ERP whether it answers when it starts, whenever the connection settings change, and every two minutes after. If the ERP is not defined, or it does not answer, the card is not shown and the ERP view says so. While nobody is signed in, the card shows a quick sign-in: the Operator ID and PIN, and Sign In (Enter in the PIN field signs in too). The PIN is sent once and wiped.

## Enhanced data

A result can carry much more than its figures. Turn on **Send Enhanced Data** in Settings > ERP (off by default) and Send adds, for each position:

- **The whole measurement** as core made it: rate, amplitude, the beat error from the unlock and from the drop, jitter, percentiles, the two states, coverage, confidence, every caveat and the clock it was corrected with; the stretches it was read over, the settings (beat rate, lift angle, calibre) and the Positions Test's values (mean rate, largest differences, DVH). Nothing is computed for the ERP: this is core's `Measurement` and `StateIndices` written out.
- **A recording**: a lossless FLAC cut of the take's own sound for the position, from the start of its first stretch to the end of its last (up to 24 MB).

The result is sent as schema `ln_tg_timing_result_v2`. The station signs the SHA-256 of the block and of the recording, not the data itself, so the signature stays small and the ERP can still tell if anything was swapped; the block travels with the result and the recording follows in a call of its own. The ERP keeps both as private files on the result and records the three figures as before.

Whether the ERP takes it is asked first (`ping` lists `enhanced_results_v1` and `recording_upload_v1`). An ERP that does not is sent the basic results, which it can verify, and the window says so. If a recording cannot be cut (no sound file, or it is over the size the ERP takes) the position's block still goes, and the line under Send says what was left out. The format and the calls are in the ERP repository's `docs/integration/timegrapher_station_integration.md` (section 5A).

The command line does not send enhanced data yet.

## What it cannot do yet

- It sends the summary figures only (the command line), not the per-beat samples.
- It does not queue: a position that fails to send is listed, and the others still go.
- The operator's PIN sign-in stands in for an operator signature; nothing from this tool is `Verified` (both device signatures).
- It reads `summary.json`, so a session has to have been measured first. Signing in from inside the desktop app is the app's own (above), not part of this tool.

## Tests

`cargo test -p timegrapher-erp` runs the tests, including the published test vector (the canonical bytes and signature were made with the ERP's own canonical JSON and Python's `cryptography` library, so the Rust side is checked against the other language) and the program itself against a stand-in ERP on this computer. The stand-in checks the API key, the PIN, the sign-in token and the signature over the bytes it rebuilds from what it receives, as the ERP does.
