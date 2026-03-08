# Disk Clean Script for Windows - Cleans selected categories
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

$categories = @()
try {
    $parsed = $CategoriesJson | ConvertFrom-Json
    if ($parsed -is [array]) { $categories = $parsed } else { $categories = @($parsed) }
} catch {
    $categories = @()
}

function Contains-Category($name) { return $categories -contains $name }

function Add-Detail($category, $path, $size, $status, $reason) {
    if ($results.details.Count -lt 50) {
        $results.details += @{ category = $category; path = $path; size = $size; status = $status; reason = $reason }
    }
}

function Remove-ItemSafely($path, $isFolder) {
    try {
        if ($isFolder) { Remove-Item -Path $path -Recurse -Force -ErrorAction Stop }
        else { Remove-Item -Path $path -Force -ErrorAction Stop }
        return $true
    } catch {
        try {
            if ($isFolder) { cmd /c "rd /s /q `"$path`" 2>nul" | Out-Null }
            else { cmd /c "del /f /q `"$path`" 2>nul" | Out-Null }
            return -not (Test-Path $path)
        } catch { return $false }
    }
}

# Clean temp files
if (Contains-Category "temp") {
    $tempPath = $env:TEMP
    if ($tempPath -and (Test-Path $tempPath)) {
        Get-ChildItem -Path $tempPath -Force -ErrorAction SilentlyContinue | ForEach-Object {
            try {
                $itemSize = 0
                if ($_.PSIsContainer) {
                    $itemSize = (Get-ChildItem -Path $_.FullName -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum -ErrorAction SilentlyContinue).Sum
                } else { $itemSize = $_.Length }
                $itemSize = if ($itemSize) { [long]$itemSize } else { 0 }
                $deleted = Remove-ItemSafely $_.FullName $_.PSIsContainer
                if ($deleted) {
                    $results.cleaned += $itemSize
                    $results.successCount++
                    Add-Detail "temp" $_.Name $itemSize "success" ""
                } else {
                    $results.failedCount++
                    Add-Detail "temp" $_.Name $itemSize "failed" "File in use"
                }
            } catch { }
        }
    }
}

# Clean system temp
if (Contains-Category "systemTemp") {
    $sysTemp = "C:\Windows\Temp"
    if (Test-Path $sysTemp) {
        Get-ChildItem -Path $sysTemp -Force -ErrorAction SilentlyContinue | ForEach-Object {
            try {
                $itemSize = 0
                if ($_.PSIsContainer) {
                    $itemSize = (Get-ChildItem -Path $_.FullName -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum -ErrorAction SilentlyContinue).Sum
                } else { $itemSize = $_.Length }
                $itemSize = if ($itemSize) { [long]$itemSize } else { 0 }
                $deleted = Remove-ItemSafely $_.FullName $_.PSIsContainer
                if ($deleted) {
                    $results.cleaned += $itemSize
                    $results.successCount++
                } else {
                    $results.failedCount++
                }
            } catch { }
        }
    }
}

# Clean prefetch
if (Contains-Category "prefetch") {
    $prefetchPath = "C:\Windows\Prefetch"
    if (Test-Path $prefetchPath) {
        Get-ChildItem -Path $prefetchPath -Force -ErrorAction SilentlyContinue | ForEach-Object {
            try {
                $itemSize = if ($_.Length) { [long]$_.Length } else { 0 }
                $deleted = Remove-ItemSafely $_.FullName $false
                if ($deleted) {
                    $results.cleaned += $itemSize
                    $results.successCount++
                } else {
                    $results.failedCount++
                }
            } catch { }
        }
    }
}

# Clean recycle bin
if (Contains-Category "recycleBin") {
    try {
        $sizeBefore = 0
        $itemCountBefore = 0
        try {
            $shell = New-Object -ComObject Shell.Application
            $recycleBin = $shell.NameSpace(0x0a)
            if ($recycleBin) {
                $items = $recycleBin.Items()
                if ($items) {
                    $itemCountBefore = $items.Count
                    foreach ($item in $items) {
                        try {
                            $s = $item.ExtendedProperty("Size")
                            if ($s) { $sizeBefore += [long]$s }
                        } catch { }
                    }
                }
            }
        } catch { }

        if ($itemCountBefore -gt 0) {
            $cleared = $false
            $errorMsg = ""

            # Method 1: SHEmptyRecycleBin API
            try {
                $apiCode = 'using System; using System.Runtime.InteropServices; public class RecycleBinAPI { [DllImport("shell32.dll", CharSet = CharSet.Unicode)] public static extern int SHEmptyRecycleBin(IntPtr hwnd, string pszRootPath, int dwFlags); }'
                Add-Type -TypeDefinition $apiCode -ErrorAction SilentlyContinue
                $result = [RecycleBinAPI]::SHEmptyRecycleBin([IntPtr]::Zero, $null, 7)
                if ($result -eq 0) { $cleared = $true }
            } catch {
                $errorMsg = $_.Exception.Message
            }

            # Method 2: Clear-RecycleBin
            if (-not $cleared) {
                try {
                    Clear-RecycleBin -Force -ErrorAction Stop
                    $cleared = $true
                } catch {
                    if (-not $errorMsg) { $errorMsg = $_.Exception.Message }
                }
            }

            # Method 3: Direct deletion
            if (-not $cleared) {
                try {
                    $userSid = ([System.Security.Principal.WindowsIdentity]::GetCurrent()).User.Value
                    $drives = Get-PSDrive -PSProvider FileSystem | Where-Object { $_.Root -match '^[A-Z]:\\$' }
                    foreach ($drive in $drives) {
                        $recyclePath = Join-Path $drive.Root '$Recycle.Bin'
                        if (Test-Path $recyclePath) {
                            $userRecyclePath = Join-Path $recyclePath $userSid
                            if (Test-Path $userRecyclePath) {
                                Get-ChildItem -Path $userRecyclePath -Force -ErrorAction SilentlyContinue | ForEach-Object {
                                    Remove-ItemSafely $_.FullName $_.PSIsContainer | Out-Null
                                }
                            }
                        }
                    }
                } catch {
                    if (-not $errorMsg) { $errorMsg = $_.Exception.Message }
                }
            }

            # Verify
            $itemCountAfter = 0
            try {
                $shell2 = New-Object -ComObject Shell.Application
                $recycleBin2 = $shell2.NameSpace(0x0a)
                if ($recycleBin2 -and $recycleBin2.Items()) {
                    $itemCountAfter = $recycleBin2.Items().Count
                }
            } catch { }

            $actualCleared = $itemCountBefore - $itemCountAfter
            if ($actualCleared -gt 0) {
                $clearedSize = 0
                if ($itemCountBefore -gt 0) { $clearedSize = [long]($sizeBefore * $actualCleared / $itemCountBefore) }
                $results.cleaned += $clearedSize
                $results.successCount += $actualCleared
                Add-Detail "recycleBin" "RecycleBin" $clearedSize "success" "Cleared $actualCleared items"
            } else {
                $results.failedCount++
                $reason = "Cannot clear recycle bin"
                if ($errorMsg) { $reason = $errorMsg }
                Add-Detail "recycleBin" "RecycleBin" 0 "failed" $reason
            }
        } else {
            Add-Detail "recycleBin" "RecycleBin" 0 "success" "Already empty"
        }
    } catch {
        $results.failedCount++
        Add-Detail "recycleBin" "RecycleBin" 0 "failed" $_.Exception.Message
    }
}

# Clean browser caches
if (Contains-Category "browserCache") {
    $browserResults = @{}

    function Is-BrowserRunning($browserName) {
        $processName = ""
        if ($browserName -eq "Chrome") { $processName = "chrome" }
        elseif ($browserName -eq "Edge") { $processName = "msedge" }
        elseif ($browserName -eq "Firefox") { $processName = "firefox" }
        if ($processName) { return $null -ne (Get-Process -Name $processName -ErrorAction SilentlyContinue) }
        return $false
    }

    $browserPaths = @()

    # Chrome
    $chromeUserData = "$env:LOCALAPPDATA\Google\Chrome\User Data"
    if (Test-Path $chromeUserData) {
        Get-ChildItem -Path $chromeUserData -Directory -ErrorAction SilentlyContinue | ForEach-Object {
            $profilePath = $_.FullName
            @("Cache", "Code Cache", "GPUCache") | ForEach-Object {
                $cachePath = Join-Path $profilePath $_
                if (Test-Path $cachePath) { $browserPaths += @{ path = $cachePath; browser = "Chrome" } }
            }
        }
    }

    # Edge
    $edgeUserData = "$env:LOCALAPPDATA\Microsoft\Edge\User Data"
    if (Test-Path $edgeUserData) {
        Get-ChildItem -Path $edgeUserData -Directory -ErrorAction SilentlyContinue | ForEach-Object {
            $profilePath = $_.FullName
            @("Cache", "Code Cache", "GPUCache") | ForEach-Object {
                $cachePath = Join-Path $profilePath $_
                if (Test-Path $cachePath) { $browserPaths += @{ path = $cachePath; browser = "Edge" } }
            }
        }
    }

    # Firefox
    $firefoxProfiles = "$env:LOCALAPPDATA\Mozilla\Firefox\Profiles"
    if (Test-Path $firefoxProfiles) {
        Get-ChildItem -Path $firefoxProfiles -Directory -ErrorAction SilentlyContinue | ForEach-Object {
            $cachePath = Join-Path $_.FullName "cache2"
            if (Test-Path $cachePath) { $browserPaths += @{ path = $cachePath; browser = "Firefox" } }
        }
    }

    $browserPaths | ForEach-Object {
        if (-not $browserResults.ContainsKey($_.browser)) {
            $browserResults[$_.browser] = @{ sizeBefore = 0; sizeAfter = 0; isRunning = (Is-BrowserRunning $_.browser) }
        }
    }

    foreach ($item in $browserPaths) {
        $bpath = $item.path
        $browser = $item.browser
        try {
            $pathSizeBefore = 0
            Get-ChildItem -Path $bpath -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object {
                if (-not $_.PSIsContainer -and $_.Length) { $pathSizeBefore += $_.Length }
            }
            $browserResults[$browser].sizeBefore += $pathSizeBefore

            if ($pathSizeBefore -gt 0) {
                Get-ChildItem -Path $bpath -Force -ErrorAction SilentlyContinue | ForEach-Object {
                    Remove-ItemSafely $_.FullName $_.PSIsContainer | Out-Null
                }
                $pathSizeAfter = 0
                Get-ChildItem -Path $bpath -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object {
                    if (-not $_.PSIsContainer -and $_.Length) { $pathSizeAfter += $_.Length }
                }
                $browserResults[$browser].sizeAfter += $pathSizeAfter
            }
        } catch { }
    }

    foreach ($browser in $browserResults.Keys) {
        $br = $browserResults[$browser]
        $actualCleaned = $br.sizeBefore - $br.sizeAfter
        if ($br.sizeBefore -eq 0) { continue }
        if ($actualCleaned -gt 0) {
            $results.cleaned += $actualCleaned
            $results.successCount++
            $cleanedMB = [math]::Round($actualCleaned / 1MB, 2)
            Add-Detail "browserCache" $browser $actualCleaned "success" "Cleared ${cleanedMB}MB"
        } else {
            $results.failedCount++
            $reason = "Files locked"
            if ($br.isRunning) { $reason = "Close $browser first" }
            Add-Detail "browserCache" $browser 0 "failed" $reason
        }
    }
}

# Output
$cleanedMB = [math]::Round($results.cleaned / 1MB, 2)
$output = @{
    cleaned = $results.cleaned
    cleanedMB = $cleanedMB
    successCount = $results.successCount
    failedCount = $results.failedCount
    details = $results.details
}
$output | ConvertTo-Json -Depth 3 -Compress
