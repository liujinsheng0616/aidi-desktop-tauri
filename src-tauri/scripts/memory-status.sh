#!/bin/bash
# Memory Status Script for macOS - Gets memory usage and top processes
# Returns JSON with memory info and process list

# Get memory info using vm_stat
page_size=$(pagesize)
vm_stat_output=$(vm_stat)

pages_free=$(echo "$vm_stat_output" | grep "Pages free" | awk '{print $3}' | tr -d '.')
pages_active=$(echo "$vm_stat_output" | grep "Pages active" | awk '{print $3}' | tr -d '.')
pages_inactive=$(echo "$vm_stat_output" | grep "Pages inactive" | awk '{print $3}' | tr -d '.')
pages_speculative=$(echo "$vm_stat_output" | grep "Pages speculative" | awk '{print $3}' | tr -d '.')
pages_wired=$(echo "$vm_stat_output" | grep "Pages wired down" | awk '{print $4}' | tr -d '.')
pages_compressed=$(echo "$vm_stat_output" | grep "Pages occupied by compressor" | awk '{print $5}' | tr -d '.')

# Calculate memory
free_mem=$((pages_free * page_size))
active_mem=$((pages_active * page_size))
inactive_mem=$((pages_inactive * page_size))
speculative_mem=$((pages_speculative * page_size))
wired_mem=$((pages_wired * page_size))
compressed_mem=$((pages_compressed * page_size))

# Total memory from sysctl
total_mem=$(sysctl -n hw.memsize)

# Used = total - free - inactive - speculative
used_mem=$((total_mem - free_mem - inactive_mem - speculative_mem))
available_mem=$((free_mem + inactive_mem))

total_gb=$(echo "scale=2; $total_mem / 1073741824" | bc)
used_gb=$(echo "scale=2; $used_mem / 1073741824" | bc)
free_gb=$(echo "scale=2; $available_mem / 1073741824" | bc)
used_percent=$(echo "scale=1; $used_mem * 100 / $total_mem" | bc)
available_percent=$(echo "scale=1; 100 - $used_percent" | bc)

# Get top 5 memory-consuming processes
top_processes=$(ps aux --sort=-%mem 2>/dev/null | head -6 | tail -5 | awk '{
    mem_kb = $6;
    mem_mb = mem_kb / 1024;
    printf "{ \"name\": \"%s\", \"pid\": %s, \"memory\": %d, \"memoryMB\": %.1f }", $11, $2, mem_kb * 1024, mem_mb
}' | paste -sd ',' -)

# Fallback for macOS ps format
if [ -z "$top_processes" ]; then
    top_processes=$(ps -arcwwwxo "pid,rss,comm" | head -6 | tail -5 | awk '{
        mem_kb = $2;
        mem_mb = mem_kb / 1024;
        gsub(/.*\//, "", $3);
        printf "{ \"name\": \"%s\", \"pid\": %s, \"memory\": %d, \"memoryMB\": %.1f }", $3, $1, mem_kb * 1024, mem_mb
    }' | paste -sd ',' -)
fi

# Determine status
if (( $(echo "$available_percent > 30" | bc -l) )); then
    status="good"
elif (( $(echo "$available_percent > 15" | bc -l) )); then
    status="warning"
else
    status="danger"
fi

# Output JSON
cat << EOF
{
  "dimension": "memory",
  "status": "$status",
  "summary": "内存使用: ${used_percent}%, 可用: ${available_percent}%",
  "details": {
    "total": $total_mem,
    "totalGB": $total_gb,
    "used": $used_mem,
    "usedGB": $used_gb,
    "free": $available_mem,
    "freeGB": $free_gb,
    "usedPercent": $used_percent,
    "availablePercent": $available_percent,
    "topProcesses": [$top_processes]
  }
}
EOF
