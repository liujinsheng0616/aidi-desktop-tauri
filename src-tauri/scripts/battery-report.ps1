# Battery Health Report Script
# Uses powercfg to get battery design vs full charge capacity and cycle count
# Returns JSON with battery health info

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

# Generate battery report as XML to temp file
$tempFile = [System.IO.Path]::GetTempFileName()
powercfg /batteryreport /xml /output "$tempFile" 2>$null | Out-Null

if (-not (Test-Path $tempFile) -or (Get-Item $tempFile).Length -eq 0) {
    # No battery
    $output = @{
        dimension = "battery"
        status = "info"
        summary = "无电池或未检测到电池"
        details = @{
            hasBattery = $false
        }
    }
    $output | ConvertTo-Json -Depth 3 -Compress
    Remove-Item $tempFile -ErrorAction SilentlyContinue
    exit
}

# Parse XML
[xml]$batteryXml = Get-Content $tempFile -Raw
Remove-Item $tempFile -ErrorAction SilentlyContinue

$batteries = $batteryXml.BatteryReport.Batteries.Battery

$batteryInfo = @()

if ($batteries) {
    foreach ($bat in $batteries) {
        $designCapacity = [int]$bat.DesignCapacity
        $fullCapacity = [int]$bat.FullChargeCapacity
        $cycleCount = [int]$bat.CycleCount

        $healthPercent = if ($designCapacity -gt 0) { [math]::Round(($fullCapacity / $designCapacity) * 100, 1) } else { 0 }

        $batteryInfo += @{
            name = $bat.Id
            designCapacity = $designCapacity
            fullChargeCapacity = $fullCapacity
            healthPercent = $healthPercent
            cycleCount = $cycleCount
        }
    }
}

# Status based on worst battery
$worstStatus = "good"
foreach ($b in $batteryInfo) {
    if ($b.healthPercent -lt 60) { $worstStatus = "danger"; break }
    if ($b.healthPercent -lt 80) { $worstStatus = "warning" }
}

$statusLabel = if ($worstStatus -eq "danger") { "需更换" } elseif ($worstStatus -eq "warning") { "健康度下降" } else { "良好" }

$summary = if ($batteryInfo.Count -eq 0) { "无电池" } else { "电池健康度: $($batteryInfo[0].healthPercent)% ($statusLabel)" }

$output = @{
    dimension = "battery"
    status = $worstStatus
    summary = $summary
    details = @{
        hasBattery = ($batteryInfo.Count -gt 0)
        batteries = $batteryInfo
    }
}

$output | ConvertTo-Json -Depth 4 -Compress
