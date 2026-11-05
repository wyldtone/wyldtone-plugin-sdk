# Wyldtone Plugin SDK Docker launcher for Windows PowerShell.
# SPDX-License-Identifier: Apache-2.0
# Requires Docker Desktop configured for Linux containers.
$ErrorActionPreference = 'Stop'

$sdkRoot = [System.IO.Path]::GetFullPath($PSScriptRoot).TrimEnd([char[]]@([char]'\', [char]'/'))
$composePath = Join-Path $sdkRoot 'compose.yaml'
if (-not (Test-Path -LiteralPath $composePath -PathType Leaf)) {
    Write-Error "Wyldtone SDK: compose.yaml not found in $sdkRoot"
    exit 1
}
if (-not (Get-Command docker -ErrorAction SilentlyContinue)) {
    Write-Error 'Wyldtone SDK: Docker Desktop is not installed or docker is not on PATH.'
    exit 1
}

# Reuse the Compose UID/GID defaults for Linux containers on Docker Desktop.
# Override externally if the image/workspace requires different IDs.
if ([string]::IsNullOrWhiteSpace($env:HOST_UID)) { $env:HOST_UID = '1000' }
if ([string]::IsNullOrWhiteSpace($env:HOST_GID)) { $env:HOST_GID = '1000' }

# Bind-mounted cache directories must exist before the container starts.
foreach ($subDir in @('.cache/cargo', '.cache/target')) {
    $cachePath = Join-Path $sdkRoot $subDir
    New-Item -ItemType Directory -Force -Path $cachePath | Out-Null
}

# Preserve the current folder when invoked from inside the SDK source tree.
# Docker sees the same relative path under /workspace.
$callerPath = [System.IO.Path]::GetFullPath((Get-Location).ProviderPath).TrimEnd([char[]]@([char]'\', [char]'/'))
$workDir = '/workspace'
if ($callerPath.StartsWith($sdkRoot + [System.IO.Path]::DirectorySeparatorChar,
                            [StringComparison]::OrdinalIgnoreCase)) {
    $relative = $callerPath.Substring($sdkRoot.Length + 1).Replace('\', '/')
    $workDir = '/workspace/' + $relative
}

if ($args.Count -eq 0) { $args = @('help') }

& docker compose --project-directory $sdkRoot -f $composePath run --rm --workdir $workDir sdk @args
if ($null -ne $LASTEXITCODE) { exit $LASTEXITCODE }
exit 1
