$ErrorActionPreference = 'Stop'

# scripts/update-packaging.mjs rewrites these two lines for each release.
$url64 = 'https://github.com/kovirlabs/tenajlo/releases/download/v1.4.0/Tenajlo_1.4.0_x64-setup.exe'
$checksum64 = '44d9d316e72cc01ec98a731bc2b65caf0e38136cd8902d407b813feb15401f33'

# Tauri's NSIS installer is per-user (%LocalAppData%\Tenajlo) and needs no admin rights.
# /S runs it silently.
$packageArgs = @{
  packageName    = $env:ChocolateyPackageName
  fileType       = 'exe'
  url64bit       = $url64
  checksum64     = $checksum64
  checksumType64 = 'sha256'
  silentArgs     = '/S'
  validExitCodes = @(0)
  softwareName   = 'Tenajlo*'
}

Install-ChocolateyPackage @packageArgs
