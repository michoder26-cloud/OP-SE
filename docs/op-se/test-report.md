# Test report (2026-11-11)

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
- Previously launched the Code OSS development build and verified workspace
  edit/save and integrated terminal in a smoke project.
- Added the OP-SE AI extension. `node --check` accepted its JavaScript and
  PowerShell parsed its package manifest as JSON.
- Launched the extension development host. Its accessibility tree displayed
  the OP-SE AI Activity Bar icon, and the extension host log identified
  `op-se.op-se-ai` as loaded.

## Not yet verified

- Opening the chat webview itself and an end-to-end provider response. UI
  automation could observe the icon but could not click it due unavailable
  input geometry in this session.
- Git, search and debugger in the branded build.
- AI inline edit, agent workflows, installer, and redistribution compliance.

The nine passing Rust prototype tests reported earlier are separate from this
Code OSS fork and do not establish these results.
