# OP-SE feature matrix (updated 2026-11-12)

Status means **ยังไม่เริ่ม** (not started), **กำลังทำ** (in progress), **ใช้งานได้** (usable in the stated tested scope), or **ติดข้อจำกัด** (blocked). Code OSS source presence alone does not count as an OP-SE test pass. Cursor public evidence and REA limits are in `rea-evidence.md`.

| Capability | Status | Evidence / remaining work |
| --- | --- | --- |
| Open a folder and workspace | ใช้งานได้ | Branded Windows window opened the `op-se-ide-slice` folder and showed `hello.rs` in Explorer |
| Edit and save files | ใช้งานได้ | Eight save integration tests passed; Git smoke test also applied an edit, saved it, and observed the modified file. Manual keyboard check remains |
| Explorer create/rename/move/delete, tabs, split panes | กำลังทำ | Present in Code OSS source; OP-SE runtime checks remain |
| Project text search | ใช้งานได้ | Two `findTextInFiles` integration tests passed, including cancellation; search view and replace remain unchecked |
| Project replace, command palette, shortcuts, settings | กำลังทำ | Present in Code OSS source; branded runtime checks remain |
| Integrated shell terminal | ใช้งานได้ | Branded extension-host test `echo works in the default shell` passed on Windows; interactive window check remains |
| Git status, diff, staging, commit | ใช้งานได้ | Git smoke suite passed five tests against a temporary repository; manual source-control UI check remains |
| Diagnostics, autocomplete, go-to-definition | กำลังทำ | Language services are in the base; check a real supported project and installed language extensions |
| Debugger and extensions | กำลังทำ | Base workbench exists; redistribution audit and runtime checks remain |
| OP-SE AI chat and selected-code context | กำลังทำ | Existing OP-SE AI extension loads in the development host; provider response not yet verified end to end |
| Streaming, cancellation, timeout, provider errors | กำลังทำ | Four adapter tests passed and a real local Ollama response streamed `OK`; webview end-to-end behavior remains unchecked |
| Retry policy | ยังไม่เริ่ม | Must retry only bounded transient failures |
| Inline diff accept/reject | กำลังทำ | Edit Selection command generates a full-file diff and checks for intervening edits before applying; UI acceptance test remains |
| Multi-file agent and tool approvals | ยังไม่เริ่ม | No Code OSS integration yet |
| Checkpoint and rollback preserving user edits | ยังไม่เริ่ม | Requires overlap and conflict tests |
| Lexical index, ignore/secret filters, provenance, token budget | ยังไม่เริ่ม | Rust prototype is separate; integration absent |
| Semantic retrieval | ยังไม่เริ่ม | Add after lexical retrieval is measured |
| OS secure credential storage | กำลังทำ | Code OSS extension uses SecretStorage for API keys; Rust prototype remains separate |
| Self-hosted control plane | กำลังทำ | Rust HTTP API stores organizations, members, entitlements, and idempotent usage events in SQLite; two integration tests and loopback HTTP smoke passed. Desktop management UI is not connected yet |
| Windows control-plane package | ใช้งานได้ | Release build and local ZIP with executable, README, example settings, and MIT license verified |
| Optional managed hosting, billing, and support | ยังไม่เริ่ม | Design only; no backend code or sales flow |
| Windows installer | ติดข้อจำกัด | Development build runs; redistribution and bundled service audit required before publishing |

The current development build is a Code OSS fork, not a finished Cursor replacement. The earlier Rust prototype is not counted as integrated functionality; its old working tree currently contains deleted files and has not been restored.
