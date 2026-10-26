#!/bin/bash
# Memory Optimize Script for macOS - Frees up memory
# Returns JSON with optimization results

# Get memory before
page_size=$(pagesize)
vm_before=$(vm_stat | grep "Pages free" | awk '{print $3}' | tr -d '.')
free_before=$((vm_before * page_size))

# Get top 5 processes by memory before optimization
top_procs_before=$(ps -eo pid,rss,comm -r | head -6 | tail -5)

sync

# Try purge: first without sudo, then with AppleScript admin privileges
purge_ok=false
error_reason=""
if command -v purge &> /dev/null; then
    purge 2>/dev/null
    if [ $? -eq 0 ]; then
        purge_ok=true
    else
        # Try with admin privileges via AppleScript (will prompt for password)
        # Capture exit code: -128 = user cancelled the password dialog
        osascript -e 'do shell script "purge" with administrator privileges' >/dev/null 2>&1
        osa_status=$?
        if [ $osa_status -eq 0 ]; then
            purge_ok=true
        elif [ $osa_status -eq 128 ] || [ $osa_status -eq -128 ]; then
            # -128 in shell becomes 128; user clicked "Cancel" on the password dialog
            error_reason="user_cancelled"
        else
            # Wrong password or other auth failure
            error_reason="permission_denied"
        fi
    fi
else
    error_reason="purge_not_found"
fi

# Wait for memory to be freed
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

# Build per-process details from top 5 processes
details=""
proc_count=0
while IFS= read -r line_before; do
    pid=$(echo "$line_before" | awk '{print $1}')
    rss_before=$(echo "$line_before" | awk '{print $2}')
    name=$(echo "$line_before" | awk '{for(i=3;i<=NF;i++) printf $i" "; print ""}' | sed 's/ $//')

    [ -z "$pid" ] && continue

    # Check same process after
    rss_after=$(ps -p "$pid" -o rss= 2>/dev/null | tr -d ' ')
    if [ -z "$rss_after" ]; then
        rss_after=0
    fi

    # RSS is in KB
    proc_freed=$((rss_before - rss_after))
    if [ $proc_freed -lt 0 ]; then
        proc_freed=0
    fi

    proc_freed_mb=$(echo "scale=2; $proc_freed / 1024" | bc)
    rss_before_mb=$(echo "scale=2; $rss_before / 1024" | bc)
    rss_after_mb=$(echo "scale=2; $rss_after / 1024" | bc)

    # Escape name for JSON
    escaped_name=$(printf '%s' "$name" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n\t\r')

    if [ -n "$details" ]; then
        details="$details,"
    fi
    details="$details{ \"name\": \"$escaped_name\", \"pid\": $pid, \"freedMB\": $proc_freed_mb, \"beforeMB\": $rss_before_mb, \"afterMB\": $rss_after_mb }"
    proc_count=$((proc_count + 1))
done <<< "$top_procs_before"

# Build errorReason JSON value: null when empty, quoted string otherwise
if [ -n "$error_reason" ]; then
    error_reason_json="\"$error_reason\""
else
    error_reason_json="null"
fi

cat << EOF
{
  "success": $purge_ok,
  "freedBytes": $freed,
  "freedMB": $freed_mb,
  "processesOptimized": $proc_count,
  "freeMemoryBefore": $free_before,
  "freeMemoryAfter": $free_after,
  "freeMemoryBeforeMB": $free_before_mb,
  "freeMemoryAfterMB": $free_after_mb,
  "errorReason": $error_reason_json,
  "details": [$details]
}
EOF
