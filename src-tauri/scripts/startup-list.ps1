# Startup List Script - Gets all startup items
# Returns JSON with startup programs list

# Fix Chinese encoding: Force UTF-8 output
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

$startupItems = @()

# Registry: Current User Run
$registry = [Microsoft.Win32.Registry]::CurrentUser
$runPath = "Software\Microsoft\Windows\CurrentVersion\Run"
$runKey = $registry.OpenSubKey($runPath)
$backupKey = $registry.OpenSubKey("Software\AIDI\DisabledStartup\Run")
try {
    $activeNames = @()
    if ($runKey) {
        $activeNames = @($runKey.GetValueNames() | Where-Object { $_ })
        foreach ($name in $activeNames) {
            $startupItems += @{
                name = $name
                command = [string]$runKey.GetValue($name)
                source = "HKCU_Run"
                enabled = $true
                location = "HKCU:\$runPath"
            }
        }
    }
    if ($backupKey) {
        foreach ($name in $backupKey.GetValueNames()) {
            if (!$name -or $activeNames -contains $name) { continue }
            $startupItems += @{
                name = $name
                command = ""
                source = "HKCU_Run"
                enabled = $false
                location = "HKCU:\$runPath"
            }
        }
    }
} finally {
    if ($runKey) { $runKey.Dispose() }
    if ($backupKey) { $backupKey.Dispose() }
    $registry.Dispose()
}

# Registry: Local Machine Run
$lmRun = Get-ItemProperty -Path "HKLM:\Software\Microsoft\Windows\CurrentVersion\Run" -ErrorAction SilentlyContinue
if ($lmRun) {
    $lmRun.PSObject.Properties | Where-Object { $_.Name -notlike "PS*" } | ForEach-Object {
        $startupItems += @{
            name = $_.Name
            command = $_.Value
            source = "HKLM_Run"
            enabled = $true
            location = "HKLM:\Software\Microsoft\Windows\CurrentVersion\Run"
        }
    }
}

# Startup Folder - Current User
$startupFolder = [Environment]::GetFolderPath(
    [Environment+SpecialFolder]::Startup, [Environment+SpecialFolderOption]::DoNotVerify)
if ($startupFolder -and (Test-Path -LiteralPath $startupFolder)) {
    Get-ChildItem -LiteralPath $startupFolder -File -Force | Where-Object Name -ne 'desktop.ini' | ForEach-Object {
        $startupItems += @{
            name = $_.BaseName
            command = $_.FullName
            source = "StartupFolder"
            enabled = $true
            location = $startupFolder
        }
    }
}
$disabledFolder = Join-Path $env:APPDATA "AIDI\DisabledStartup"
if ($startupFolder -and (Test-Path -LiteralPath $disabledFolder)) {
    Get-ChildItem -LiteralPath $disabledFolder -File -Force | Where-Object Name -ne 'desktop.ini' | ForEach-Object {
        $activePath = Join-Path $startupFolder $_.Name
        if (!(Test-Path -LiteralPath $activePath)) {
            $startupItems += @{
                name = $_.BaseName
                command = $activePath
                source = "StartupFolder"
                enabled = $false
                location = $startupFolder
            }
        }
    }
}

# Startup Folder - All Users
$allUsersStartup = "$env:ProgramData\Microsoft\Windows\Start Menu\Programs\Startup"
if (Test-Path $allUsersStartup) {
    Get-ChildItem -Path $allUsersStartup -Force -ErrorAction SilentlyContinue | ForEach-Object {
        $startupItems += @{
            name = $_.BaseName
            command = $_.FullName
            source = "AllUsersStartup"
            enabled = $true
            location = $allUsersStartup
        }
    }
}

$count = @($startupItems | Where-Object enabled).Count

# Determine status
if ($count -lt 15) {
    $status = "good"
} elseif ($count -lt 25) {
    $status = "warning"
} else {
    $status = "danger"
}

$output = @{
    dimension = "startup"
    status = $status
    summary = "$count 个已启用启动项"
    details = @{
        count = $count
        items = $startupItems
    }
}

$output | ConvertTo-Json -Depth 4 -Compress
