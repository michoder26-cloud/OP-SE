param(
	[Parameter(Position = 0)]
	[string]$Workspace = (Get-Location).Path,
	[switch]$SkipPrelaunch,
	[switch]$CheckOnly
)

$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$launcher = Join-Path $PSScriptRoot 'code.bat'
$electron = Join-Path $repository '.build\electron\OP-SE.exe'
$compiledWorkbench = Join-Path $repository 'out\vs\code\electron-browser\workbench\workbench-dev.html'

if (-not (Test-Path -LiteralPath $Workspace -PathType Container)) {
	throw "Workspace folder does not exist: $Workspace"
}
$workspacePath = (Resolve-Path -LiteralPath $Workspace).Path

if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
	$localNode = 'D:\OpenCode-tools\node-v24.22.1-win-x64'
	if (Test-Path -LiteralPath (Join-Path $localNode 'node.exe') -PathType Leaf) {
		$env:PATH = "$localNode;$env:PATH"
	} else {
		throw 'Node.js is required. Install the version in package.json and add node.exe to PATH.'
	}
}

foreach ($requiredFile in @($launcher, $electron, $compiledWorkbench)) {
	if (-not (Test-Path -LiteralPath $requiredFile -PathType Leaf)) {
		throw "Development build is incomplete: $requiredFile. Run pnpm run gulp compile and node build/lib/preLaunch.ts first."
	}
}

Write-Host "OP-SE development build: $electron"
Write-Host "Workspace: $workspacePath"
if ($CheckOnly) {
	return
}

if ($SkipPrelaunch) {
	$env:VSCODE_SKIP_PRELAUNCH = '1'
}

Push-Location $repository
try {
	& $launcher $workspacePath
	if ($LASTEXITCODE -ne 0) {
		throw "OP-SE exited with code $LASTEXITCODE"
	}
} finally {
	Pop-Location
}
