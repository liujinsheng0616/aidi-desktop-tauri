#!/bin/bash
# Disk Clean Script for macOS - Cleans selected categories
# Input: JSON array of category keys

input="$1"

cleaned=0
success_count=0
failed_count=0
details_json="[]"
details_count=0
MAX_DETAILS=100  # Limit details to avoid huge output

# Check if category is selected
contains() {
    echo "$input" | grep -q "\"$1\""
}

# Add detail to JSON array (limited to MAX_DETAILS)
add_detail() {
    local category="$1"
    local path="$2"
    local size="$3"
    local status="$4"
    local reason="$5"

    # Only add if under limit
    if [ $details_count -ge $MAX_DETAILS ]; then
        return
    fi
    details_count=$((details_count + 1))

    # Escape path for JSON (backslash, quote, and control chars)
    escaped_path=$(printf '%s' "$path" | sed 's/\\/\\\\/g; s/"/\\"/g' | tr -d '\n\t\r')

    if [ "$details_json" = "[]" ]; then
        details_json="[{\"category\":\"$category\",\"path\":\"$escaped_path\",\"size\":$size,\"status\":\"$status\",\"reason\":\"$reason\"}]"
    else
        details_json="${details_json%]},{\"category\":\"$category\",\"path\":\"$escaped_path\",\"size\":$size,\"status\":\"$status\",\"reason\":\"$reason\"}]"
    fi
}

# Try to delete and verify, returns actual deleted size
try_delete_file() {
    local item="$1"
    local category="$2"

    local item_size=$(du -sk "$item" 2>/dev/null | cut -f1)
    item_size=$((item_size * 1024))

    rm -f "$item" 2>/dev/null

    # Check if file still exists
    if [ ! -e "$item" ]; then
        cleaned=$((cleaned + item_size))
        success_count=$((success_count + 1))
        add_detail "$category" "$item" "$item_size" "success" ""
    else
        failed_count=$((failed_count + 1))
        add_detail "$category" "$item" "$item_size" "failed" "权限不足或文件被占用"
    fi
}

# Try to delete directory and verify, returns actual deleted size
try_delete_dir() {
    local item="$1"
    local category="$2"

    local size_before=$(du -sk "$item" 2>/dev/null | cut -f1)
    size_before=$((size_before * 1024))

    rm -rf "$item" 2>/dev/null

    # Check if completely deleted
    if [ ! -e "$item" ]; then
        cleaned=$((cleaned + size_before))
        success_count=$((success_count + 1))
        add_detail "$category" "$item" "$size_before" "success" ""
    else
        # Partially deleted, calculate actual deleted size
        local size_after=$(du -sk "$item" 2>/dev/null | cut -f1)
        size_after=$((size_after * 1024))
        local actual_deleted=$((size_before - size_after))

        if [ $actual_deleted -gt 0 ]; then
            cleaned=$((cleaned + actual_deleted))
            success_count=$((success_count + 1))
            add_detail "$category" "$item" "$actual_deleted" "partial" "部分文件被占用"
        fi

        failed_count=$((failed_count + 1))
        add_detail "$category" "$item" "$size_after" "failed" "权限不足或文件被占用"
    fi
}

