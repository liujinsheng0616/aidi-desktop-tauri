# Memory & GPU Diagnostic Script
# Gets physical memory stick details and GPU info
# Returns JSON with memory stick and GPU details

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

# Physical memory sticks
$memorySticks = Get-CimInstance Win32_PhysicalMemory -ErrorAction SilentlyContinue | ForEach-Object {
    @{
        capacityGB = [math]::Round($_.Capacity / 1GB, 1)
        speedMHz = $_.Speed
        manufacturer = $_.Manufacturer
        partNumber = if ($_.PartNumber) { $_.PartNumber.Trim() } else { "" }
        deviceLocator = $_.DeviceLocator
        bankLabel = $_.BankLabel
        serialNumber = $_.SerialNumber
    }
}

# GPU info
$gpus = Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | ForEach-Object {
    @{
        name = $_.Name
        driverVersion = $_.DriverVersion
        driverDate = if ($_.DriverDate) { $_.DriverDate.ToString("yyyy-MM-dd") } else { "" }
        adapterRAM_GB = if ($_.AdapterRAM) { [math]::Round($_.AdapterRAM / 1GB, 1) } else { 0 }
        resolution = "$($_.CurrentHorizontalResolution)x$($_.CurrentVerticalResolution)"
        status = $_.Status
    }
}

$totalMemoryGB = if ($memorySticks) { [math]::Round(($memorySticks | Measure-Object -Property capacityGB -Sum).Sum, 1) } else { 0 }
$stickCount = if ($memorySticks) { $memorySticks.Count } else { 0 }

$gpuNames = if ($gpus) { ($gpus | ForEach-Object { $_.name }) -join ", " } else { "未知" }

$output = @{
    dimension = "mem_gpu"
    status = "info"
    summary = "内存: ${totalMemoryGB}GB (${stickCount}条), 显卡: $gpuNames"
    details = @{
        memorySticks = $memorySticks
        totalMemoryGB = $totalMemoryGB
        stickCount = $stickCount
        gpus = $gpus
    }
}

$output | ConvertTo-Json -Depth 4 -Compress
