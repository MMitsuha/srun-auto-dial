# CLAUDE.md

This file gives coding agents the project-specific contracts needed to work safely in this repository.

## Project overview

`srun-auto-dial` is a Rust TUI and REST API for Srun campus-network authentication, plus a Next.js management UI. It supports:

- login, logout, and status through a local Linux interface;
- temporary macvlan sessions using a caller-selected MAC address;
- controlled batches of locally administered random MAC addresses;
- manual credentials or server-side JSON credential files.

The Rust backend is Linux-only. Netlink, macvlan, raw DHCP packets, and route changes require root or equivalent `CAP_NET_ADMIN` and `CAP_NET_RAW` capabilities.

## Stable runtime topology

| Component | Default | Contract |
|---|---|---|
| Rust API | `127.0.0.1:3000` | Direct endpoints live under `/api/*` |
| Next.js Web | `0.0.0.0:3001` in the container; port `3001` in development | Browser-facing UI |
| Next proxy | same-origin `/api/backend/*` | Maps to backend `/api/*` |

The browser API client must continue to use `/api/backend/*`. `API_URL` and `API_KEY` are read only by `web/src/app/api/backend/[...path]/route.ts`; never move either value into `NEXT_PUBLIC_*`, client components, HTML, or browser bundles. The proxy injects `X-API-Key` server-side.

## Build, test, and run

Rust commands must run on Linux:

```bash
cargo fmt --all -- --check
cargo check --locked --all-targets
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
cargo build --release --locked

# TUI
sudo cargo run --locked -- tui
sudo cargo run --locked -- -vv tui

# REST API, default 127.0.0.1:3000
sudo cargo run --locked -- server
sudo cargo run --locked -- -c srun.toml server --host 0.0.0.0 --port 3000
```

Web commands:

```bash
cd web
bun install --frozen-lockfile
bun run lint
bun run typecheck
bun test
bun run build

# Starts Next on port 3001; API_URL/API_KEY remain server-only.
API_URL=http://127.0.0.1:3000 API_KEY=secret bun run dev
```

The repository has a noninteractive ESLint flat configuration. `bun run lint` uses `--max-warnings 0` and is part of CI alongside typecheck, tests, and the production build.

## Architecture

```text
src/
├── main.rs         # clap entry point, config validation, TUI/server dispatch
├── config.rs       # TOML loading, defaults, URL and server validation
├── error.rs        # typed SrunError, HTTP status/code/public-message mapping
├── service.rs      # shared business logic, validation, operation serialization
├── srun/
│   ├── mod.rs      # typed portal requests/responses and login/logout protocol
│   ├── base64.rs   # Srun-specific nonstandard base64 alphabet
│   ├── xencode.rs  # Srun XXTEA-derived encoder
│   └── utils.rs    # headers, JSONP, hashes, callback/timestamp helpers
├── net/
│   ├── dhcp.rs     # correlated DHCP Discover/Request/ACK/NAK flow
│   └── netlink.rs  # links, macvlan, addresses, and routes
├── api/
│   ├── auth.rs     # constant-time API-key middleware
│   ├── handlers.rs # validation and uniform JSON rejection responses
│   ├── models.rs   # request and response wire models
│   └── server.rs   # Axum routes on backend port 3000
└── tui/mod.rs      # interactive caller of the same service layer

web/
├── src/app/        # Dashboard, Connect, Disconnect, errors, and backend proxy
├── src/components/ # shared accessible UI components
├── src/lib/        # browser API client and form validation
├── tests/          # Bun parser and validation tests
└── Dockerfile      # standalone Next server on PORT=3001
```

## Backend API contract

Success responses use:

```json
{
  "success": true,
  "data": {}
}
```

Failures use a structured error. `field` is optional:

```json
{
  "success": false,
  "error": {
    "code": "validation_failed",
    "message": "count must be between 1 and 100.",
    "field": "count"
  }
}
```

Keep `code` stable and machine-readable. Public messages must be actionable but must not expose secrets, request URLs containing protocol material, or raw netlink internals. A credential-file error may identify the server path the caller selected when that is needed to correct the request. Log the detailed source chain on the backend. Axum extractor failures, authentication failures, unknown routes, and method mismatches must retain the same envelope.

Direct backend routes:

| Method | Path |
|---|---|
| GET | `/api/health` |
| GET | `/api/interfaces` |
| GET | `/api/status?interface=X` |
| POST | `/api/login/local` |
| POST | `/api/logout/local` |
| POST | `/api/status/macvlan` |
| POST | `/api/login/macvlan` |
| POST | `/api/logout/macvlan` |
| POST | `/api/login/random` |

Manual `username` and `password` must be supplied together. Manual credentials and `userinfo_path` are mutually exclusive. `userinfo_path` is always resolved on the backend host.

