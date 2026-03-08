# Memory Status Script - Gets memory usage and top processes
# Returns JSON with memory info and process list

# Fix Chinese encoding: Force UTF-8 output
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

$os = Get-CimInstance Win32_OperatingSystem
$totalMemory = $os.TotalVisibleMemorySize * 1KB
$freeMemory = $os.FreePhysicalMemory * 1KB
$usedMemory = $totalMemory - $freeMemory
$usedPercent = [math]::Round(($usedMemory / $totalMemory) * 100, 1)
$availablePercent = 100 - $usedPercent

# Get top 10 memory-consuming processes
$topProcesses = Get-Process |
    Sort-Object WorkingSet64 -Descending |
    Select-Object -First 10 |
    ForEach-Object {
        @{
            name = $_.ProcessName
            pid = $_.Id
            memory = $_.WorkingSet64
            memoryMB = [math]::Round($_.WorkingSet64 / 1MB, 1)
        }
    }

# Determine status based on available memory
if ($availablePercent -gt 30) {
    $status = "good"
} elseif ($availablePercent -gt 15) {
    $status = "warning"
} else {
    $status = "danger"
}

$output = @{
    dimension = "memory"
    status = $status
    summary = "内存已使用 $usedPercent%，可用 $availablePercent%"
    details = @{
        total = $totalMemory
        totalGB = [math]::Round($totalMemory / 1GB, 2)
        used = $usedMemory
        usedGB = [math]::Round($usedMemory / 1GB, 2)
        free = $freeMemory
        freeGB = [math]::Round($freeMemory / 1GB, 2)
        usedPercent = $usedPercent
        availablePercent = $availablePercent
        topProcesses = $topProcesses
    }
}

$output | ConvertTo-Json -Depth 4 -Compress
