# System Info Script - Gets system hardware and OS information
# Returns JSON with system details

# Fix Chinese encoding: Force UTF-8 output
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$ErrorActionPreference = "SilentlyContinue"

# OS Info
$os = Get-CimInstance Win32_OperatingSystem
$cs = Get-CimInstance Win32_ComputerSystem
$bios = Get-CimInstance Win32_BIOS

# Network Info - Get local IPv4 address
$localIP = ""
Get-NetIPAddressConfiguration | Where-Object { $_.AddressFamily -eq "IPv4" -and $_.IPAddress -notlike "127.*" } | Select-Object -First 1 | ForEach-Object {
    $localIP = $_.IPAddress
}

# CPU Info
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1

# Memory Info
$totalMemoryGB = [math]::Round($cs.TotalPhysicalMemory / 1GB, 2)

# GPU Info
$gpu = Get-CimInstance Win32_VideoController | Select-Object -First 1

# Disk Info
$totalDiskGB = 0
Get-Volume | Where-Object { $_.DriveLetter -and $_.DriveType -eq "Fixed" } | ForEach-Object {
    $totalDiskGB += $_.Size / 1GB
}
$totalDiskGB = [math]::Round($totalDiskGB, 2)

$output = @{
    dimension = "system"
    status = "info"
    summary = "$($cs.Manufacturer) $($cs.Model)"
    details = @{
        hostname = $cs.Name
        ip = $localIP
        manufacturer = $cs.Manufacturer
        model = $cs.Model
        serialNumber = $bios.SerialNumber
        manufactureDate = if ($bios.ReleaseDate) { "$($bios.ReleaseDate.Year)年$($bios.ReleaseDate.Month)月" } else { "Unknown" }
        os = @{
            name = $os.Caption
            version = $os.Version
            build = $os.BuildNumber
            architecture = $os.OSArchitecture
            installDate = $os.InstallDate.ToString("yyyy-MM-dd")
            lastBoot = $os.LastBootUpTime.ToString("yyyy-MM-dd HH:mm:ss")
        }
        cpu = @{
            name = $cpu.Name
            cores = $cpu.NumberOfCores
            threads = $cpu.NumberOfLogicalProcessors
            maxSpeed = "$([math]::Round($cpu.MaxClockSpeed / 1000, 2)) GHz"
        }
        memory = @{
            totalGB = $totalMemoryGB
        }
        gpu = @{
            name = $gpu.Name
            driverVersion = $gpu.DriverVersion
            resolution = "$($gpu.CurrentHorizontalResolution)x$($gpu.CurrentVerticalResolution)"
        }
        storage = @{
            totalGB = $totalDiskGB
        }
    }
}

$output | ConvertTo-Json -Depth 4 -Compress
