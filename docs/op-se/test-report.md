# Test report (2026-11-10)

## Executed

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

## Not yet verified

- Code OSS desktop compilation, launch, workspace edit/save and terminal.
- Git, search, debugger, extension loading in the branded build.
- AI workflows in the Code OSS fork, installer, and redistribution compliance.

The nine passing Rust prototype tests reported earlier are separate from this
Code OSS fork and do not establish these results.
