# List the Hangul releases installed on this machine and show which one COM currently resolves to.
# Run this first: it tells you which -HwpVersion values page_oracle_run.ps1 will accept.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$CLSID = '{2291CF00-64A1-4877-A9B4-68CFE89612D6}'

Write-Output '=== installed Hangul releases ==='
$found = @()
foreach ($r in @(${env:ProgramFiles(x86)}, $env:ProgramFiles) | Where-Object { $_ }) {
  $hnc = Join-Path $r 'Hnc'
  if (-not (Test-Path -LiteralPath $hnc)) { continue }
  foreach ($office in (Get-ChildItem $hnc -Directory -ErrorAction SilentlyContinue)) {
    $exe = Get-Item -Path (Join-Path $office.FullName 'HOffice*\Bin\Hwp.exe') -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($exe) {
      $pv = $exe.VersionInfo.ProductVersion
      $major = ($pv -split '[.,]')[0].Trim()
      $year = ($office.Name -replace '^\s*Office\s*', '')
      $found += [pscustomobject]@{ Version = $year; Major = $major; ProductVersion = $pv; Path = $exe.FullName }
    }
  }
}
if ($found.Count -eq 0) { Write-Output '  (none found under Hnc)' }
$found | Format-Table -AutoSize

Write-Output '=== COM registration ==='
$hklm = (Get-ItemProperty "HKLM:\SOFTWARE\Classes\WOW6432Node\CLSID\$CLSID\LocalServer32" -ErrorAction SilentlyContinue).'(default)'
Write-Output "  HKLM (machine default): $hklm"
$hkcu = (Get-ItemProperty "HKCU:\Software\Classes\Wow6432Node\CLSID\$CLSID\LocalServer32" -ErrorAction SilentlyContinue).'(default)'
if ($hkcu) { Write-Output "  HKCU (override, wins): $hkcu" } else { Write-Output '  HKCU (override): none -- machine default applies' }

Write-Output '=== what COM actually hands out right now ==='
try {
  $h = New-Object -ComObject HWPFrame.HwpObject
  Write-Output ("  hwp.Version = " + $h.Version)
  $proc = Get-Process Hwp -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($proc) { Write-Output ("  running exe = " + $proc.Path) }
  try { $h.Quit() } catch { }
  [System.Runtime.InteropServices.Marshal]::ReleaseComObject($h) | Out-Null
  Get-Process Hwp -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
} catch {
  Write-Output ("  COM activation failed: " + $_.Exception.Message)
}
