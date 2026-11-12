param(
	[Parameter(Position = 0)]
	[string]$Workspace = (Get-Location).Path,
	[switch]$SkipPrelaunch,
	[switch]$AiDevelopmentHost = $true,
	[switch]$CheckOnly
)

$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$launcher = Join-Path $PSScriptRoot 'code.bat'
$electron = Join-Path $repository '.build\electron\OP-SE.exe'
$compiledWorkbench = Join-Path $repository 'out\vs\code\electron-browser\workbench\workbench-dev.html'
$aiExtension = Join-Path $repository 'extensions\op-se-ai'

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

$requiredFiles = @($launcher, $electron, $compiledWorkbench, (Join-Path $aiExtension 'package.json'), (Join-Path $aiExtension 'extension.js'), (Join-Path $aiExtension 'provider.js'), (Join-Path $aiExtension 'media\ai.svg'))
foreach ($requiredFile in $requiredFiles) {
	if (-not (Test-Path -LiteralPath $requiredFile -PathType Leaf)) {
		throw "Development build is incomplete: $requiredFile. Run pnpm run gulp compile and node build/lib/preLaunch.ts first."
	}
}

Write-Host "OP-SE development build: $electron"
Write-Host "Workspace: $workspacePath"
Write-Host "AI extension: $aiExtension"
if ($CheckOnly) {
	return
}

if ($SkipPrelaunch) {
	$env:VSCODE_SKIP_PRELAUNCH = '1'
}

Push-Location $repository
try {
	if ($AiDevelopmentHost) {
		& $launcher "--extensionDevelopmentPath=$aiExtension" $workspacePath
	} else {
		& $launcher $workspacePath
	}
	if ($LASTEXITCODE -ne 0) {
		throw "OP-SE exited with code $LASTEXITCODE"
	}
} finally {
	Pop-Location
}
