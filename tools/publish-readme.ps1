# Upload the repository presentation: README, LICENSE and the screenshots it
# references. Creates or updates each file through the contents API.
#
#   $env:GITHUB_TOKEN = 'ghp_...'
#   powershell -ExecutionPolicy Bypass -File tools\publish-readme.ps1

param(
    [string]$Token = $env:GITHUB_TOKEN,
    [string]$Owner = 'romareyme',
    [string]$Repo = 'material-notepad',
    [string]$Branch = 'main'
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot

if (-not $Token) {
    foreach ($file in @((Join-Path $env:USERPROFILE '.dsh\github-token'), (Join-Path $Root '.github-token'))) {
        if (Test-Path $file) {
            $Token = (Get-Content $file -Raw).Trim()
            if ($Token) { Write-Host "Using token from $file"; break }
        }
    }
}
if (-not $Token) { throw 'No token: set GITHUB_TOKEN, write ~/.dsh/github-token, or pass -Token.' }

$Headers = @{
    Authorization          = "Bearer $Token"
    Accept                 = 'application/vnd.github+json'
    'User-Agent'           = 'material-notepad-publisher'
    'X-GitHub-Api-Version' = '2022-11-28'
}

function Get-Sha {
    param([string]$Path)
    try {
        $uri = "https://api.github.com/repos/$Owner/$Repo/contents/$Path`?ref=$Branch"
        return (Invoke-RestMethod -Method GET -Uri $uri -Headers $Headers).sha
    } catch {
        return $null
    }
}

function Put-File {
    param([string]$Local, [string]$Remote, [string]$Message)

    $bytes = [IO.File]::ReadAllBytes($Local)
    $body = @{
        message = $Message
        content = [Convert]::ToBase64String($bytes)
        branch  = $Branch
    }
    $sha = Get-Sha -Path $Remote
    if ($sha) { $body.sha = $sha }

    $json = $body | ConvertTo-Json -Compress
    $uri = "https://api.github.com/repos/$Owner/$Repo/contents/$Remote"
    $result = Invoke-RestMethod -Method PUT -Uri $uri -Headers $Headers `
        -Body ([Text.Encoding]::UTF8.GetBytes($json)) -ContentType 'application/json; charset=utf-8'
    Write-Host ("{0,-42} {1} ({2:N0} KiB)" -f $Remote, $(if ($sha) { 'updated' } else { 'created' }), ($bytes.Length / 1KB))
}

Put-File -Local (Join-Path $Root 'README.md') -Remote 'README.md' -Message 'Present the project: features, screenshots, install and build'
Put-File -Local (Join-Path $Root 'LICENSE') -Remote 'LICENSE' -Message 'Add the GPL-3.0 license'
Put-File -Local (Join-Path $Root 'MIGRATION.md') -Remote 'MIGRATION.md' -Message 'Document the port from Notepad Next'

# Screenshots referenced by the README.
$shots = Get-ChildItem (Join-Path $Root 'docs\screenshots') -File -Filter *.png
foreach ($shot in $shots) {
    Put-File -Local $shot.FullName -Remote "docs/screenshots/$($shot.Name)" -Message "Add screenshot $($shot.Name)"
}

Write-Host ''
Write-Host "Repository: https://github.com/$Owner/$Repo"
