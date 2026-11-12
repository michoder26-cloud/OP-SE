# OP-SE self-hosted control plane

This Rust service is the first working slice of OP-SE's optional organization and usage backend. The desktop IDE works without this service. Self-hosted entitlements do not require an OP-SE account or payment. Managed hosting, checkout, invoices, and support subscriptions are not implemented.

## Run on Windows

From this directory in PowerShell:

```powershell
$env:OP_SE_CONTROL_ADMIN_TOKEN = [Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
cargo run
```

The service listens on `127.0.0.1:43117` and stores data in `./data/op-se-control.db`. Set `OP_SE_CONTROL_DB` to another local path if needed. Keep the token private and out of the repository. The service refuses non-loopback bindings; use a TLS reverse proxy and a separate authentication design before exposing it remotely. Copy `.env.example` only as a reference; this service reads environment variables and does not parse `.env` files.

Build the Windows release ZIP from this directory with `./package.ps1`. It contains the executable, this README, `.env.example`, and the MIT license at `dist/op-se-control-plane-win-x64.zip`. The ZIP is ignored by Git; it is a control-plane package, not an IDE installer.

## API

`GET /health` and `GET /v1/self-host/entitlements` are public. All other routes require `Authorization: Bearer <OP_SE_CONTROL_ADMIN_TOKEN>`.

| Route | Purpose |
| --- | --- |
| `POST /v1/organizations` | Create a local organization with `slug` and `name` |
| `GET /v1/organizations/{id}` | Read an organization |
| `POST /v1/organizations/{id}/members` | Add or update a member's `email` and `role` (`admin` or `member`) |
| `GET /v1/organizations/{id}/members` | List members |
| `POST /v1/organizations/{id}/usage-events` | Record `event_id` (UUID), `meter`, and positive `quantity` |
| `GET /v1/organizations/{id}/usage-summary` | Read totals by meter |

Meters are `model_input_tokens`, `model_output_tokens`, `hosted_compute_seconds`, and `storage_byte_hours`. Repeating the same `event_id` with the same meter and quantity returns `duplicate`; changing its payload returns HTTP 409. Usage is recorded only when an authenticated caller explicitly sends an event. No telemetry is sent by the IDE.

## Tests

```powershell
cargo test
```

The tests use a temporary SQLite database and exercise the real router, authorization, organization and member writes, idempotent usage, input validation, and persistence across reopen. This is a local administrative API, not a production multi-tenant billing service. It has one administrator token and synchronous SQLite access; per-user identity, rate limits, audit logs, TLS, payment webhooks, invoices, and support operations are future slices.
