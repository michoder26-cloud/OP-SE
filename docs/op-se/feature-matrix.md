# Feature matrix

Status refers to OP-SE, not to Cursor. "Available in base" still requires an
OP-SE build and hands-on verification.

| Capability | Status | Evidence / next check |
| --- | --- | --- |
| Workspace, file explorer, edit/save, tabs, split panes | In progress | Present in Code OSS source; branded build not yet launched |
| Search/replace, palette, shortcuts, settings | In progress | Present in Code OSS source; run workspace smoke test |
| Terminal, Git, diagnostics, debugger, extensions | In progress | Present in Code OSS source; packaging and runtime checks pending |
| Chat with selected-code context and streaming | In progress | OP-SE AI sidebar extension loads in development host; provider response not yet verified end to end |
| Inline diff, multi-file review | Not started | Rust prototype has separate implementation; Code OSS integration pending |
| AI completion, tool calls, command execution | Not started | Define extension/service contract and approval UX |
| Checkpoint and safe rollback | Not started | Must preserve pre-existing user edits |
| Local lexical code index | In progress | Rust prototype has implementation; integration pending |
| Semantic retrieval | Not started | Add only after measured need |
| Secure OS credential storage | In progress | Code OSS extension uses VS Code SecretStorage for API keys; Rust prototype remains separate |
| Optional managed hosting, billing, support | Not started | Separate service and entitlement design |
| Windows installer | Blocked | Desktop build and redistribution audit pending |

## Cursor public product evidence

Cursor's public pricing page advertises individual plans, Teams and Enterprise,
pooled usage and team controls. Current plan names and prices can change:
https://cursor.com/pricing . Its Admin API documentation exposes team members,
usage and spending data: https://prod.cursor.com/docs/account/teams/admin-api .
These are public product claims, not evidence of Cursor's internal billing
implementation. OP-SE's hosting, billing and support design will be original.
