# OP-SE Code OSS fork

This checkout is based on the MIT-licensed `microsoft/vscode` source at upstream commit `9192ec48`. The product name and application identifiers are set in `product.json`. The editor, workspace, terminal, Git, debug and extension workbench come from Code OSS. An OP-SE AI sidebar extension now provides an initial chat interface.

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

The launcher copies the OP-SE AI extension into the development profile so its icon appears in the normal app. To test the extension directly from source, add `-AiDevelopmentHost`. The separate extension development window may open without the folder; choose **File > Open Folder** there and select your workspace.

The `scripts/code.bat` launcher may run prelaunch again. To use an already
prepared build, set `VSCODE_SKIP_PRELAUNCH=1` in the same shell before launching.
Click the OP-SE AI icon in the Activity Bar. Configure `OP-SE AI: Base URL` and
`OP-SE AI: Model` in Settings, then use `OP-SE AI: Set API Key` from the Command
Palette when your provider requires a key. The default URL targets local Ollama's
OpenAI-compatible endpoint. A ChatGPT or Claude subscription is not an API key
and is not connected to this Code OSS fork yet.
Select code in a workspace file and run `OP-SE AI: Edit Selection` from the Command Palette to generate a diff. Inspect it before choosing Apply or Discard. The command refuses to apply a result if the file changed while the model was working or during review. This path has not yet been tested through the visible UI.
Use a workspace you trust. The current fork has not passed an installer or
extension redistribution audit.
