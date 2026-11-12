# OP-SE architecture decision (updated 2026-11-12)

## Decision

Use the MIT-licensed Code OSS source as the desktop IDE base. Build OP-SE features as maintained workbench changes and extensions, with Rust services where native processing helps. The earlier Rust editor prototype is a separate repository; its current working tree contains deleted files, so it is not part of this build.

## Why

Code OSS already implements workspaces, files, tabs and split editors, search
and replace, integrated terminal, Git, debugger interfaces, language service
integration, command palette, settings, and an extension host. Electron plus
Monaco supplies an editor component, but the surrounding IDE systems would
need to be built and maintained separately. This fork's update cost is high;
changes should be small and rebased against upstream regularly.

The Code OSS license is MIT. Do not assume that Microsoft's branded builds, Marketplace access, remote services, or all bundled extension artifacts can be redistributed. Audit every build artifact and configure an eligible extension registry before publishing installers. The current checkout has not passed that distribution audit. `product.json` still contains Microsoft/Copilot service configuration and bundled extension references that need review before release.

## First vertical slice

1. Brand the local Code OSS build as OP-SE and compile the Windows desktop build (done).
2. Launch a real workspace and verify editor save and shell terminal through the branded build's integration tests (done; see `test-report.md`).
3. Run manual keyboard checks in the visible window before calling the UI experience complete; extension-host tests already cover file edit/save, terminal, project search, and Git against real temporary workspaces.

The earlier Rust prototype had workspace file editing, but lacked a PTY terminal and language service integration. The current product shell therefore uses Code OSS.

## Commercial boundary

The self-hosted source and local features remain free under their licenses.
Managed hosting and support are optional services with separate contracts.
The client must work without OP-SE account registration. BYO model credentials
and local models remain options. The optional self-hosted Rust control plane
stores organizations, members, and explicitly submitted usage events locally.
Payment processing and hosted service entitlements belong only to a future
managed deployment; they must never gate the offline IDE.
See `business-platform.md` for the original service design and `rea-evidence.md` for Cursor evidence limits.
