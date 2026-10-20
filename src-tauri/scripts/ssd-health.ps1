# SSD Health Script - SSD temperature, wear, power-on hours
# Returns JSON with SSD reliability counters

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

$disks = Get-PhysicalDisk -ErrorAction SilentlyContinue | ForEach-Object {
    $reliability = $_ | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue

    $wearPercent = if ($reliability -and $null -ne $reliability.Wear -and $reliability.Wear -ge 0) { $reliability.Wear } else { 0 }
    $powerOnHours = if ($reliability -and $reliability.PowerOnHours) { $reliability.PowerOnHours } else { 0 }
    $temperature = if ($reliability -and $reliability.Temperature) { $reliability.Temperature } else { 0 }
    $readErrors = if ($reliability -and $reliability.ReadErrorsTotal) { $reliability.ReadErrorsTotal } else { 0 }
    $writeErrors = if ($reliability -and $reliability.WriteErrorsTotal) { $reliability.WriteErrorsTotal } else { 0 }

    @{
        name = $_.FriendlyName
        mediaType = $_.MediaType
        sizeGB = [math]::Round($_.Size / 1GB, 2)
        healthStatus = $_.HealthStatus
        wearPercent = $wearPercent
        powerOnHours = $powerOnHours
        temperatureC = $temperature
        readErrors = $readErrors
        writeErrors = $writeErrors
    }
}

# Determine status based on worst disk
$worstStatus = "good"
foreach ($d in $disks) {
    if ($d.wearPercent -gt 80 -or $d.powerOnHours -gt 30000) { $worstStatus = "danger"; break }
    if ($d.wearPercent -gt 50 -or $d.powerOnHours -gt 20000) { $worstStatus = "warning" }
}

$statusLabel = if ($worstStatus -eq "danger") { "需更换" } elseif ($worstStatus -eq "warning") { "磨损较高" } else { "良好" }

$output = @{
    dimension = "ssd"
    status = $worstStatus
    summary = "$($disks.Count) 块磁盘，SSD 状态: $statusLabel"
    details = @{
        disks = $disks
    }
}

$output | ConvertTo-Json -Depth 4 -Compress
