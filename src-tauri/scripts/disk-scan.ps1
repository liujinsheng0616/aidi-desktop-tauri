# Disk Scan Script - Scans for junk files
# Returns JSON with file list and total size

# Fix Chinese encoding: Force UTF-8 output
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

$results = @{
    files = @()
    totalSize = 0
    categories = @{
        temp = @{ size = 0; count = 0 }
        systemTemp = @{ size = 0; count = 0 }
        prefetch = @{ size = 0; count = 0 }
        recycleBin = @{ size = 0; count = 0 }
        browserCache = @{ size = 0; count = 0 }
    }
}

function Add-Files($path, $category) {
    if (Test-Path $path) {
        Get-ChildItem -Path $path -Recurse -Force -ErrorAction SilentlyContinue |
        Where-Object { !$_.PSIsContainer } |
        ForEach-Object {
            $results.files += @{
                path = $_.FullName
                size = $_.Length
                modified = $_.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss")
                category = $category
            }
            $results.totalSize += $_.Length
            $results.categories[$category].size += $_.Length
            $results.categories[$category].count++
        }
    }
}

# User temp files
Add-Files $env:TEMP "temp"

# System temp files
Add-Files "C:\Windows\Temp" "systemTemp"

# Prefetch files
Add-Files "C:\Windows\Prefetch" "prefetch"

# Recycle Bin
$shell = New-Object -ComObject Shell.Application
$recycleBin = $shell.NameSpace(0x0a)
if ($recycleBin) {
    $recycleBin.Items() | ForEach-Object {
        $size = $_.ExtendedProperty("Size")
        if ($size) {
            $results.files += @{
                path = $_.Path
                size = $size
                modified = ""
                category = "recycleBin"
            }
            $results.totalSize += $size
            $results.categories["recycleBin"].size += $size
            $results.categories["recycleBin"].count++
        }
    }
}

# Browser caches ONLY (not history, passwords, cookies, bookmarks)
$browserPaths = @(
    "$env:LOCALAPPDATA\Google\Chrome\User Data\Default\Cache",
    "$env:LOCALAPPDATA\Google\Chrome\User Data\Default\Code Cache",
    "$env:LOCALAPPDATA\Microsoft\Edge\User Data\Default\Cache",
    "$env:LOCALAPPDATA\Microsoft\Edge\User Data\Default\Code Cache"
)

# Firefox cache is in a different location with random profile folder
$firefoxProfiles = "$env:LOCALAPPDATA\Mozilla\Firefox\Profiles"
if (Test-Path $firefoxProfiles) {
    Get-ChildItem -Path $firefoxProfiles -Directory | ForEach-Object {
        $browserPaths += "$($_.FullName)\cache2"
    }
}

foreach ($browserPath in $browserPaths) {
    if (Test-Path $browserPath) {
        Get-ChildItem -Path $browserPath -Recurse -Force -ErrorAction SilentlyContinue |
        Where-Object { !$_.PSIsContainer } |
        ForEach-Object {
            $results.files += @{
                path = $_.FullName
                size = $_.Length
                modified = $_.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss")
                category = "browserCache"
            }
            $results.totalSize += $_.Length
            $results.categories["browserCache"].size += $_.Length
            $results.categories["browserCache"].count++
        }
    }
}

# Determine status
$sizeGB = $results.totalSize / 1GB
if ($sizeGB -lt 0.5) {
    $status = "good"
} elseif ($sizeGB -lt 2) {
    $status = "warning"
} else {
    $status = "danger"
}

$output = @{
    dimension = "disk"
    status = $status
    summary = [string]::Format("{0:N2} GB", $sizeGB) + " junk files found"
    details = $results
}

$output | ConvertTo-Json -Depth 5 -Compress
