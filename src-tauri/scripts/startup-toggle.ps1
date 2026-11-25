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

function Move-StartupRegistryValue($from, $to, [string]$name) {
    if ($from.GetValueNames() -notcontains $name) {
        throw "Startup entry not found: $name"
    }

    $options = [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
    $value = $from.GetValue($name, $null, $options)
    $kind = $from.GetValueKind($name)
    if ($null -eq $value) {
        throw "Cannot back up this registry value: $name"
    }
    if ($to.GetValueNames() -contains $name) {
        $existing = $to.GetValue($name, $null, $options)
        if ($to.GetValueKind($name) -ne $kind -or
            (ConvertTo-Json -InputObject $existing -Compress) -cne (ConvertTo-Json -InputObject $value -Compress)) {
            throw "A different startup entry with this name already exists: $name"
        }
    } else {
        $to.SetValue($name, $value, $kind)
    }

    # Keep the source until the destination has been saved, including after a retry.
    $to.Flush()
    $from.DeleteValue($name, $true)
}

$mutex = $null
$locked = $false
try {
    $item = $ItemJson | ConvertFrom-Json

    $name = $item.name
    $enabled = $item.enabled
    $command = $item.command
    $source = $item.source

    if ($source -in @("HKCU_Run", "StartupFolder")) {
        if ($enabled -isnot [bool]) {
            throw "Invalid startup state"
        }
        $sid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
        $mutex = [System.Threading.Mutex]::new($false, "Local\AIDI-Startup-$sid")
        try {
            $locked = $mutex.WaitOne(5000)
        } catch [System.Threading.AbandonedMutexException] {
            $locked = $true
        }
        if (!$locked) {
            throw "Another startup operation is still running"
        }
    }

    switch ($source) {
        "HKCU_Run" {
            if ([string]::IsNullOrEmpty($name)) {
                throw "Missing startup entry name"
            }
            $registry = [Microsoft.Win32.Registry]::CurrentUser
            $runKey = $null
            $backupKey = $null
            try {
                $runKey = $registry.CreateSubKey("Software\Microsoft\Windows\CurrentVersion\Run")
                $backupKey = $registry.CreateSubKey("Software\AIDI\DisabledStartup\Run")
                if ($enabled) {
                    Move-StartupRegistryValue $backupKey $runKey $name
                } else {
                    Move-StartupRegistryValue $runKey $backupKey $name
                }
            } finally {
                if ($runKey) { $runKey.Dispose() }
                if ($backupKey) { $backupKey.Dispose() }
                $registry.Dispose()
            }
            $result.success = $true
            $result.message = if ($enabled) { "Restored startup item: $name" } else { "Disabled startup item: $name" }
        }
        "StartupFolder" {
            $startupFolder = [Environment]::GetFolderPath(
                [Environment+SpecialFolder]::Startup, [Environment+SpecialFolderOption]::DoNotVerify)
            $disabledFolder = Join-Path $env:APPDATA "AIDI\DisabledStartup"
            if (!$startupFolder -or !$command) {
                throw "Startup folder or shortcut path not found"
            }
            $activePath = [IO.Path]::GetFullPath($command)
            $fileName = [IO.Path]::GetFileName($activePath)
            if (!$fileName -or $fileName.IndexOfAny([IO.Path]::GetInvalidFileNameChars()) -ge 0 -or
                [IO.Path]::GetDirectoryName($activePath) -ne $startupFolder) {
                throw "Item is outside the current user's startup folder"
            }
            $backupPath = Join-Path $disabledFolder $fileName
            if ($enabled) {
                [void][IO.Directory]::CreateDirectory($startupFolder)
                [IO.File]::Move($backupPath, $activePath)
            } else {
                [void][IO.Directory]::CreateDirectory($disabledFolder)
                [IO.File]::Move($activePath, $backupPath)
            }
            $result.success = $true
            $result.message = if ($enabled) { "Restored startup item: $name" } else { "Disabled startup item: $name" }
        }
        { $_ -in @("AllUsersStartup", "HKLM_Run") } {
            $result.message = "Manage system-wide startup items in Windows settings"
        }
        default {
            $result.success = $false
            $result.message = "Unknown startup source: $source"
        }
    }
} catch {
    $result.success = $false
    $result.message = $_.Exception.Message
} finally {
    if ($locked) { $mutex.ReleaseMutex() }
    if ($mutex) { $mutex.Dispose() }
}

$result | ConvertTo-Json -Compress
