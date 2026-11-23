# Upload the project sources to GitHub in one run.
#
# Uploads a curated tree (not the build output, not the throwaway probe scripts)
# through the contents API, so no git installation is needed.
#
#   $env:GITHUB_TOKEN = 'ghp_...'
#   powershell -ExecutionPolicy Bypass -File tools\publish-source.ps1

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

# Directories and files that make up the project.
$Include = @(
    'Cargo.toml', 'Cargo.lock', 'build.rs', '.gitignore', 'LICENSE', 'README.md', 'MIGRATION.md',
    'src\*', 'assets\icon.ico', 'assets\icons\*', 'assets\fonts\*',
    'installer\*', 'vendor\egui-winit\*', 'docs\screenshots\*'
)

# The tools worth shipping: generators and the publishing scripts.
$Tools = @(
    'make_icons.py', 'check_icons.py', 'dump_frames.py', 'gen_installer.py',
    'install-per-user.ps1', 'publish-github.ps1', 'publish-readme.ps1',
    'publish-source.ps1', 'shot-state.ps1', 'shots.ps1'
)

$files = New-Object System.Collections.Generic.List[object]
foreach ($pattern in $Include) {
    $full = Join-Path $Root $pattern
    Get-ChildItem -Path $full -File -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -notmatch '\\target\\|\\dist\\|\\\.git\\' } |
        ForEach-Object { $files.Add($_) }
}
foreach ($name in $Tools) {
    $path = Join-Path $Root "tools\$name"
    if (Test-Path $path) { $files.Add((Get-Item $path)) }
}

$files = $files | Sort-Object FullName -Unique

function Put-File {
    param([string]$Local, [string]$Remote)

    $bytes = [IO.File]::ReadAllBytes($Local)
    $body = @{
        message = "Add $Remote"
        content = [Convert]::ToBase64String($bytes)
        branch  = $Branch
    }
    try {
        $existing = Invoke-RestMethod -Uri "https://api.github.com/repos/$Owner/$Repo/contents/$Remote`?ref=$Branch" -Headers $Headers
        $body.sha = $existing.sha
        $body.message = "Update $Remote"
    } catch { }

    $json = $body | ConvertTo-Json -Compress
    Invoke-RestMethod -Method PUT -Uri "https://api.github.com/repos/$Owner/$Repo/contents/$Remote" `
        -Headers $Headers -Body ([Text.Encoding]::UTF8.GetBytes($json)) `
        -ContentType 'application/json; charset=utf-8' | Out-Null
    Write-Host ("  {0}" -f $Remote)
}

Write-Host "Uploading $($files.Count) files to $Owner/$Repo ($Branch)"
foreach ($file in $files) {
    $relative = $file.FullName.Substring($Root.Length + 1) -replace '\\', '/'
    Put-File -Local $file.FullName -Remote $relative
}
Write-Host ''
Write-Host "Done: https://github.com/$Owner/$Repo"
