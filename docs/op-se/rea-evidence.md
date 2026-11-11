# Cursor evidence and limits (2026-11-11)

## Method

The installed `reverse-engineer-anything` skill (REA v60) directs Electron/JavaScript artifact analysis to `analyze_javascript_application`. The REA MCP server was not registered in this Codex session, so the documented `rea-agents@6.8.0` CLI fallback was used against the locally installed Cursor app at `C:\Users\TKTF\AppData\Local\Programs\cursor\resources\app`. The app was read as inert files; no bundled code was executed by REA.

The first run selected `out/vs/workbench/workbench.desktop.main.js` (46.9 MB). The worker exhausted its 1024 MiB V8 heap and skipped that file: zero JavaScript files parsed. Its Evidence ID is `ev_ce8fff67670f3a65fcb70a1e3ed11fb99f0945e597f8890f9cdc110e4b873368`. This run establishes inventory identity only, not workbench behavior.

The second run selected `extensions/cursor-deeplink/dist/main.js` (905,436 bytes), with a 2048 MiB worker heap. REA parsed one file, visited 221,610 AST nodes, and reported zero parse failures. Its Evidence ID is `ev_93691185f2264127cb452ca20a18ad50e8b7317ca730b30f8c2354f8606b7786`; the full Evidence bundle is retained locally at `D:\rea-cursor-deeplink-evidence.json` and is not committed. The selected file is only one of 18,276 inventoried files. Semantic node and unknown retention reached configured ceilings. The ten generic endpoint candidates (for example `name`, `id`, `cookie`) are static call-site candidates, not verified URLs or billing endpoints.

## Public product facts

- Cursor's [pricing documentation](https://prod.cursor.com/help/account-and-billing/pricing) describes free and paid individual tiers, team seats, usage allowances, and Enterprise invoice billing. Plan names and prices may change.
- Cursor's [team setup documentation](https://prod.cursor.com/help/account-and-billing/teams-setup) describes a team dashboard, centralized billing, invites, Admin/Member roles, SSO, and analytics.

## Unknowns

Neither a shipped client nor public product documentation establishes Cursor's private database schema, payment processor integration, entitlement algorithm, fraud controls, internal support systems, or service topology. OP-SE must design and implement these independently. Static strings and inferred call relationships are not runtime observations.
