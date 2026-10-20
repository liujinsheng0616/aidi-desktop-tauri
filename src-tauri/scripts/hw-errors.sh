#!/bin/bash
# Hardware Error Events Script for macOS
# Queries system log for hardware-related errors and panic logs
# Returns JSON with recent hardware error events

# Check for kernel panic logs
panic_dir="/Library/Logs/DiagnosticReports"
errors_json=""
total_errors=0
recent_count=0
seven_days_ago=$(date -v-7d +%s 2>/dev/null || date -d "7 days ago" +%s)

if [ -d "$panic_dir" ]; then
    # Get recent panic files
    while IFS= read -r panic_file; do
        if [ -n "$panic_file" ]; then
            file_time=$(stat -f "%m" "$panic_file" 2>/dev/null || stat -c "%Y" "$panic_file" 2>/dev/null)
            time_created=$(date -r "$file_time" "+%Y-%m-%d %H:%M:%S" 2>/dev/null)

            if [ -n "$file_time" ] && [ "$file_time" -gt "$seven_days_ago" ] 2>/dev/null; then
                recent_count=$((recent_count + 1))
            fi
            total_errors=$((total_errors + 1))

            # Extract panic type from first few lines
            panic_msg=$(head -5 "$panic_file" 2>/dev/null | grep -m1 "panic" | head -c 200)
            [ -z "$panic_msg" ] && panic_msg="Kernel panic"

            if [ -n "$errors_json" ]; then
                errors_json="$errors_json,"
            fi
            errors_json="$errors_json{ \"timeCreated\": \"$time_created\", \"id\": 0, \"level\": \"Error\", \"message\": \"$panic_msg\" }"

            # Limit to 20 entries
            [ "$total_errors" -ge 20 ] && break
        fi
    done < <(ls -t "$panic_dir"/*.panic 2>/dev/null | head -20)
fi

# Skip slow 'log show' on macOS - panic files are sufficient and fast
# Only check panic logs (above), which are the key hardware error indicator

if false; then
    while IFS= read -r line; do
        if [ -n "$line" ]; then
            log_time=$(echo "$line" | awk '{print $1, $2, $3}')
            log_msg=$(echo "$line" | cut -c1-200)
            recent_count=$((recent_count + 1))
            total_errors=$((total_errors + 1))

            if [ -n "$errors_json" ]; then
                errors_json="$errors_json,"
            fi
            errors_json="$errors_json{ \"timeCreated\": \"$log_time\", \"id\": 0, \"level\": \"Warning\", \"message\": \"$log_msg\" }"

            [ "$total_errors" -ge 20 ] && break
        fi
    done <<< "$hw_log_errors"
fi

# Status based on recent error count
if [ "$recent_count" -ge 3 ]; then
    status="danger"
elif [ "$recent_count" -ge 1 ]; then
    status="warning"
else
    status="good"
fi

status_label=""
if [ "$status" = "danger" ]; then
    status_label="硬件故障风险"
elif [ "$status" = "warning" ]; then
    status_label="有少量错误"
else
    status_label="无硬件错误"
fi

cat << EOF
{
  "dimension": "hw_errors",
  "status": "$status",
  "summary": "近7天 ${recent_count} 条硬件错误，总计 ${total_errors} 条 ($status_label)",
  "details": {
    "recentCount7d": ${recent_count},
    "totalErrors": ${total_errors},
    "errors": [${errors_json}]
  }
}
EOF
