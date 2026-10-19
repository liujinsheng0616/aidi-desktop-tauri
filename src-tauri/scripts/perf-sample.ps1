# Performance Sample Script - Real-time CPU/Memory/Disk sampling
# Returns JSON with real-time performance counters

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

# CPU usage
$cpuSample = Get-Counter '\Processor(_Total)\% Processor Time' -ErrorAction SilentlyContinue
$cpuPercent = if ($cpuSample) { [math]::Round($cpuSample.CounterSamples.CookedValue, 1) } else { 0 }

# Available memory
$memSample = Get-Counter '\Memory\Available MBytes' -ErrorAction SilentlyContinue
$availMemMB = if ($memSample) { [math]::Round($memSample.CounterSamples.CookedValue, 0) } else { 0 }

# Disk busy time
$diskSample = Get-Counter '\PhysicalDisk(_Total)\% Disk Time' -ErrorAction SilentlyContinue
$diskPercent = if ($diskSample) { [math]::Round($diskSample.CounterSamples.CookedValue, 1) } else { 0 }

# Disk queue length
$queueSample = Get-Counter '\PhysicalDisk(_Total)\Current Disk Queue Length' -ErrorAction SilentlyContinue
$diskQueue = if ($queueSample) { [math]::Round($queueSample.CounterSamples.CookedValue, 1) } else { 0 }

# Status determination
if ($cpuPercent -gt 80 -or $diskQueue -gt 3) {
    $status = "danger"
} elseif ($cpuPercent -gt 60 -or $diskPercent -gt 80 -or $diskQueue -gt 2) {
    $status = "warning"
} else {
    $status = "good"
}

$statusLabel = if ($status -eq "danger") { "过载" } elseif ($status -eq "warning") { "偏高" } else { "正常" }

$output = @{
    dimension = "perf"
    status = $status
    summary = "CPU: $cpuPercent%, 磁盘繁忙: $diskPercent%, 队列: $diskQueue, 可用内存: $availMemMB MB ($statusLabel)"
    details = @{
        cpuPercent = $cpuPercent
        availableMemoryMB = $availMemMB
        diskPercent = $diskPercent
        diskQueueLength = $diskQueue
    }
}

$output | ConvertTo-Json -Depth 3 -Compress
