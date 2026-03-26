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

# Get marketing name：从 CoreTypes bundle 的 UTI 解析，无需 system_profiler
marketing_name=""
bundles_dir="/System/Library/CoreServices/CoreTypes.bundle/Contents/Library"
if [ -d "$bundles_dir" ]; then
    _matched_bundle=$(grep -rl "<string>${model}" "$bundles_dir" 2>/dev/null | head -1)
    if [ -n "$_matched_bundle" ]; then
        _uti=$(grep -o 'com\.apple\.[a-z0-9-]*20[0-9][0-9][a-z0-9-]*' "$_matched_bundle" 2>/dev/null | head -1)
        # com.apple.macbookpro-14-2025 -> MacBook Pro
        if echo "$_uti" | grep -qi "macbookpro"; then
            marketing_name="MacBook Pro"
        elif echo "$_uti" | grep -qi "macbookair"; then
            marketing_name="MacBook Air"
        elif echo "$_uti" | grep -qi "macbook"; then
            marketing_name="MacBook"
        elif echo "$_uti" | grep -qi "macpro"; then
            marketing_name="Mac Pro"
        elif echo "$_uti" | grep -qi "macmini"; then
            marketing_name="Mac mini"
        elif echo "$_uti" | grep -qi "imac"; then
            marketing_name="iMac"
        elif echo "$_uti" | grep -qi "macstudio"; then
            marketing_name="Mac Studio"
        fi
    fi
fi
if [ -z "$marketing_name" ]; then
    marketing_name="$model"
fi

# Serial number（用 ioreg 替代 system_profiler）
serial_number=$(ioreg -r -d 1 -c IOPlatformExpertDevice 2>/dev/null | awk -F'"' '/"IOPlatformSerialNumber"/{print $4; exit}')
if [ -z "$serial_number" ]; then
    serial_number="Unknown"
fi

# Manufacture date - 动态从 CoreTypes bundle 中按当前机型查找 UTI
manufacture_date="Unknown"

model=$(sysctl -n hw.model)
bundles_dir="/System/Library/CoreServices/CoreTypes.bundle/Contents/Library"

# 在所有 bundle 中搜索包含当前机型标识符的 plist
uti=""
if [ -d "$bundles_dir" ]; then
    matched=$(grep -rl "<string>${model}" "$bundles_dir" 2>/dev/null | head -1)
    if [ -n "$matched" ]; then
        # 从匹配的 plist 中找对应的 UTI（取包含年份的那行）
        uti=$(grep -A5 "<string>${model}</string>" "$matched" 2>/dev/null | \
            grep -o 'com\.apple\.[a-z0-9-]*20[0-9][0-9][a-z0-9-]*' | head -1)
        # 兜底：直接从文件中找含年份的 UTI
        if [ -z "$uti" ]; then
            uti=$(grep -o 'com\.apple\.[a-z0-9-]*20[0-9][0-9][a-z0-9-]*' "$matched" 2>/dev/null | head -1)
        fi
    fi
fi

if [ -n "$uti" ]; then
    # 从 UTI 中提取年份（如 com.apple.macbookpro-14-2025 -> 2025）
    year=$(echo "$uti" | grep -o '20[0-9][0-9]' | head -1)
    if [ -n "$year" ]; then
        manufacture_date="${year}年"
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

# GPU info（避免使用 system_profiler SPDisplaysDataType，该命令在 macOS Ventura+ 会触发 Apple Music 媒体库权限弹窗）
gpu_name=$(ioreg -r -d 1 -c IOPCIDevice 2>/dev/null | awk -F'"' '/"model"/{print $4; exit}' | tr -d '\0')
if [ -z "$gpu_name" ]; then
    # Apple Silicon：从 sysctl 读取芯片型号作为 GPU 名
    chip=$(sysctl -n machdep.cpu.brand_string 2>/dev/null)
    if echo "$chip" | grep -qi "apple"; then
        gpu_name="$chip GPU"
    else
        gpu_name="Integrated Graphics"
    fi
fi
# 分辨率：从 ioreg 读取内置屏幕信息，完全避免 system_profiler
resolution=$(ioreg -r -d 3 -c AppleBacklightDisplay 2>/dev/null | awk -F'"' '/"DisplayProductName"/{name=$4} END{print name}')
if [ -z "$resolution" ]; then
    resolution=$(ioreg -r -d 3 -c IODisplayConnect 2>/dev/null | grep -o '"[0-9]*" = [0-9]*' | head -1)
fi

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
