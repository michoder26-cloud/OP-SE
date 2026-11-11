# OP-SE Code OSS fork

This checkout is based on the MIT-licensed `microsoft/vscode` source at upstream commit `9192ec48`. The product name and application identifiers are set in `product.json`. The editor, workspace, terminal, Git, debug and extension workbench come from Code OSS; OP-SE AI workbench integration remains to be built.

This is a separate checkout from the earlier Rust prototype at `C:\Users\TKTF\Desktop\OP-code`. See `docs/op-se/architecture.md`, `feature-matrix.md`, `providers.md`, `business-platform.md`, `rea-evidence.md`, and `test-report.md` for the decision, current status and verification limits.

The Visual Studio Marketplace is not assumed available. No OP-SE installer or redistributable bundle is ready; bundled extensions and services require review before release.

## Run the development build on Windows

This checkout was compiled with Node 24.22.1, pnpm 12.16.0 and Visual Studio
Build Tools 2026. Open PowerShell in this directory, then run:

```powershell
$env:PATH = 'D:\OpenCode-tools\node-v24.22.1-win-x64;' + $env:PATH
$env:COREPACK_HOME = 'D:\OpenCode-tools\corepack'
$env:vs2026_install = 'D:\VisualStudio\BuildTools'
corepack pnpm install --frozen-lockfile
corepack pnpm run gulp compile
node build/lib/preLaunch.ts
./scripts/code.bat 'C:\path\to\your\workspace'
```

After the build is ready, the PowerShell wrapper validates paths and opens an existing folder:

```powershell
./scripts/op-se-dev.ps1 'C:\path\to\your\workspace' -SkipPrelaunch
```

The `scripts/code.bat` launcher may run prelaunch again. To use an already
prepared build, set `VSCODE_SKIP_PRELAUNCH=1` in the same shell before launching.
Use a workspace you trust. The current fork has not passed an installer or
extension redistribution audit.
