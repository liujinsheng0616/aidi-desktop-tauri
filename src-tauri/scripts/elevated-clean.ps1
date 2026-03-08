# Elevated Clean Script for Windows - Cleans categories requiring admin privileges
# Input: JSON array of category keys

# Fix Chinese encoding: Force UTF-8 output
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$CategoriesJson = if ($env:SCRIPT_ARGS) { $env:SCRIPT_ARGS } else { '[]' }

$ErrorActionPreference = "SilentlyContinue"

$results = @{
    cleaned = 0
    successCount = 0
    failedCount = 0
    details = @()
}

$MAX_DETAILS = 50

# Parse categories
$categories = @()
try {
    $parsed = $CategoriesJson | ConvertFrom-Json
    if ($parsed -is [array]) {
        $categories = $parsed
    } else {
        $categories = @($parsed)
    }
} catch {
    $categories = @()
}

function Add-Detail($category, $path, $size, $status, $reason) {
    if ($results.details.Count -lt $MAX_DETAILS) {
        $results.details += @{
            category = $category
            path = $path
            size = $size
            status = $status
            reason = $reason
        }
    }
}

# Helper function to safely delete files/folders
function Remove-ItemSafely($path, $isFolder) {
    $deleted = $false
    try {
        if ($isFolder) {
            Remove-Item -Path $path -Recurse -Force -ErrorAction Stop
            $deleted = $true
        } else {
            Remove-Item -Path $path -Force -ErrorAction Stop
            $deleted = $true
        }
    } catch {
        try {
            if ($isFolder) {
                cmd /c "rd /s /q `"$path`" 2>nul" | Out-Null
            } else {
                cmd /c "del /f /q `"$path`" 2>nul" | Out-Null
            }
            $deleted = -not (Test-Path $path)
        } catch {}
    }
    return $deleted
}

# Clean Windows System Temp directory (requires admin)
if ($categories -contains "systemTemp") {
    $sysTemp = "C:\Windows\Temp"
    if (Test-Path $sysTemp) {
        $items = Get-ChildItem -Path $sysTemp -Force -ErrorAction SilentlyContinue
        foreach ($item in $items) {
            try {
                $itemSize = 0
                if ($item.PSIsContainer) {
                    $itemSize = (Get-ChildItem -Path $item.FullName -Recurse -Force -ErrorAction SilentlyContinue |
                                 Measure-Object -Property Length -Sum -ErrorAction SilentlyContinue).Sum
                } else {
                    $itemSize = $item.Length
                }
                $itemSize = if ($itemSize) { [long]$itemSize } else { 0 }

                $deleted = Remove-ItemSafely $item.FullName $item.PSIsContainer

                if ($deleted) {
                    $results.cleaned += $itemSize
                    $results.successCount++
                    Add-Detail "systemTemp" $item.Name $itemSize "success" ""
                } else {
                    $results.failedCount++
                    Add-Detail "systemTemp" $item.Name $itemSize "failed" "文件被占用"
                }
            } catch {}
        }
    }
}

# Clean Prefetch directory (requires admin)
if ($categories -contains "prefetch") {
    $prefetchPath = "C:\Windows\Prefetch"
    if (Test-Path $prefetchPath) {
        $items = Get-ChildItem -Path $prefetchPath -Force -ErrorAction SilentlyContinue
        foreach ($item in $items) {
            try {
                $itemSize = if ($item.Length) { [long]$item.Length } else { 0 }

                $deleted = Remove-ItemSafely $item.FullName $false

                if ($deleted) {
                    $results.cleaned += $itemSize
                    $results.successCount++
                    Add-Detail "prefetch" $item.Name $itemSize "success" ""
                } else {
                    $results.failedCount++
                    Add-Detail "prefetch" $item.Name $itemSize "failed" "文件被占用"
                }
            } catch {}
        }
    }
}

# Output results
$cleanedMB = [math]::Round($results.cleaned / 1MB, 2)

$output = @{
    cleaned = $results.cleaned
    cleanedMB = $cleanedMB
    successCount = $results.successCount
    failedCount = $results.failedCount
    details = $results.details
}

$output | ConvertTo-Json -Depth 3 -Compress
