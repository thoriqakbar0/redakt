#Requires -Version 5.1
[CmdletBinding()]
param(
    [string] $InstallDir
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($env:OS -ne 'Windows_NT' -or $env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
    throw 'This installer requires native x64 Windows PowerShell on Windows x64.'
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        throw 'LOCALAPPDATA is unavailable. Supply -InstallDir with a writable directory.'
    }
    $InstallDir = Join-Path $env:LOCALAPPDATA 'redakt\bin'
}
$InstallDir = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($InstallDir)

$repository = 'thoriqakbar0/redakt'
$releaseBase = 'https://redakt.ta-0.com/releases'

function Get-ReleaseAsset($Asset, $Destination, $Tag) {
    $releasePath = if ($Tag) { "download/$Tag" } else { 'latest/download' }
    try {
        Invoke-WebRequest -UseBasicParsing -Uri "$releaseBase/$releasePath/$Asset" -OutFile $Destination
    }
    catch {
        if (-not (Get-Command gh -CommandType Application -ErrorAction SilentlyContinue)) {
            throw
        }
        $tagArgs = @(if ($Tag) { $Tag })
        gh release download @tagArgs --repo $repository --pattern $Asset --output $Destination --clobber
        if ($LASTEXITCODE -ne 0) {
            throw "Could not download $Asset with GitHub CLI."
        }
    }
}

$temporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ('redakt-' + [guid]::NewGuid())
$stagingDirectory = $null
$previousSecurityProtocol = [Net.ServicePointManager]::SecurityProtocol

try {
    [Net.ServicePointManager]::SecurityProtocol = $previousSecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    New-Item -ItemType Directory -Path $temporaryDirectory | Out-Null
    $checksumsPath = Join-Path $temporaryDirectory 'SHA256SUMS'
    try {
        Get-ReleaseAsset -Asset SHA256SUMS -Destination $checksumsPath
    }
    catch {
        throw "Could not download the latest release checksums. For a private repository, run gh auth login with an account that has access. $($_.Exception.Message)"
    }

    $assetPattern = 'redakt-(?<tag>v[0-9]+\.[0-9]+\.[0-9]+)-x86_64-pc-windows-msvc\.zip'
    $checksumLines = @([IO.File]::ReadAllLines($checksumsPath) | Where-Object {
        $_ -cmatch ('(?:^|[ \t*])' + $assetPattern + '$')
    })
    if ($checksumLines.Count -eq 0) {
        throw 'The latest release has no redakt Windows x64 archive. You can install with cargo install redakt --locked.'
    }
    if ($checksumLines.Count -ne 1) {
        throw 'SHA256SUMS contains multiple Windows x64 entries. No files were installed.'
    }
    $checksumPattern = '^(?<digest>[0-9a-fA-F]{64})[ \t]+\*?(?<archive>' + $assetPattern + ')$'
    if ($checksumLines[0] -cnotmatch $checksumPattern) {
        throw 'The Windows x64 checksum entry is invalid. No files were installed.'
    }
    $expectedDigest = $Matches.digest
    $archiveName = $Matches.archive
    $releaseTag = $Matches.tag
    $archivePath = Join-Path $temporaryDirectory $archiveName
    Get-ReleaseAsset -Asset $archiveName -Destination $archivePath -Tag $releaseTag
    $actualDigest = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash
    if ($actualDigest -ine $expectedDigest) {
        throw 'The archive SHA-256 checksum does not match. No files were installed.'
    }

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    $stagingDirectory = Join-Path $InstallDir ('.redakt-' + [guid]::NewGuid())
    New-Item -ItemType Directory -Path $stagingDirectory | Out-Null
    $stagedBinary = Join-Path $stagingDirectory 'redakt.exe'
    $expectedEntry = $archiveName.Substring(0, $archiveName.Length - 4) + '/redakt.exe'
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $archive = [IO.Compression.ZipFile]::OpenRead($archivePath)
    try {
        $binaryEntries = @($archive.Entries | Where-Object { $_.FullName -ceq $expectedEntry })
        if ($binaryEntries.Count -ne 1 -or $binaryEntries[0].Length -eq 0) {
            throw 'The archive does not contain exactly one expected redakt.exe. No files were installed.'
        }
        [IO.Compression.ZipFileExtensions]::ExtractToFile($binaryEntries[0], $stagedBinary)
    }
    finally {
        $archive.Dispose()
    }

    $versionOutput = & $stagedBinary --version
    if ($LASTEXITCODE -ne 0 -or ($versionOutput -join "`n") -cne ('redakt ' + $releaseTag.Substring(1))) {
        throw 'The downloaded executable failed its version check. No files were installed.'
    }

    $installedBinary = Join-Path $InstallDir 'redakt.exe'
    if ([IO.File]::Exists($installedBinary)) {
        $previousBinary = Join-Path $stagingDirectory 'previous-redakt.exe'
        [IO.File]::Replace($stagedBinary, $installedBinary, $previousBinary)
    }
    else {
        [IO.File]::Move($stagedBinary, $installedBinary)
    }
    Write-Output "Installed $versionOutput at $installedBinary"
    if (($env:Path -split ';') -notcontains $InstallDir) {
        Write-Output "Add $InstallDir to your user Path to run redakt by name."
    }
}
finally {
    [Net.ServicePointManager]::SecurityProtocol = $previousSecurityProtocol
    foreach ($directory in @($stagingDirectory, $temporaryDirectory)) {
        if ($directory -and (Test-Path -LiteralPath $directory)) {
            Remove-Item -LiteralPath $directory -Recurse -Force -ErrorAction Continue
        }
    }
}
