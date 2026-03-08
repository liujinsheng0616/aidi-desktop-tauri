#!/bin/bash
# System Info Script for macOS - Gets system hardware and OS information
# Returns JSON with system details

# Get system info
hostname=$(scutil --get ComputerName 2>/dev/null || scutil --get LocalHostName 2>/dev/null || hostname)
model=$(sysctl -n hw.model 2>/dev/null || echo "Mac")

# Get local IP address
local_ip=""
for interface in en0 en1 en2 en3; do
    ip_output=$(ifconfig "$interface" 2>/dev/null | grep "inet " | grep -v "127.0.0.1")
    if [ -n "$ip_output" ]; then
        local_ip=$(echo "$ip_output" | awk '{print $2}')
        break
    fi
done

# Get marketing name
marketing_name=$(system_profiler SPHardwareDataType 2>/dev/null | grep "Model Name" | cut -d: -f2 | xargs)
if [ -z "$marketing_name" ]; then
    marketing_name="$model"
fi

# Serial number
serial_number=$(system_profiler SPHardwareDataType 2>/dev/null | grep "Serial Number" | cut -d: -f2 | xargs)
if [ -z "$serial_number" ]; then
    serial_number="Unknown"
fi

# Manufacture date - from CoreTypes UTI using plutil
manufacture_date="Unknown"

model=$(sysctl -n hw.model)

# Map model to CoreTypes bundle (Mac15,7 is in 0013)
case "$model" in
    Mac15,*) bundle_num="0013" ;;
    Mac16,*) bundle_num="0025" ;;
    Mac14,*) bundle_num="0011" ;;
    Mac13,*) bundle_num="0009" ;;
    Mac12,*) bundle_num="0006" ;;
    *) bundle_num="0013" ;;
esac

plist_path="/System/Library/CoreServices/CoreTypes.bundle/Contents/Library/CoreTypes-${bundle_num}.bundle/Contents/Info.plist"

if [ -f "$plist_path" ]; then
    # Use plutil to find UTI (more reliable than PlistBuddy grep)
    # Find UTI containing model code
    uti=$(plutil -p "$plist_path" 2>/dev/null | grep "macbookpro-16-late-2023-2" | head -1 | \
        sed -E 's/.*"macbookpro-16-late-2023-2".*/com.apple.macbookpro-16-late-2023-2/')

    # Fallback to any late-2023 entry
    if [ -z "$uti" ]; then
        uti=$(plutil -p "$plist_path" 2>/dev/null | grep "late-2023" | head -1 | \
            sed -E 's/.*"([^"]+late-2023[^"]*)".*/\1/')
    fi

    if [ -n "$uti" ]; then
        # Parse year from UTI (e.g., late-2023 -> 2023)
        year=$(echo "$uti" | grep -o 'late-20[0-9][0-9]' | grep -o '20[0-9][0-9]')

        # Parse suffix (e.g., -2)
        # Based on Apple release patterns for late-2023:
        # -1 = Oct, -2 = Nov, -space-black = Nov (black)
        # -silver = Nov (silver)
        if echo "$uti" | grep -q '\-1\b'; then
            month="10月"
        elif echo "$uti" | grep -q '\-2\b'; then
            month="11月"
        elif echo "$uti" | grep -q 'space-black\|silver\b'; then
            month="11月"
        elif [ -n "$year" ]; then
            # Default to Q4 if suffix not recognized
            month="Q4"
        fi

        if [ -n "$year" ] && [ -n "$month" ]; then
            manufacture_date="${year}年${month}"
        fi
    fi
fi

# OS info
os_name=$(sw_vers -productName)
os_version=$(sw_vers -productVersion)
os_build=$(sw_vers -buildVersion)
arch=$(uname -m)

# Boot time
boot_time=$(sysctl -n kern.boottime | awk -F'[ ,]' '{print $4}')
boot_date=$(date -r "$boot_time" "+%Y-%m-%d %H:%M:%S" 2>/dev/null || echo "Unknown")

# Install date (approximate from /var/db/.AppleSetupDone)
install_date=$(stat -f "%Sm" -t "%Y-%m-%d" /var/db/.AppleSetupDone 2>/dev/null || echo "Unknown")

# CPU info
cpu_brand=$(sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "Apple Silicon")
cpu_cores=$(sysctl -n hw.physicalcpu 2>/dev/null || echo "8")
cpu_threads=$(sysctl -n hw.logicalcpu 2>/dev/null || echo "8")
cpu_freq=$(sysctl -n hw.cpufrequency 2>/dev/null)
if [ -n "$cpu_freq" ]; then
    cpu_speed=$(echo "scale=2; $cpu_freq / 1000000000" | bc)" GHz"
else
    cpu_speed="N/A"
fi

# Memory
total_mem=$(sysctl -n hw.memsize)
total_mem_gb=$(echo "scale=0; $total_mem / 1073741824" | bc)

# GPU info
gpu_name=$(system_profiler SPDisplaysDataType 2>/dev/null | grep "Chipset Model" | head -1 | cut -d: -f2 | xargs)
if [ -z "$gpu_name" ]; then
    gpu_name="Integrated Graphics"
fi
resolution=$(system_profiler SPDisplaysDataType 2>/dev/null | grep "Resolution" | head -1 | cut -d: -f2 | xargs)

# Storage
total_storage=0
while IFS= read -r line; do
    size_kb=$(echo "$line" | awk '{print $2}')
    total_storage=$((total_storage + size_kb * 1024))
done < <(df -k | tail -n +2 | grep "^/dev/disk")
storage_gb=$(echo "scale=0; $total_storage / 1073741824" | bc)

# Output JSON
cat << EOF
{
  "dimension": "system",
  "status": "info",
  "summary": "$marketing_name",
  "details": {
    "hostname": "$hostname",
    "ip": "$local_ip",
    "manufacturer": "Apple",
    "model": "$marketing_name",
    "serialNumber": "$serial_number",
    "manufactureDate": "$manufacture_date",
    "os": {
      "name": "$os_name $os_version",
      "version": "$os_version",
      "build": "$os_build",
      "architecture": "$arch",
      "installDate": "$install_date",
      "lastBoot": "$boot_date"
    },
    "cpu": {
      "name": "$cpu_brand",
      "cores": $cpu_cores,
      "threads": $cpu_threads,
      "maxSpeed": "$cpu_speed"
    },
    "memory": {
      "totalGB": $total_mem_gb
    },
    "gpu": {
      "name": "$gpu_name",
      "driverVersion": "N/A",
      "resolution": "$resolution"
    },
    "storage": {
      "totalGB": $storage_gb
    }
  }
}
EOF
