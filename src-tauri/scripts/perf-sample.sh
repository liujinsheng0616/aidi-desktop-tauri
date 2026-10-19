#!/bin/bash
# Performance Sample Script for macOS - Real-time CPU/Memory/Disk sampling
# Returns JSON with real-time performance counters

# CPU usage (top -l 1 gives CPU idle percentage, subtract from 100)
cpu_idle=$(top -l 1 -n 0 2>/dev/null | grep "CPU usage" | grep -o '[0-9.]*% idle' | grep -o '[0-9.]*' | head -1)
if [ -z "$cpu_idle" ]; then
    cpu_idle=0
fi
cpu_percent=$(echo "scale=1; 100 - ${cpu_idle}" | bc 2>/dev/null || echo "0")

# Available memory from vm_stat
page_size=$(pagesize 2>/dev/null || echo 6)
vm_stat_output=$(vm_stat 2>/dev/null)
pages_free=$(echo "$vm_stat_output" | grep "Pages free" | awk '{print $3}' | tr -d '.')
pages_inactive=$(echo "$vm_stat_output" | grep "Pages inactive" | awk '{print $3}' | tr -d '.')
pages_speculative=$(echo "$vm_stat_output" | grep "Pages speculative" | awk '{print $3}' | tr -d '.')
avail_mem_mb=$(( (pages_free + pages_inactive + pages_speculative) * page_size / 1048576 ))

# Disk performance - not directly available on macOS via top
disk_percent=0
# Disk queue - not directly available on macOS
disk_queue=0

# Status determination
if (( $(echo "$cpu_percent > 80" | bc -l 2>/dev/null || echo 0) )); then
    status="danger"
elif (( $(echo "$cpu_percent > 60" | bc -l 2>/dev/null || echo 0) )); then
    status="warning"
else
    status="good"
fi

status_label=""
if [ "$status" = "danger" ]; then
    status_label="过载"
elif [ "$status" = "warning" ]; then
    status_label="偏高"
else
    status_label="正常"
fi

cat << EOF
{
  "dimension": "perf",
  "status": "$status",
  "summary": "CPU: ${cpu_percent}%, 可用内存: ${avail_mem_mb} MB ($status_label)",
  "details": {
    "cpuPercent": ${cpu_percent},
    "availableMemoryMB": ${avail_mem_mb},
    "diskPercent": ${disk_percent},
    "diskQueueLength": ${disk_queue}
  }
}
EOF