### Random batch wire shape

Do not regress to serializing Rust `Result` as `{ "Ok": ... }` or `{ "Err": ... }`. `/api/login/random` returns this summary inside the top-level success `data`:

```text
{
  requested,
  attempted,
  succeeded,
  failed,
  stopped_reason?,
  results: [
    {
      mac,
      success,
      data?,
      error?: { code, message, field? }
    }
  ]
}
```

The summary must satisfy `attempted == results.length`, `succeeded + failed == attempted`, and `attempted <= requested`. `stopped_reason` is omitted unless a batch stops early. The Web runtime validator and results table consume this exact shape.

## Credential-file contract

The file is a JSON array of objects with string `username` and `password` fields:

```json
[
  { "username": "user-a", "password": "secret-a" },
  { "username": "user-b", "password": "secret-b" }
]
```

Enforced limits and invariants:

- maximum size: 1 MiB / 1,048,576 bytes;
- 1 through 10,000 entries;
- nonblank usernames;
- nonempty passwords;
- unique usernames within the file;
- random batch count from 1 through 100;
- each account is attempted at most three times within one batch.

Keep credential-file parse errors separate from malformed portal JSON. Never derive/log a plaintext password-bearing structure with an unredacted debug representation.

## Network-operation invariants

- `SrunService` is shared by concurrent Axum requests, so state mutations require async synchronization.
- macvlan operations are serialized for their full lifetime. A competing operation returns `operation_in_progress` rather than deleting or reusing another operation's interface.
- Every macvlan session owns a unique 15-byte `srnxxxxxxxxxxxx` temporary interface. A fixed abstract Unix socket serializes mutations across the shared Linux network namespace (including host-network containers); only its holder may remove stale managed names before an operation. Do not restore a single fixed `srun` name or remove the cross-process namespace lock.
- The abstract socket is a cooperative lock for trusted processes in that network namespace; it has no filesystem mode protecting its name from a hostile local squatter. Deployment isolation remains responsible for excluding untrusted processes from the backend's network namespace.
- Cleanup is part of the wire-level outcome. Preserve `cleanup_failed_after_success` and `operation_failed_cleanup_incomplete`, and stop a random batch when either shows that residual network state may remain.
- Explicit cleanup runs after the portal operation; cancellation/drop schedules best-effort deferred cleanup for that owned interface.
- The DHCP lease IP is compared with the IP observed by the portal before login/status/logout proceeds.
- Local login/logout mutations are serialized separately from read-only status calls.
- DHCP replies must remain correlated by xid, client hardware address, and selected server. ACK and NAK are distinct outcomes. Do not wrap an uncancellable `spawn_blocking` worker in a shorter timeout and then delete its interface while it can still run.

## Srun protocol invariants

- `srun/base64.rs` uses the alphabet `LVoJPiCN2R8G90yg+hmFHuacZ1OWMnrsSTXkYpUq/3dlbfKwv6xztjI7DeBE45QA`. It is not RFC 4648 base64.
- `srun/xencode.rs` is the Srun XXTEA-derived transformation. Preserve compatibility and golden vectors when refactoring.
- Login flow is user-info → challenge → serialized/escaped login info → xencode + custom base64 → HMAC-MD5 password field + SHA1 checksum → portal login.
- Portal responses are JSONP. Parsing must validate the expected callback and then deserialize endpoint-specific response types.
- Portal URLs may be absolute HTTP or HTTPS URLs. HTTPS support comes from reqwest rustls; keep it enabled if HTTPS remains accepted by config validation.
- Never log challenge tokens, credential-derived hashes, checksums, signatures, API keys, or raw sensitive portal data.

## Configuration and containers

`Config::load(None)` reads `srun.toml` from the process working directory. Only a missing implicit file falls back to defaults; other read/parse/validation failures are errors. `portal_url` accepts absolute `http://` and `https://` URLs and must not contain embedded credentials.

The backend image intentionally sets `WORKDIR /etc/srun-auto-dial`, so mounting `/etc/srun-auto-dial/srun.toml` works without passing `-c`. It exposes backend port `3000` and normally runs with host networking plus `NET_ADMIN`/`NET_RAW` capabilities.

The Web standalone image sets and exposes `PORT=3001`. `API_URL`, `API_KEY`, and optional proxy timeout variables are runtime environment variables; they must not be baked into a public browser bundle.

## Change checklist

When changing a wire model, validation rule, port, environment variable, or route:

1. update Rust model/handler/service tests;
2. update Web runtime validators and Bun tests;
3. update `README.md`, this file, and container/CI configuration together;
4. run the complete Linux Rust checks and Web typecheck/tests/build;
5. verify no generated `.next`, `node_modules`, or `*.tsbuildinfo` artifacts enter Git.
