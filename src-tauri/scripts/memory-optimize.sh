#!/bin/bash
# Memory Optimize Script for macOS - Frees up memory
# Returns JSON with optimization results

# Get memory before
page_size=$(pagesize)
vm_before=$(vm_stat | grep "Pages free" | awk '{print $3}' | tr -d '.')
free_before=$((vm_before * page_size))

# Purge inactive memory (requires sudo, may not work without it)
# Using a safer approach - just trigger garbage collection
sync

# Try to free memory using purge command if available
if command -v purge &> /dev/null; then
    purge 2>/dev/null
fi

# Wait a moment
sleep 1

# Get memory after
vm_after=$(vm_stat | grep "Pages free" | awk '{print $3}' | tr -d '.')
free_after=$((vm_after * page_size))

freed=$((free_after - free_before))
if [ $freed -lt 0 ]; then
    freed=0
fi

freed_mb=$(echo "scale=2; $freed / 1048576" | bc)
free_before_mb=$(echo "scale=2; $free_before / 1048576" | bc)
free_after_mb=$(echo "scale=2; $free_after / 1048576" | bc)

cat << EOF
{
  "success": true,
  "freedBytes": $freed,
  "freedMB": $freed_mb,
  "processesOptimized": 0,
  "freeMemoryBefore": $free_before,
  "freeMemoryAfter": $free_after,
  "freeMemoryBeforeMB": $free_before_mb,
  "freeMemoryAfterMB": $free_after_mb
}
EOF
