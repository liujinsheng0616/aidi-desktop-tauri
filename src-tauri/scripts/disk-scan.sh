#!/bin/bash
# Disk Scan Script for macOS - Scans for junk files
# Returns JSON with file list and total size

temp_size=0
temp_count=0
cache_size=0
cache_count=0
logs_size=0
logs_count=0
trash_size=0
trash_count=0
browser_size=0
browser_count=0

# Function to get directory size and count
scan_dir() {
    local dir="$1"
    local category="$2"
    if [ -d "$dir" ]; then
        local size=$(du -sk "$dir" 2>/dev/null | cut -f1)
        local count=$(find "$dir" -type f 2>/dev/null | wc -l | tr -d ' ')
        size=$((size * 1024))
        case $category in
            temp) temp_size=$((temp_size + size)); temp_count=$((temp_count + count)) ;;
            cache) cache_size=$((cache_size + size)); cache_count=$((cache_count + count)) ;;
            logs) logs_size=$((logs_size + size)); logs_count=$((logs_count + count)) ;;
            trash) trash_size=$((trash_size + size)); trash_count=$((trash_count + count)) ;;
            browser) browser_size=$((browser_size + size)); browser_count=$((browser_count + count)) ;;
        esac
    fi
}

# User temp files
scan_dir "$TMPDIR" "temp"
scan_dir "/tmp" "temp"

# User cache
scan_dir "$HOME/Library/Caches" "cache"

# System logs
scan_dir "$HOME/Library/Logs" "logs"
scan_dir "/var/log" "logs"

# Trash - check permission first
trash_needs_auth=false
if [ -d "$HOME/.Trash" ]; then
    # Try to access Trash, if permission denied, mark as needs auth
    if ls "$HOME/.Trash" >/dev/null 2>&1; then
        scan_dir "$HOME/.Trash" "trash"
    else
        trash_needs_auth=true
    fi
fi

# Browser caches ONLY (not history, passwords, cookies, bookmarks)
# These are safe cache directories that browsers will recreate
scan_dir "$HOME/Library/Caches/Google/Chrome" "browser"
scan_dir "$HOME/Library/Caches/com.apple.Safari" "browser"
scan_dir "$HOME/Library/Caches/Firefox" "browser"
scan_dir "$HOME/Library/Caches/Microsoft Edge" "browser"
# Note: History, passwords, bookmarks are in ~/Library/Application Support/, not Caches

# Calculate total
total_size=$((temp_size + cache_size + logs_size + trash_size + browser_size))
size_gb=$(echo "scale=2; $total_size / 1073741824" | bc)

# Determine status
if (( $(echo "$size_gb < 0.5" | bc -l) )); then
    status="good"
elif (( $(echo "$size_gb < 2" | bc -l) )); then
    status="warning"
else
    status="danger"
fi

# Output JSON
cat << EOF
{
  "dimension": "disk",
  "status": "$status",
  "summary": "${size_gb} GB 垃圾文件",
  "details": {
    "totalSize": $total_size,
    "files": [],
    "categories": {
      "temp": { "size": $temp_size, "count": $temp_count },
      "systemTemp": { "size": $cache_size, "count": $cache_count },
      "prefetch": { "size": $logs_size, "count": $logs_count },
      "recycleBin": { "size": $trash_size, "count": $trash_count, "needsAuth": $trash_needs_auth },
      "browserCache": { "size": $browser_size, "count": $browser_count }
    }
  }
}
EOF
