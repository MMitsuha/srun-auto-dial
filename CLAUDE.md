# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rust CLI tool + REST API server for automated login/logout on Srun (深澜) campus network authentication systems. Supports three MAC address modes: local adapter, user-specified custom, and randomly generated (via macvlan). Default portal target is `portal.hdu.edu.cn` but configurable via `srun.toml`.

**Linux-only** — requires root or `CAP_NET_ADMIN`/`CAP_NET_RAW` for macvlan creation and raw DHCP packets.

## Build & Run

```bash
cargo build
cargo build --release

# Interactive TUI mode
cargo run -- tui
cargo run -- -v tui          # with info logging
cargo run -- -vvv tui        # with trace logging

# REST API server mode
cargo run -- server
cargo run -- server --port 8080 --host 0.0.0.0

# With custom config
cargo run -- -c srun.toml tui
```

Rust edition 2024. No tests currently.

## Architecture

```
src/
├── main.rs        # clap entry point, dispatches to tui or api
├── error.rs       # SrunError enum (thiserror), Result alias
├── config.rs      # TOML config loading (Config, ServerConfig)
├── service.rs     # Core business logic (SrunService)
├── srun/
│   ├── mod.rs     # SrunClient — portal protocol (get_userinfo, get_challenge, login, logout)
│   ├── base64.rs  # Custom base64 with NON-STANDARD alphabet — do not replace
│   ├── xencode.rs # XXTEA-based encoding (port of Srun JS _encryptBase64)
│   └── utils.rs   # HMAC-MD5, SHA1, JSONP parsing, HTTP headers, timestamps
├── net/
│   ├── mod.rs     # Re-exports
│   ├── netlink.rs # Linux netlink: macvlan CRUD, link up, IP/route assignment
│   └── dhcp.rs    # Raw DHCP client (pnet + dhcproto) with retry
├── tui/
│   └── mod.rs     # inquire-based interactive TUI
└── api/
    ├── mod.rs     # Re-exports
    ├── server.rs  # axum server setup with tracing + API key middleware
    ├── handlers.rs# Route handlers for all endpoints
    ├── models.rs  # Request/response JSON types
    └── auth.rs    # API key middleware (X-API-Key or Bearer token)
```

### Key Design Decisions

- **Service layer** (`service.rs`): `SrunService` encapsulates all business logic. Both TUI and API call the same service methods — no duplication.
- **Error handling**: All errors use `SrunError` enum via `thiserror`. No `Result<T, String>`, no `anyhow`.
- **Configuration**: `srun.toml` with `Config` struct. Portal URL, AC ID, server host/port, API key all configurable. Defaults provided.
- **Logging**: `tracing` throughout. Verbosity via `-v`/`-vv`/`-vvv` flags.

### REST API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/health` | Health check |
| GET | `/api/status?interface=X` | Query online status |
| GET | `/api/interfaces` | List network interfaces |
| POST | `/api/login/local` | Login via local interface |
| POST | `/api/logout/local` | Logout via local interface |
| POST | `/api/login/macvlan` | Login via macvlan (custom MAC) |
| POST | `/api/logout/macvlan` | Logout via macvlan |
| POST | `/api/login/random` | Batch login with random MACs |

API key authentication is optional — configured via `api_key` in `srun.toml`.

### Srun Protocol Notes

- `srun/base64.rs` uses alphabet `LVoJPiCN2R8G90yg+hmFHuacZ1OWMnrsSTXkYpUq/3dlbfKwv6xztjI7DeBE45QA` — this is Srun-specific, **not** RFC 4648.
- `srun/xencode.rs` is XXTEA-derived — obfuscated constants are from the original JS.
- Login flow: `get_userinfo` → `get_challenge` → encode info with xencode+base64 → compute HMAC-MD5 password + SHA1 checksum → `srun_portal` call.
- All portal responses are JSONP-wrapped.

## Configuration

See `srun.toml.example`. User credentials in `userinfo.json`:
```json
[{"username": "user", "password": "pass"}]
```
