# Publish Material Notepad to GitHub: create the repository, cut a release and
# upload the installer as a release asset.
#
# Needs a personal access token with the `repo` scope (classic) or Contents +
# Administration (fine-grained). Pass it with -Token or set GITHUB_TOKEN.
#
#   $env:GITHUB_TOKEN = 'ghp_...'
#   powershell -ExecutionPolicy Bypass -File tools\publish-github.ps1
#
#   powershell -ExecutionPolicy Bypass -File tools\publish-github.ps1 `
#       -Token 'ghp_...' -RepoName material-notepad -Private
#
# Draft: nothing is published publicly until the release is published on GitHub
# unless -Publish is given.

param(
    [string]$Token = $env:GITHUB_TOKEN,
    [string]$Owner = '',
    [string]$RepoName = 'material-notepad',
    [string]$Description = 'Material Notepad — Notepad Next rewritten in Rust with a Material You (Material 3) interface',
    [switch]$Private,
    [string]$Tag = 'v1.0.0',
    [switch]$Publish,
    [string]$Artifacts = ''
)

$ErrorActionPreference = 'Stop'

if (-not $Token) {
    # Same places the DSH GitHub plugin reads: a token file, so the secret never
    # has to be pasted into a chat or a command line.
    $candidates = @(
        (Join-Path $env:USERPROFILE '.dsh\github-token'),
        (Join-Path (Get-Location) '.github-token')
    )
    foreach ($file in $candidates) {
        if (Test-Path $file) {
            $Token = (Get-Content $file -Raw).Trim()
            if ($Token) {
                Write-Host "Using token from $file"
                break
            }
        }
    }
}

if (-not $Token) {
    throw 'No token. Write it to %USERPROFILE%\.dsh\github-token (one line) or set GITHUB_TOKEN, or pass -Token.'
}

$Api = 'https://api.github.com'
$Uploads = 'https://uploads.github.com'
$Headers = @{
    Authorization          = "Bearer $Token"
    Accept                 = 'application/vnd.github+json'
    'User-Agent'           = 'material-notepad-publisher'
    'X-GitHub-Api-Version' = '2022-11-28'
}

function Invoke-GitHub {
    param(
        [string]$Method,
        [string]$Uri,
        $Body
    )
    # $args is a reserved automatic variable in PowerShell, so the splat table
    # must live under another name.
    $request = @{ Method = $Method; Uri = $Uri; Headers = $Headers }
    if ($null -ne $Body) {
        $json = $Body | ConvertTo-Json -Depth 10 -Compress
        $request.Body = [Text.Encoding]::UTF8.GetBytes($json)
        $request.ContentType = 'application/json; charset=utf-8'
    }
    Invoke-RestMethod @request
}

if (-not $Owner) {
    $me = Invoke-GitHub -Method GET -Uri "$Api/user"
    $Owner = $me.login
    Write-Host "Authenticated as $Owner"
}

# 1. Repository (created when missing).
$repoUri = "$Api/repos/$Owner/$RepoName"
$repo = $null
try {
    $repo = Invoke-GitHub -Method GET -Uri $repoUri
    Write-Host "Repository already exists: $($repo.html_url)"
} catch {
    $repo = Invoke-GitHub -Method POST -Uri "$Api/user/repos" -Body @{
        name        = $RepoName
        description = $Description
        private     = [bool]$Private
        has_issues  = $true
        has_wiki    = $false
    }
    Write-Host "Created repository: $($repo.html_url)"
}

# 1b. Seed the repository: a release needs at least one commit to point its tag at.
try {
    Invoke-GitHub -Method GET -Uri "$Api/repos/$Owner/$RepoName/contents/README.md" | Out-Null
} catch {
    $readme = @'
# Material Notepad

Notepad Next rewritten in Rust with a Material You (Material 3) interface.

* Thin 40 px top bar, no side rail: the text fills the window.
* Per-type document icons with a coloured extension badge.
* `Ctrl` + mouse wheel zooms the editor font.
* Per-user installer that never asks for elevation.

Downloads are attached to the latest release.
'@
    Invoke-GitHub -Method PUT -Uri "$Api/repos/$Owner/$RepoName/contents/README.md" -Body @{
        message = 'Initial commit'
        content = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($readme))
        branch  = 'main'
    } | Out-Null
    Write-Host 'Seeded the repository with an initial commit'
}

# 2. Release.
if (-not $Artifacts) {
    $Artifacts = Join-Path (Split-Path -Parent $PSScriptRoot) 'dist'
}

$assets = Get-ChildItem -Path $Artifacts -File |
    Where-Object { $_.Name -match 'setup\.exe$|portable\.zip$|^material-notepad\.exe$' } |
    Sort-Object Length -Descending
if (-not $assets) { throw "No artifacts found in $Artifacts" }

$checksums = (Get-ChildItem -Path $Artifacts -File |
    Where-Object { $assets.Name -contains $_.Name } |
    ForEach-Object { '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name }) -join "`n"

$notes = @'
Material Notepad 1.0.0 - Notepad Next rewritten in Rust with a Material You (Material 3) interface.

* Thin 40 px top bar, no side rail: the text fills the window.
* Tab titles elide inside the tab; long names no longer overflow.
* Ctrl + mouse wheel zooms the editor font.
* Ctrl+A / Backspace work in the editor (a stale caret index used to close the window).
* Material You document icons with a coloured extension badge (txt, cfg, json, md, ini, log, csv).
* Per-user installer that never asks for elevation; the portable archive installs without %TEMP%.

SHA-256:

```
@CHECKSUMS@
```
'@
$notes = $notes.Replace('@CHECKSUMS@', $checksums)

$existing = $null
try {
    $existing = Invoke-GitHub -Method GET -Uri "$Api/repos/$Owner/$RepoName/releases/tags/$Tag"
    Write-Host "Release $Tag already exists: $($existing.html_url)"
} catch {
    $existing = Invoke-GitHub -Method POST -Uri "$Api/repos/$Owner/$RepoName/releases" -Body @{
        tag_name         = $Tag
        name             = "Material Notepad 1.0.0"
        body             = $notes
        draft            = -not $Publish
        prerelease       = $false
        target_commitish = 'main'
    }
    Write-Host "Created release $Tag ($(if ($Publish) { 'published' } else { 'draft' }))"
}

# 3. Assets.
foreach ($asset in $assets) {
    $name = $asset.Name
    if ($existing.assets | Where-Object { $_.name -eq $name }) {
        Write-Host "Asset already uploaded: $name"
        continue
    }
    $uri = "$Uploads/repos/$Owner/$RepoName/releases/$($existing.id)/assets?name=$([uri]::EscapeDataString($name))"
    $response = Invoke-RestMethod -Method POST -Uri $uri -Headers $Headers `
        -ContentType 'application/octet-stream' -InFile $asset.FullName
    Write-Host ("Uploaded {0} ({1:N1} MiB) -> {2}" -f $name, ($asset.Length / 1MB), $response.browser_download_url)
}

Write-Host ''
Write-Host "Repository: $($repo.html_url)"
Write-Host "Release:    $($existing.html_url)"
if (-not $Publish) {
    Write-Host 'The release is a draft: open the link above and press Publish to make it public.'
}