# Clean temp files
if contains "temp"; then
    # Clean $TMPDIR
    if [ -d "$TMPDIR" ]; then
        for item in "$TMPDIR"/*; do
            [ -e "$item" ] || continue
            if [ -d "$item" ]; then
                try_delete_dir "$item" "temp"
            else
                try_delete_file "$item" "temp"
            fi
        done
    fi

    # Clean /tmp (only user's files)
    if [ -d "/tmp" ]; then
        while IFS= read -r -d '' item; do
            try_delete_file "$item" "temp"
        done < <(find /tmp -type f -user $(whoami) -print0 2>/dev/null)
    fi
fi

# Clean user caches (systemTemp)
if contains "systemTemp"; then
    if [ -d "$HOME/Library/Caches" ]; then
        while IFS= read -r -d '' item; do
            try_delete_file "$item" "systemTemp"
        done < <(find "$HOME/Library/Caches" -type f -mtime +7 -print0 2>/dev/null)
    fi
fi

# Clean logs (prefetch)
if contains "prefetch"; then
    if [ -d "$HOME/Library/Logs" ]; then
        while IFS= read -r -d '' item; do
            try_delete_file "$item" "prefetch"
        done < <(find "$HOME/Library/Logs" -type f -mtime +30 -print0 2>/dev/null)
    fi
fi

# Finder can clear trash across volumes. Verify the result independently of its command status.
if contains "recycleBin"; then
    script_dir=$(dirname "$0")
    . "$script_dir/trash-stats.sh"
    if ! trash_snapshot details; then
        failed_count=$((failed_count + 1))
        add_detail "recycleBin" "废纸篓" 0 "failed" "无法读取废纸篓，请检查 Finder 自动化权限"
    elif [ "$trash_items_count" -gt 0 ]; then
        trash_before_bytes=$trash_bytes
        trash_before_count=$trash_items_count
        trash_before_details=$trash_details
        /usr/bin/osascript -e 'with timeout of 10 seconds
            tell application "Finder" to empty trash
            end timeout' >/dev/null 2>&1
        empty_status=$?

        trash_verified=false
        for attempt in 1 2 3 4 5; do
            if trash_snapshot; then
                trash_verified=true
                [ "$trash_items_count" -eq 0 ] && break
            else
                trash_verified=false
                break
            fi
            [ "$attempt" -lt 5 ] && sleep 1
        done

        if [ "$trash_verified" = true ] && [ "$trash_items_count" -eq 0 ]; then
            cleaned=$((cleaned + trash_before_bytes))
            success_count=$((success_count + trash_before_count))
            while IFS=$'\t' read -r item_size item_name; do
                [ -z "$item_name" ] && continue
                item_size=$(printf '%s\n' "$item_size" | awk '{printf "%.0f", $1}')
                add_detail "recycleBin" "$item_name" "$item_size" "success" ""
            done <<< "$trash_before_details"
        else
            failed_count=$((failed_count + 1))
            if [ "$trash_verified" != true ]; then
                add_detail "recycleBin" "废纸篓" 0 "failed" "无法验证清理结果，请在 Finder 中检查"
            elif [ "$empty_status" -ne 0 ]; then
                add_detail "recycleBin" "废纸篓" "$trash_bytes" "failed" "Finder 清空未完成，请在 Finder 中检查"
            else
                add_detail "recycleBin" "废纸篓" "$trash_bytes" "failed" "仍有项目留在废纸篓，请在 Finder 中检查"
            fi
        fi
    fi
fi

# Clean browser caches
if contains "browserCache"; then
    for cache_dir in \
        "$HOME/Library/Caches/Google/Chrome" \
        "$HOME/Library/Caches/com.apple.Safari" \
        "$HOME/Library/Caches/Firefox" \
        "$HOME/Library/Caches/Microsoft Edge"; do

        if [ -d "$cache_dir" ]; then
            for item in "$cache_dir"/*; do
                [ -e "$item" ] || continue
                if [ -d "$item" ]; then
                    try_delete_dir "$item" "browserCache"
                else
                    try_delete_file "$item" "browserCache"
                fi
            done
        fi
    done
fi

# Calculate MB with proper formatting (ensure leading zero)
if [ $cleaned -eq 0 ]; then
    cleaned_mb=0
else
    cleaned_mb=$(awk "BEGIN {printf \"%.2f\", $cleaned / 1048576}")
fi

cat << EOF
{
  "cleaned": $cleaned,
  "cleanedMB": $cleaned_mb,
  "successCount": $success_count,
  "failedCount": $failed_count,
  "details": $details_json
}
EOF
