# Disk Health Script - Gets disk health and usage info
# Returns JSON with disk information

# Fix Chinese encoding: Force UTF-8 output
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

$volumes = Get-Volume | Where-Object { $_.DriveLetter -and $_.DriveType -eq "Fixed" } | ForEach-Object {
    $usedPercent = if ($_.Size -gt 0) { [math]::Round((($_.Size - $_.SizeRemaining) / $_.Size) * 100, 1) } else { 0 }

    @{
        drive = $_.DriveLetter + ":"
        label = $_.FileSystemLabel
        size = $_.Size
        sizeGB = [math]::Round($_.Size / 1GB, 2)
        free = $_.SizeRemaining
        freeGB = [math]::Round($_.SizeRemaining / 1GB, 2)
        used = $_.Size - $_.SizeRemaining
        usedGB = [math]::Round(($_.Size - $_.SizeRemaining) / 1GB, 2)
        usedPercent = $usedPercent
        fileSystem = $_.FileSystem
        status = if ($usedPercent -gt 90) { "danger" } elseif ($usedPercent -gt 75) { "warning" } else { "good" }
    }
}

$physicalDisks = Get-PhysicalDisk -ErrorAction SilentlyContinue | ForEach-Object {
    @{
        name = $_.FriendlyName
        mediaType = $_.MediaType
        size = $_.Size
        sizeGB = [math]::Round($_.Size / 1GB, 2)
        healthStatus = $_.HealthStatus
        operationalStatus = $_.OperationalStatus
    }
}

# Overall status based on disk usage
$worstStatus = "good"
foreach ($vol in $volumes) {
    if ($vol.status -eq "danger") { $worstStatus = "danger"; break }
    if ($vol.status -eq "warning") { $worstStatus = "warning" }
}

$output = @{
    dimension = "health"
    status = $worstStatus
    summary = "$($volumes.Count) drives, Worst: $worstStatus"
    details = @{
        volumes = $volumes
        physicalDisks = $physicalDisks
    }
}

$output | ConvertTo-Json -Depth 4 -Compress
