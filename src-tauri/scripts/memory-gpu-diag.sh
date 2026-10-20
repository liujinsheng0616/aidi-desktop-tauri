#!/bin/bash
# Memory & GPU Diagnostic Script for macOS
# Gets physical memory stick details and GPU info
# Returns JSON with memory stick and GPU details

# Get memory stick info via system_profiler
mem_profiler=$(system_profiler SPMemoryDataType 2>/dev/null)

# Parse memory sticks
sticks_json=""
stick_count=0
total_memory_gb=$(sysctl -n hw.memsize 2>/dev/null | awk '{printf "%.1f", $1/1073741824}')

# Extract DIMM/RAM info from system_profiler
while IFS= read -r line; do
    if echo "$line" | grep -q "Size:"; then
        size_gb=$(echo "$line" | awk '{print $2}' | awk '{printf "%.1f", $1}')
        [ -z "$size_gb" ] && continue
        speed_mhz=$(echo "$line" | grep -o 'speed.*' | grep -o '[0-9]*' | head -1)
        [ -z "$speed_mhz" ] && speed_mhz=0

        if [ -n "$sticks_json" ]; then
            sticks_json="$sticks_json,"
        fi
        sticks_json="$sticks_json{ \"capacityGB\": $size_gb, \"speedMHz\": $speed_mhz, \"manufacturer\": \"Apple\", \"partNumber\": \"\", \"deviceLocator\": \"Built-in\", \"bankLabel\": \"\", \"serialNumber\": \"\" }"
        stick_count=$((stick_count + 1))
    fi
done <<< "$mem_profiler"

# If no sticks found, report soldered memory
if [ "$stick_count" -eq 0 ]; then
    sticks_json="{ \"capacityGB\": $total_memory_gb, \"speedMHz\": 0, \"manufacturer\": \"Apple\", \"partNumber\": \"\", \"deviceLocator\": \"Soldered\", \"bankLabel\": \"\", \"serialNumber\": \"\" }"
    stick_count=1
fi

# Get GPU info via system_profiler
gpu_profiler=$(system_profiler SPDisplaysDataType 2>/dev/null)
gpus_json=""

while IFS= read -r line; do
    if echo "$line" | grep -q "Chipset Model:"; then
        gpu_name=$(echo "$line" | cut -d: -f2 | xargs)
        [ -z "$gpu_name" ] && gpu_name="Unknown GPU"

        vram_gb=$(echo "$gpu_profiler" | grep -A5 "$gpu_name" | grep "VRAM" | grep -o '[0-9]*' | head -1)
        [ -z "$vram_gb" ] && vram_gb=0
        resolution=$(echo "$gpu_profiler" | grep "Resolution" | head -1 | awk '{print $2 "x" $4}')
        [ -z "$resolution" ] && resolution="Unknown"

        if [ -n "$gpus_json" ]; then
            gpus_json="$gpus_json,"
        fi
        gpus_json="$gpus_json{ \"name\": \"$gpu_name\", \"driverVersion\": \"\", \"driverDate\": \"\", \"adapterRAM_GB\": $vram_gb, \"resolution\": \"$resolution\", \"status\": \"OK\" }"
    fi
done <<< "$gpu_profiler"

[ -z "$gpus_json" ] && gpus_json="{ \"name\": \"Unknown\", \"driverVersion\": \"\", \"driverDate\": \"\", \"adapterRAM_GB\": 0, \"resolution\": \"Unknown\", \"status\": \"OK\" }"

cat << EOF
{
  "dimension": "mem_gpu",
  "status": "info",
  "summary": "内存: ${total_memory_gb}GB (${stick_count}条), 显卡: 见详情",
  "details": {
    "memorySticks": [$sticks_json],
    "totalMemoryGB": ${total_memory_gb},
    "stickCount": ${stick_count},
    "gpus": [$gpus_json]
  }
}
EOF
