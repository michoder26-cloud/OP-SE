# Public business-system observations and original OP-SE design

## Evidence boundary

Cursor's public [pricing page](https://cursor.com/pricing) describes free,
individual, team and enterprise tiers, usage allowances, centralized billing,
team analytics and enterprise spend controls. Its public
[Admin API docs](https://prod.cursor.com/docs/account/teams/admin-api)
describe member, usage and spending endpoints. These establish product-facing
capabilities only. They do not reveal Cursor's private billing service,
database schema, fraud controls, cost accounting or provider contracts. REA
analysis of the installed desktop app cannot by itself verify those server
internals.

## Original OP-SE service design

| Capability | Self-hosted community | Optional managed service |
| --- | --- | --- |
| Desktop IDE and local workspace | Free | Free client remains usable |
| AI provider | User-selected local model or own credentials | User-selected, with clear billing path |
| Accounts and teams | Local users and self-managed identity | Hosted organizations and invites |
| Usage records | Local, opt-in diagnostics | Per-organization metering and export |
| Billing | None | Separate subscription and invoice service |
| Support | Community issues | Paid support agreement |
| Data retention | User-controlled | Explicit organization policy |

The core app must not require a hosted account or phone-home license check.
Managed service endpoints should be optional adapters, with an explicit
boundary and data-transfer preview. Metering, invoices, and entitlement
enforcement must live only in the managed service, so self-hosting remains
fully functional without it.

This is a proposed OP-SE design, not a claim about Cursor's implementation.
