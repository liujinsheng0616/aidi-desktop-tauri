# Startup Toggle Script - Enable/disable a startup item
# Input: JSON with name, location, and enabled status

# Fix Chinese encoding: Force UTF-8 output
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$ItemJson = $env:SCRIPT_ARGS

$ErrorActionPreference = "Stop"

$result = @{
    success = $false
    message = ""
}

try {
    $item = $ItemJson | ConvertFrom-Json

    $name = $item.name
    $location = $item.location
    $enabled = $item.enabled
    $command = $item.command
    $source = $item.source

    switch ($source) {
        "HKCU_Run" {
            $regPath = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
            if ($enabled) {
                # Re-enable: restore the registry value
                if ($command) {
                    Set-ItemProperty -Path $regPath -Name $name -Value $command
                    $result.success = $true
                    $result.message = "Enabled startup item: $name"
                }
            } else {
                # Disable: remove from registry (backup value is in $command)
                Remove-ItemProperty -Path $regPath -Name $name -ErrorAction SilentlyContinue
                $result.success = $true
                $result.message = "Disabled startup item: $name"
            }
        }
        "HKLM_Run" {
            $regPath = "HKLM:\Software\Microsoft\Windows\CurrentVersion\Run"
            if ($enabled) {
                if ($command) {
                    Set-ItemProperty -Path $regPath -Name $name -Value $command
                    $result.success = $true
                    $result.message = "Enabled startup item: $name"
                }
            } else {
                Remove-ItemProperty -Path $regPath -Name $name -ErrorAction SilentlyContinue
                $result.success = $true
                $result.message = "Disabled startup item: $name"
            }
        }
        "StartupFolder" {
            $path = $command
            if ($enabled) {
                # Can't easily re-enable a deleted shortcut
                $result.success = $false
                $result.message = "Cannot re-enable startup folder items"
            } else {
                if (Test-Path $path) {
                    # Move to a disabled folder instead of deleting
                    $disabledFolder = "$env:APPDATA\AIDI\DisabledStartup"
                    if (!(Test-Path $disabledFolder)) {
                        New-Item -ItemType Directory -Path $disabledFolder -Force | Out-Null
                    }
                    Move-Item -Path $path -Destination $disabledFolder -Force
                    $result.success = $true
                    $result.message = "Disabled startup item: $name"
                }
            }
        }
        "AllUsersStartup" {
            $result.success = $false
            $result.message = "Modifying All Users startup requires administrator privileges"
        }
        default {
            $result.success = $false
            $result.message = "Unknown startup source: $source"
        }
    }
} catch {
    $result.success = $false
    $result.message = $_.Exception.Message
}

$result | ConvertTo-Json -Compress
