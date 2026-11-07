$ErrorActionPreference = 'Stop'

# The NSIS installer registers under HKCU (per-user); Get-UninstallRegistryKey searches HKCU
# as well as HKLM. The uninstaller keeps Tenajlo's settings and accounts when run with /S.
[array]$keys = Get-UninstallRegistryKey -SoftwareName 'Tenajlo'

if ($keys.Count -eq 1) {
  $packageArgs = @{
    packageName    = $env:ChocolateyPackageName
    fileType       = 'exe'
    silentArgs     = '/S'
    validExitCodes = @(0)
    file           = ($keys[0].UninstallString -replace '"', '')
  }
  Uninstall-ChocolateyPackage @packageArgs
}
elseif ($keys.Count -eq 0) {
  Write-Warning "$env:ChocolateyPackageName has already been uninstalled by other means."
}
else {
  Write-Warning "$($keys.Count) matches found for Tenajlo; not uninstalling to be safe:"
  $keys | ForEach-Object { Write-Warning "- $($_.DisplayName)" }
}
