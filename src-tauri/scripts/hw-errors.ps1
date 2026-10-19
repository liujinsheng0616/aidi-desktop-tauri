# Hardware Error Events Script
# Queries Windows Event Log for WHEA hardware errors and Kernel-Power events
# Returns JSON with recent hardware error events

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

# Get WHEA hardware errors
$wheaEvents = Get-WinEvent -FilterHashtable @{
    LogName = 'System'
    ProviderName = 'Microsoft-Windows-WHEA-Logger'
} -MaxEvents 20 -ErrorAction SilentlyContinue | ForEach-Object {
    @{
        timeCreated = $_.TimeCreated.ToString("yyyy-MM-dd HH:mm:ss")
        id = $_.Id
        level = $_.LevelDisplayName
        message = ($_.Message -replace "`r`n", " " -replace "`r", " " -replace "`n", " ").Trim().Substring(0, [math]::Min(200, ($_.Message).Length))
    }
}

# Get Kernel-Power events (ID 41 = unexpected shutdown)
$powerEvents = Get-WinEvent -FilterHashtable @{
    LogName = 'System'
    ProviderName = 'Microsoft-Windows-Kernel-Power'
    Id = 41
} -MaxEvents 10 -ErrorAction SilentlyContinue | ForEach-Object {
    @{
        timeCreated = $_.TimeCreated.ToString("yyyy-MM-dd HH:mm:ss")
        id = $_.Id
        level = $_.LevelDisplayName
        message = "异常关机 (Event ID 41)"
    }
}

$allErrors = @()
if ($wheaEvents) { $allErrors += $wheaEvents }
if ($powerEvents) { $allErrors += $powerEvents }

# Sort by time descending
$allErrors = $allErrors | Sort-Object { [DateTime]::Parse($_.timeCreated) } -Descending | Select-Object -First 20

# Count errors in last 7 days
$sevenDaysAgo = (Get-Date).AddDays(-7)
$recentCount = ($allErrors | Where-Object { [DateTime]::Parse($_.timeCreated) -gt $sevenDaysAgo }).Count

# Status based on recent error count
if ($recentCount -ge 3) {
    $status = "danger"
} elseif ($recentCount -ge 1) {
    $status = "warning"
} else {
    $status = "good"
}

$statusLabel = if ($status -eq "danger") { "硬件故障风险" } elseif ($status -eq "warning") { "有少量错误" } else { "无硬件错误" }

$output = @{
    dimension = "hw_errors"
    status = $status
    summary = "近7天 $recentCount 条硬件错误，总计 $($allErrors.Count) 条 ($statusLabel)"
    details = @{
        recentCount7d = $recentCount
        totalErrors = $allErrors.Count
        errors = $allErrors
    }
}

$output | ConvertTo-Json -Depth 4 -Compress
