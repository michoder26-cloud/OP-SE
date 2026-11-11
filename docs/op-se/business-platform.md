# OP-SE business platform design

## Product boundary

The desktop IDE and self-hosted deployment must work without an OP-SE account, license server, or payment. Users own their workspace and provider credentials. Optional managed hosting and support are separate purchased services. This is an original design informed by the publicly documented product categories in [Cursor pricing](https://prod.cursor.com/help/account-and-billing/pricing) and [team setup](https://prod.cursor.com/help/account-and-billing/teams-setup); it does not claim to reproduce Cursor's private backend.

## Proposed hosted service

| Component | Responsibility | Self-hosted behavior |
| --- | --- | --- |
| Organization and identity | Members, invitations, roles, SSO configuration | Local admin and optional external identity provider; no OP-SE identity required |
| Subscription and entitlements | Hosted plan, seat count, feature access, renewal state | No entitlement gate for the IDE or self-hosted features |
| Usage metering | Idempotent events for hosted compute, storage, and model proxy usage | Local-only counters if the operator enables them; no default telemetry |
| Billing adapter | Checkout, invoices, taxes, refunds, payment webhooks | Absent; self-hosting is free |
| Hosted workspace control plane | Provisioning, lifecycle, quotas, backups, and audit events | Operator owns deployment, backup, and retention |
| Support | Tickets, service tier, incident response | Community issue tracker and documentation |

Billing webhooks must be authenticated and idempotent. Entitlements should derive from a durable subscription state, not a mutable client flag. Usage events need a stable event ID and reconciliation path; the client must show the amount and destination before any provider-backed request. Hosted failures must not disable the offline editor. Keep payment data at the payment provider and avoid storing card data in OP-SE.

## Development sequence

1. Complete the local IDE slice and verify workspace, edit/save, terminal, search, and Git in the branded build.
2. Add a provider adapter and local AI chat with explicit context, streaming, cancellation, safe storage, and diff review. Keep it usable with local models and user-owned credentials.
3. Add workspace indexing, agent tool approvals, checkpoints, and rollback with tests proving pre-existing edits survive.
4. Define a self-hosted service deployment and API contracts. Add optional managed hosting only after the local product is reliable.
5. Implement organization identity, metering, billing adapter, and support operations behind hosted-only boundaries. Test webhook replay, seat changes, cancellation, and offline continuity before selling service.

No hosted service or billing code exists yet.
