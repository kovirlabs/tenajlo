# Starts the local test Forgejo (compose.yml) and, the first time, generates its secrets and
# creates two users, an access token and a sample repository. Everything you need to sign in
# is written to credentials.txt. Safe to re-run: existing secrets and users are kept.
#
#   powershell -ExecutionPolicy Bypass -File dev\test-server\setup.ps1
#
# Windows twin of setup.sh; keep them doing the same steps. Works in Windows PowerShell 5.1
# and PowerShell 7.
$ErrorActionPreference = 'Stop'
Set-Location $PSScriptRoot

$Image = 'codeberg.org/forgejo/forgejo:11'
$Url = 'http://localhost:3001'

# Native commands don't stop the script when they fail; this does. A plain function (no
# param block), so flags like -f and -T reach docker instead of being bound by PowerShell.
function Invoke-Native {
    $exe = $args[0]
    $rest = @($args | Select-Object -Skip 1)
    $output = & $exe @rest
    if ($LASTEXITCODE -ne 0) { throw "Failed ($LASTEXITCODE): $($args -join ' ')" }
    $output
}

function Invoke-Compose { Invoke-Native docker compose -f compose.yml @args }

# Letters and digits from the OS's secure random generator (no modulo bias).
function New-Password([int]$Length = 20) {
    $chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    $bytes = New-Object byte[] 1
    $result = ''
    while ($result.Length -lt $Length) {
        $rng.GetBytes($bytes)
        if ($bytes[0] -lt 248) { $result += $chars[$bytes[0] % 62] }
    }
    $result
}

# UTF-8 without a byte-order mark and with LF endings: what docker compose expects.
function Write-TextFile([string]$Path, [string[]]$Lines) {
    $full = Join-Path $PSScriptRoot $Path
    [System.IO.File]::WriteAllText($full, (($Lines -join "`n") + "`n"))
}

# 1. Server secrets, generated once with Forgejo's own generator.
if (-not (Test-Path .env)) {
    Write-Host 'Generating server secrets...'
    function New-Secret([string]$Name) {
        (Invoke-Native docker run --rm $Image forgejo generate secret $Name | Out-String).Trim()
    }
    Write-TextFile .env @(
        "FORGEJO__security__SECRET_KEY=$(New-Secret SECRET_KEY)",
        "FORGEJO__security__INTERNAL_TOKEN=$(New-Secret INTERNAL_TOKEN)",
        "FORGEJO__oauth2__JWT_SECRET=$(New-Secret JWT_SECRET)",
        "FORGEJO__server__LFS_JWT_SECRET=$(New-Secret LFS_JWT_SECRET)"
    )
}

# 2. Start it and wait until it answers.
Invoke-Compose up -d | Out-Host
Write-Host 'Waiting for Forgejo...'
$ready = $false
for ($i = 0; $i -lt 60 -and -not $ready; $i++) {
    try {
        Invoke-WebRequest -UseBasicParsing "$Url/api/healthz" | Out-Null
        $ready = $true
    } catch {
        Start-Sleep -Seconds 2
    }
}
if (-not $ready) {
    throw "Forgejo didn't start; see: docker compose -f dev\test-server\compose.yml logs"
}

if (Test-Path credentials.txt) {
    Write-Host 'Already set up.'
    Get-Content credentials.txt
    exit 0
}

# 3. Users: "tester" (you, an admin) and "teammate" (to make changes you then pull).
function Invoke-Forgejo { Invoke-Compose exec -T -u git forgejo forgejo @args }
$testerPw = New-Password
$teammatePw = New-Password
Invoke-Forgejo admin user create --username tester --email tester@example.invalid `
    --password $testerPw --admin --must-change-password=false | Out-Host
Invoke-Forgejo admin user create --username teammate --email teammate@example.invalid `
    --password $teammatePw --must-change-password=false | Out-Host

# 4. An access token with the permissions Tenajlo asks for, plus write:user for SSH keys.
$token = (Invoke-Forgejo admin user generate-access-token --username tester `
        --token-name tenajlo --scopes 'write:user,write:repository' --raw | Out-String).Trim()

# 5. A private sample repository that teammate can also push to.
$basic = [Convert]::ToBase64String([Text.Encoding]::ASCII.GetBytes("tester:$testerPw"))
$headers = @{ Authorization = "Basic $basic" }
$repo = @{ name = 'sample'; private = $true; auto_init = $true; description = 'Try Tenajlo here' }
Invoke-RestMethod -Method Post -Uri "$Url/api/v1/user/repos" -Headers $headers `
    -ContentType 'application/json' -Body ($repo | ConvertTo-Json) | Out-Null
Invoke-RestMethod -Method Put -Uri "$Url/api/v1/repos/tester/sample/collaborators/teammate" `
    -Headers $headers -ContentType 'application/json' -Body '{"permission":"write"}' | Out-Null

Write-TextFile credentials.txt @(
    "Tenajlo local test server (dev/test-server). Keep this file private; it isn't committed.",
    '',
    "Server address:   $Url",
    "Access token:     $token",
    '                  (Tenajlo: Settings > Accounts > sign in with the address and this token)',
    '',
    "tester    (you, admin)   password: $testerPw",
    "teammate  (2nd person)   password: $teammatePw",
    '',
    'Sample repository, private, both users can push:',
    "  HTTPS: $Url/tester/sample.git",
    '  SSH:   ssh://git@localhost:2223/tester/sample.git'
)
Write-Host ''
Get-Content credentials.txt
