# OP-SE test report (updated 2026-11-12)

## Executed on Windows

- Inspected the upstream Code OSS checkout and existing OP-SE branding patch.
- Confirmed Node 24.22.1 and pnpm 12.16.0 run with `COREPACK_HOME` on drive D.
- Confirmed the Visual Studio 2026 toolchain passes the Code OSS preinstall
  check when `vs2026_install` points to `D:\VisualStudio\BuildTools`.
- `pnpm install --frozen-lockfile` completed successfully (exit code 0),
  including native module and bundled extension postinstall steps.
- `pnpm run gulp compile` completed successfully (exit code 0); the core
  TypeScript compilation reported zero errors. This verifies compilation,
  not desktop behavior.
- Started `node build/lib/preLaunch.ts` to prepare the Electron runtime.
- `scripts/code.bat <workspace>` launched the branded Electron development app. The OP-SE window displayed the real `C:\Users\TKTF\AppData\Local\Temp\op-se-ide-slice` folder and `hello.rs` in Explorer.
- `scripts/op-se-dev.ps1 <workspace> -CheckOnly` validated the launcher, compiled workbench, Electron executable, Node availability, and workspace path with exit code 0.
- `scripts/test-integration.bat --suite api-folder --grep save` completed with exit code 0 and **8 passing** tests, covering save, saveAll, save events, and encoding.
- `scripts/test-integration.bat --suite api-folder --grep "echo works in the default shell"` completed with exit code 0 and **1 passing** test. It created a real terminal in the branded extension host, sent a shell command, and observed terminal output.
- `scripts/test-integration.bat --suite api-folder --grep findTextInFiles` completed with exit code 0 and **2 passing** tests, including cancellation.
- `scripts/test-integration.bat --suite git --grep "git smoke test"` completed with exit code 0 and **5 passing**, **2 pending** tests against a temporary Git repository. Passing cases covered working-tree status, opening a diff editor, staging, committing, and commit-message encoding.
- REA CLI parsed one selected Cursor extension bundle with zero parse failures. The large workbench bundle was skipped after a 1024 MiB worker heap exhaustion; details in `rea-evidence.md`.
- Previously launched an extension development host. Its accessibility tree displayed the OP-SE AI Activity Bar icon, and the extension host log identified `op-se.op-se-ai` as loaded.
- `node --test extensions/op-se-ai/test/provider.test.js` passed **4** tests for secure endpoint selection, split SSE chunks, HTTP errors without key exposure, and cancellation of an active stream. These tests use a controlled local HTTP server and do not prove a real model response.
- `node extensions/op-se-ai/test/ollama-smoke.js` with `OP_SE_TEST_MODEL` set to an installed local model completed with exit code 0 and streamed one text chunk, `OK`, from the real Ollama OpenAI-compatible endpoint. The first 16-token probe returned no text because that model spent its tokens on reasoning; a 128-token probe passed.
- `node --check` accepted the modified AI extension, provider, and smoke script; the extension manifest parsed as JSON. These are syntax checks, not an inline-edit UI test.
- `cargo test --manifest-path services/op-se-control-plane/Cargo.toml` completed with exit code 0 and **2 passing** integration tests. They exercised free entitlements without login, token authentication, organization/member writes, slug validation, idempotent usage, conflicting replay, and SQLite persistence across reopen.
- `cargo run --manifest-path services/op-se-control-plane/Cargo.toml` started the real service on `127.0.0.1:43118`. HTTP requests returned health `ok`, `billing_required=false`, created `smoke-team`, recorded a 9-unit usage event, returned `duplicate` on replay, and returned a summary quantity of 9. The smoke service was stopped after verification.
- `cargo build --release --manifest-path services/op-se-control-plane/Cargo.toml` and `services/op-se-control-plane/package.ps1` completed with exit code 0. The regenerated local ZIP at `services/op-se-control-plane/dist/op-se-control-plane-win-x64.zip` was verified to contain the executable, README, `.env.example`, and `LICENSE.txt`. This packages the control plane only.
- The ZIP was extracted and its release executable started on `127.0.0.1:43119`. HTTP health returned `ok` and self-hosted entitlements returned `billing_required=false`; the process was then stopped.

## Not yet verified

- Manual keyboard edit and disk save in the visible OP-SE window. Windows computer-use capture timed out, so the API tests above are the evidence for save behavior.
- Search/replace and Git controls in the visible window, debugger, extension installation, language services, and terminal command exit code in the branded window.
- Opening the OP-SE AI chat webview and completing a request from that UI. The provider adapter worked with real Ollama, but the webview path remains unchecked.
- Inline diff accept/reject in the visible app, agent edits, rollback, timeout under a real network failure, and secure-storage behavior in the running extension.
- Installer build and redistribution compliance. No package is published.
- Managed hosting, payment processing, invoices, support workflow, and remote deployment of the control plane. The service is currently loopback-only with a single admin token.
- `cargo clippy` was attempted but the `clippy` component is not installed for this Rust toolchain, so no Clippy result is claimed.

The nine passing Rust prototype tests reported earlier are separate from this Code OSS fork and do not establish these results. No mock result is reported as a feature pass.
