# Warframe API Helper (Rust)

A Rust CLI that extracts Warframe authentication credentials (`accountId` +
`nonce`) from the **running game process**, then downloads your inventory from
Digital Extremes' inventory API and saves it as JSON plus an encrypted
`lastData.dat` compatible with common Warframe helper tooling.

**Not affiliated with Digital Extremes.** Use only on accounts and devices you
are authorized to use. Comply with DE's terms and content policy.

Binary: `warframe-api-helper`  
Repo: [Obsidian-Jackal/warframe-api-helper](https://github.com/Obsidian-Jackal/warframe-api-helper)

## Overview

By default the tool:

1. Scans the **running Warframe process** for `?accountId=` … `&nonce=`
2. Downloads inventory from DE's inventory endpoints
3. Writes `inventory[_name].json` and encrypted `lastData[_name].dat`

The `nonce` is session-specific and expires when you close Warframe or log out.
HTTP **409** / body `Log-in expired` is reported as `LOGIN_EXPIRED:` so you know
to re-scan.

## Features

- Live process scan on Windows and Linux (Wine/Proton process names supported)
- Reuse a known `--nonce` (+ optional `--account-id`) without memory access
- Account ID fallbacks on the nonce path: CLI/env → `lastData*.dat` → `EE.log`
- Downloads from `mobile.warframe.com/api/inventory.php`, then
  `api.warframe.com/api/inventory.php`
- Stale-session detection (`LOGIN_EXPIRED` / HTTP 409)
- Named output files from player alias (`inventory_<name>.json`, `lastData_<name>.dat`)
- Encrypted `lastData.dat` (default key shared with the C++ helper / AlecaFrame, or `--password`)
- `--all-matches`, `--verbose`, `--no-download`, custom `--pid` / `--output`

## Requirements

- **Rust toolchain** (stable) — [rustup](https://rustup.rs/)
- **Warframe** installed; for process scan you must be **logged in** and preferably
  at your Base of Operations (a Tenno relay is even more reliable); run the tool as
  the same OS user that owns the game process
- Optional: prior `lastData*.dat` when using `--nonce` without `--account-id`
  (reuse the same `--password` if the file was not written with the default key)

## How It Works

1. **Process search** (unless `--nonce` / `-n` or `NONCE`): finds Warframe
   (or uses `--pid`) and scans memory for `?accountId=` … `&nonce=`
2. **Match selection**: first complete pair by default; `--all-matches` keeps
   searching and picks a best 24-hex accountId match
3. **Download** (unless `--no-download`): GET with query
   `?accountId=…&nonce=…`
4. **Save**: JSON inventory + encrypted `lastData` (includes `accountId` for later reuse)

Providing `--nonce` / `-n` or `NONCE` skips process search and builds auth from
that nonce plus `accountId` (CLI/env → `lastData*.dat` → `EE.log`).

## Building

```bash
cargo build --release
# binary: target/release/warframe-api-helper
```

Debug build:

```bash
cargo build
# binary: target/debug/warframe-api-helper
```

Cross-compile with a standard Rust target, for example:

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

## Encryption Key

`lastData.dat` is encrypted so other tools can share the same file format. With
no `--password`, the key matches
[Sainan/warframe-api-helper](https://github.com/Sainan/warframe-api-helper)
and AlecaFrame / browse.wf `lastData.dat`. Pass `--password` to use your own
secret in place of that default (AlecaFrame will not open those files).

### Option 1: Default key (no flags)

Same default key as the C++ helper. Prior `lastData*.dat` files written with
that default can be opened without passing a password.

### Option 2: Password

```bash
./target/release/warframe-api-helper --password=YOUR_PASSWORD
# short: -p=YOUR_PASSWORD
```

### Option 3: Password via environment

```bash
export ENCRYPTION_PASSWORD=YOUR_PASSWORD
./target/release/warframe-api-helper
```

Files written with `--password` (or `ENCRYPTION_PASSWORD`) need the same
password to decrypt again.

## Usage

1. Launch Warframe and log in
2. Run the tool from the repo (or copy the release binary elsewhere):

```bash
./target/release/warframe-api-helper
```

### Provide nonce and/or account ID

```bash
# Flags (override env)
./target/release/warframe-api-helper --nonce=1234567890 --account-id=0123456789abcdef01234567
./target/release/warframe-api-helper -n=1234567890 -a=0123456789abcdef01234567

# Environment
export NONCE=1234567890
export ACCOUNT_ID=0123456789abcdef01234567
./target/release/warframe-api-helper
```

**Account ID resolution** (nonce path), in order:

1. `--account-id` / `-a` or `ACCOUNT_ID`
2. Decrypt `lastData*.dat` with the current encryption key
3. Read from `EE.log` (common Steam/Proton/Wine paths)

**Nonce** must come from `--nonce` / `-n` or `NONCE` when skipping a fresh scan.

### Skip memory scan

```bash
./target/release/warframe-api-helper --nonce=1234567890
./target/release/warframe-api-helper -n=1234567890
```

`--nonce` / `-n` or `NONCE` skips process search. `accountId` comes from CLI/env,
then `lastData*.dat`, then `EE.log`.

### Other useful flags

| Flag | Meaning |
|------|---------|
| `--no-download` | Find credentials only; do not hit the inventory API |
| `--all-matches` | Keep searching after the first complete auth string |
| `-v` / `--verbose` | Print match details, auth string, and full request URL |
| `--pid N` | Scan this PID instead of auto-detect |
| `--output FILE` | Inventory JSON path (default `inventory.json`) |
| `-o` / `--original-format` | Keep `accountId` in the written JSON object |

```bash
# Scan only
./target/release/warframe-api-helper --no-download

# Verbose download (prints full URL with nonce)
./target/release/warframe-api-helper -v
```

### Output files

Written under the **data directory**: next to the executable, or the **crate
root** when the binary lives in `target/release` or `target/debug`.

| File | Contents |
|------|----------|
| `inventory_[NAME].json` | Inventory JSON (readable) |
| `lastData_[NAME].dat` | Encrypted inventory cache (includes `accountId`) |

If the account name cannot be determined, files are `inventory.json` and
`lastData.dat`.

## Troubleshooting

### Process not found

- Ensure Warframe is running and you are logged in
- On Linux (Wine/Proton), the process name may be truncated (e.g. `Warframe.x64.ex`)
- Try `--pid` with the PID from `pgrep -a -i warframe` / Task Manager

### No matches / failed to extract auth

- Wait at your Base of Operations after login so the auth string is in memory
  (a Tenno Relay is even more reliable)
- Confirm you are not using a stale `--nonce` (look for `LOGIN_EXPIRED:`)

### `LOGIN_EXPIRED` / HTTP 409

- Nonce expired — log in again and re-scan (or pass a fresh `--nonce`)

### Request failed / invalid JSON

- Check network / DE API availability
- Confirm the inventory hosts are reachable
- Retry after a short wait; avoid hammering the endpoint

### Build

- Install Rust via rustup; this crate uses rustls (no `libssl-dev` needed)
- If linking fails on Linux, install a normal C toolchain for crates like `ring`

## Security & privacy

- Process scanning reads session credentials (`accountId` + `nonce`) from memory
- Before a download, stdout prints `Using accountId:` / `Using nonce:` (`-v` also
  prints the full request URL). Use `--no-download` to skip the API call
- A provided `--nonce` / `NONCE` skips process scan; `lastData` / `EE.log` only supply `accountId`
- Treat `lastData*.dat` as sensitive (encrypted inventory + `accountId`)
- Default key matches the C++ helper / AlecaFrame; `--password` uses your own secret
- Traffic goes to Digital Extremes inventory hosts only
- Antivirus may flag process-memory readers

## License

This crate is licensed under the **BSD 3-Clause License** (`LICENSE`,
`Cargo.toml`).

Upstream C++ helper lineage ([Sainan/warframe-api-helper](https://github.com/Sainan/warframe-api-helper))
uses **MIT + Commons Clause**; see `NOTICE` for that text and attribution.

## Credits

- Upstream: [Sainan/warframe-api-helper](https://github.com/Sainan/warframe-api-helper)
