# Startup List Script - Gets all startup items
# Returns JSON with startup programs list

# Fix Chinese encoding: Force UTF-8 output
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

$startupItems = @()

# Registry: Current User Run
$cuRun = Get-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -ErrorAction SilentlyContinue
if ($cuRun) {
    $cuRun.PSObject.Properties | Where-Object { $_.Name -notlike "PS*" } | ForEach-Object {
        $startupItems += @{
            name = $_.Name
            command = $_.Value
            source = "HKCU_Run"
            enabled = $true
            location = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
        }
    }
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
$startupFolder = [Environment]::GetFolderPath("Startup")
if (Test-Path $startupFolder) {
    Get-ChildItem -Path $startupFolder -Force -ErrorAction SilentlyContinue | ForEach-Object {
        $startupItems += @{
            name = $_.BaseName
            command = $_.FullName
            source = "StartupFolder"
            enabled = $true
            location = $startupFolder
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

$count = $startupItems.Count

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
    summary = "$count 个启动项"
    details = @{
        count = $count
        items = $startupItems
    }
}

$output | ConvertTo-Json -Depth 4 -Compress
