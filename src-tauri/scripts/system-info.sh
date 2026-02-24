#!/bin/bash
# System Info Script for macOS - Gets system hardware and OS information
# Returns JSON with system details

# Get system info
hostname=$(hostname)
model=$(sysctl -n hw.model 2>/dev/null || echo "Mac")

# Get marketing name
marketing_name=$(system_profiler SPHardwareDataType 2>/dev/null | grep "Model Name" | cut -d: -f2 | xargs)
if [ -z "$marketing_name" ]; then
    marketing_name="$model"
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
    "manufacturer": "Apple",
    "model": "$marketing_name",
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
