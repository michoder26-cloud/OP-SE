# OP-SE architecture decision (2026-11-10)

## Decision

Use the MIT-licensed Code OSS source as the desktop IDE base. Keep the existing
Rust editor prototype in the separate OP-SE repository as an experiment while
the desktop workbench is brought up. Build OP-SE features as maintained workbench
changes and extensions, with Rust services only where native processing helps.

## Why

Code OSS already implements workspaces, files, tabs and split editors, search
and replace, integrated terminal, Git, debugger interfaces, language service
integration, command palette, settings, and an extension host. Electron plus
Monaco supplies an editor component, but the surrounding IDE systems would
need to be built and maintained separately. This fork's update cost is high;
changes should be small and rebased against upstream regularly.

The Code OSS license is MIT. Do not assume that Microsoft's branded builds,
Marketplace access, remote services, or all bundled extension artifacts can be
redistributed. Audit every build artifact and configure an eligible extension
registry before publishing installers. The current checkout has not passed that
distribution audit.

## First vertical slice

1. Brand the local Code OSS build as OP-SE.
2. Complete dependency installation and compile the Windows desktop build.
3. Launch a real workspace, edit and save a file, then run a command in the
   integrated terminal. Record the observed result in `test-report.md`.

The Rust prototype already supports opening and saving workspace files, but
its missing PTY terminal and language service integration are reasons to move
the product shell to Code OSS.

## Commercial boundary

The self-hosted source and local features remain free under their licenses.
Managed hosting and support are optional services with separate contracts.
The client must work without OP-SE account registration. BYO model credentials
and local models remain options. Any future billing, team administration, and
usage metering belong to the optional hosted service, not the offline IDE.
