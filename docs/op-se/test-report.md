# OP-SE test report (2026-11-11)

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

## Not yet verified

- Manual keyboard edit and disk save in the visible OP-SE window. Windows computer-use capture timed out, so the API tests above are the evidence for save behavior.
- Search/replace and Git controls in the visible window, debugger, extension installation, language services, and terminal command exit code in the branded window.
- OP-SE AI workflows in the Code OSS fork, including streaming cancellation, inline diff, agent edits, rollback, provider/network failures, and secret handling.
- Installer build and redistribution compliance. No package is published.

The nine passing Rust prototype tests reported earlier are separate from this Code OSS fork and do not establish these results. No mock result is reported as a feature pass.
