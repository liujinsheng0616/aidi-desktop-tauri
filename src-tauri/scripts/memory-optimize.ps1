# Memory Optimize Script - Frees up memory
# Returns JSON with optimization results

# Fix Chinese encoding: Force UTF-8 output
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

# Get memory before
$osBefore = Get-CimInstance Win32_OperatingSystem
$freeMemoryBefore = $osBefore.FreePhysicalMemory * 1KB

# Clear working sets of processes (soft memory release)
$processesOptimized = 0

# Call Windows API to empty working sets
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public class MemoryOptimizer {
    [DllImport("psapi.dll")]
    public static extern bool EmptyWorkingSet(IntPtr hProcess);

    [DllImport("kernel32.dll")]
    public static extern IntPtr GetCurrentProcess();
}
"@

Get-Process | Where-Object { $_.WorkingSet64 -gt 50MB } | ForEach-Object {
    try {
        $handle = $_.Handle
        if ($handle) {
            [MemoryOptimizer]::EmptyWorkingSet($handle) | Out-Null
            $processesOptimized++
        }
    } catch {
        # Skip processes we can't access
    }
}

# Force garbage collection for .NET processes
[System.GC]::Collect()
[System.GC]::WaitForPendingFinalizers()

# Wait a moment for memory to be freed
Start-Sleep -Milliseconds 500

# Get memory after
$osAfter = Get-CimInstance Win32_OperatingSystem
$freeMemoryAfter = $osAfter.FreePhysicalMemory * 1KB

$freed = $freeMemoryAfter - $freeMemoryBefore

$output = @{
    success = $true
    freedBytes = $freed
    freedMB = [math]::Round($freed / 1MB, 2)
    processesOptimized = $processesOptimized
    freeMemoryBefore = $freeMemoryBefore
    freeMemoryAfter = $freeMemoryAfter
    freeMemoryBeforeMB = [math]::Round($freeMemoryBefore / 1MB, 2)
    freeMemoryAfterMB = [math]::Round($freeMemoryAfter / 1MB, 2)
}

$output | ConvertTo-Json -Depth 2 -Compress
