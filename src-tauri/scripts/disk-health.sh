#!/bin/bash
# Disk Health Script for macOS - Gets disk health and usage info
# Returns JSON with disk information

volumes=""
worst_status="good"

# Get disk info using df
while IFS= read -r line; do
    filesystem=$(echo "$line" | awk '{print $1}')
    size_kb=$(echo "$line" | awk '{print $2}')
    used_kb=$(echo "$line" | awk '{print $3}')
    avail_kb=$(echo "$line" | awk '{print $4}')
    capacity=$(echo "$line" | awk '{print $5}' | tr -d '%')
    mount=$(echo "$line" | awk '{print $9}')

    # Skip if not a real disk
    if [[ ! "$filesystem" == /dev/* ]]; then
        continue
    fi

    # Calculate sizes
    size_bytes=$((size_kb * 1024))
    used_bytes=$((used_kb * 1024))
    free_bytes=$((avail_kb * 1024))
    size_gb=$(echo "scale=2; $size_bytes / 1073741824" | bc | awk '{printf "%.2f", $0}')
    used_gb=$(echo "scale=2; $used_bytes / 1073741824" | bc | awk '{printf "%.2f", $0}')
    free_gb=$(echo "scale=2; $free_bytes / 1073741824" | bc | awk '{printf "%.2f", $0}')

    # Determine status
    if [ "$capacity" -gt 90 ]; then
        vol_status="danger"
        worst_status="danger"
    elif [ "$capacity" -gt 75 ]; then
        vol_status="warning"
        if [ "$worst_status" != "danger" ]; then
            worst_status="warning"
        fi
    else
        vol_status="good"
    fi

    # Get volume name
    vol_name=$(diskutil info "$mount" 2>/dev/null | grep "Volume Name" | cut -d: -f2 | xargs)
    if [ -z "$vol_name" ]; then
        vol_name="$mount"
    fi

    if [ -n "$volumes" ]; then
        volumes="$volumes,"
    fi
    volumes="$volumes{ \"drive\": \"$mount\", \"label\": \"$vol_name\", \"size\": $size_bytes, \"sizeGB\": $size_gb, \"free\": $free_bytes, \"freeGB\": $free_gb, \"used\": $used_bytes, \"usedGB\": $used_gb, \"usedPercent\": $capacity, \"fileSystem\": \"APFS\", \"status\": \"$vol_status\" }"

done < <(df -k | tail -n +2)

# Get physical disk info
physical_disks=""
disk_info=$(diskutil list physical 2>/dev/null | grep "^/dev/disk" | head -3)
while IFS= read -r disk; do
    if [ -n "$disk" ]; then
        disk_path=$(echo "$disk" | awk '{print $1}')
        disk_details=$(diskutil info "$disk_path" 2>/dev/null)
        disk_name=$(echo "$disk_details" | grep "Device / Media Name" | cut -d: -f2 | xargs)
        disk_size=$(echo "$disk_details" | grep "Disk Size" | cut -d: -f2 | cut -d'(' -f1 | xargs)
        disk_type=$(echo "$disk_details" | grep "Solid State" | grep -q "Yes" && echo "SSD" || echo "HDD")

        size_bytes=$(echo "$disk_details" | grep "Disk Size" | grep -o '[0-9]* Bytes' | awk '{print $1}')
        size_gb=$(echo "scale=2; ${size_bytes:-0} / 1073741824" | bc 2>/dev/null || echo "0")

        # Try smartctl for wear data; derive healthStatus (no longer hardcoded Healthy)
        wear_percent=0
        power_on_hours=0
        smartctl_available=false
        if command -v smartctl &>/dev/null; then
            smart_data=$(smartctl -A "$disk_path" 2>/dev/null)
            if [ -n "$smart_data" ]; then
                smartctl_available=true
                wear_percent=$(echo "$smart_data" | grep "Percentage Used" | awk '{print $NF}' | tr -d ' %' 2>/dev/null)
                [ -z "$wear_percent" ] && wear_percent=0
                power_on_hours=$(echo "$smart_data" | grep "Power On Hours" | awk '{print $NF}' 2>/dev/null)
                [ -z "$power_on_hours" ] && power_on_hours=0
            fi
        fi

        if [ "$smartctl_available" = "true" ]; then
            if [ "$wear_percent" -gt 80 ] 2>/dev/null || [ "$power_on_hours" -gt 30000 ] 2>/dev/null; then
                health_status="Unhealthy"
            elif [ "$wear_percent" -gt 50 ] 2>/dev/null || [ "$power_on_hours" -gt 20000 ] 2>/dev/null; then
                health_status="Warning"
            else
                health_status="Healthy"
            fi
        else
            health_status="Unknown"
        fi

        if [ -n "$physical_disks" ]; then
            physical_disks="$physical_disks,"
        fi
        physical_disks="$physical_disks{ \"name\": \"$disk_name\", \"mediaType\": \"$disk_type\", \"size\": ${size_bytes:-0}, \"sizeGB\": $size_gb, \"healthStatus\": \"$health_status\", \"operationalStatus\": \"OK\" }"
    fi
done <<< "$disk_info"

# Count volumes (count number of "drive" keys instead of commas)
vol_count=$(echo "$volumes" | grep -o '"drive"' | wc -l | tr -d ' ')

# Output JSON
cat << EOF
{
  "dimension": "health",
  "status": "$worst_status",
  "summary": "$vol_count 个磁盘, 状态: $worst_status",
  "details": {
    "volumes": [$volumes],
    "physicalDisks": [$physical_disks]
  }
}
EOF
