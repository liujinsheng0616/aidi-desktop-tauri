#!/bin/bash
# SSD Health Script for macOS
# Uses diskutil and system_profiler to get disk health info
# Returns JSON with disk reliability info

disks_json=""
worst_status="good"

# Get physical disks via system_profiler
disk_info=$(system_profiler SPSerialATADataType 2>/dev/null)
if [ -z "$disk_info" ]; then
    disk_info=$(system_profiler SPNVMeDataType 2>/dev/null)
fi

# Get disk info via diskutil
diskutil_list=$(diskutil list physical 2>/dev/null | grep "^/dev/disk" | head -5)

while IFS= read -r disk; do
    if [ -n "$disk" ]; then
        disk_path=$(echo "$disk" | awk '{print $1}')
        disk_details=$(diskutil info "$disk_path" 2>/dev/null)

        disk_name=$(echo "$disk_details" | grep "Device / Media Name" | cut -d: -f2 | xargs)
        [ -z "$disk_name" ] && disk_name="$disk_path"

        disk_size_bytes=$(echo "$disk_details" | grep "Disk Size" | grep -o '[0-9]* Bytes' | awk '{print $1}')
        [ -z "$disk_size_bytes" ] && disk_size_bytes=0

        disk_size_gb=$(echo "scale=2; $disk_size_bytes / 1073741824" | bc | awk '{printf "%.2f", $0}')

        disk_type=$(echo "$disk_details" | grep "Solid State" | grep -q "Yes" && echo "SSD" || echo "HDD")
        [ -z "$disk_type" ] && disk_type="Unknown"

        health_status="Healthy"

        # Try smartctl for wear data (if installed)
        wear_percent=0
        power_on_hours=0
        temperature=0
        read_errors=0
        write_errors=0

        if command -v smartctl &>/dev/null; then
            smart_data=$(smartctl -A "$disk_path" 2>/dev/null)
            wear_percent=$(echo "$smart_data" | grep "Percentage Used" | awk '{print $NF}' | tr -d ' %' 2>/dev/null)
            [ -z "$wear_percent" ] && wear_percent=0
            power_on_hours=$(echo "$smart_data" | grep "Power On Hours" | awk '{print $NF}' 2>/dev/null)
            [ -z "$power_on_hours" ] && power_on_hours=0
            temperature=$(echo "$smart_data" | grep "Temperature" | head -1 | awk '{print $NF}' 2>/dev/null)
            [ -z "$temperature" ] && temperature=0
        fi

        if [ "$wear_percent" -gt 80 ] 2>/dev/null || [ "$power_on_hours" -gt 30000 ] 2>/dev/null; then
            worst_status="danger"
        elif [ "$wear_percent" -gt 50 ] 2>/dev/null || [ "$power_on_hours" -gt 20000 ] 2>/dev/null; then
            [ "$worst_status" != "danger" ] && worst_status="warning"
        fi

        if [ -n "$disks_json" ]; then
            disks_json="$disks_json,"
        fi
        disks_json="$disks_json{ \"name\": \"$disk_name\", \"mediaType\": \"$disk_type\", \"sizeGB\": $disk_size_gb, \"healthStatus\": \"$health_status\", \"wearPercent\": $wear_percent, \"powerOnHours\": $power_on_hours, \"temperatureC\": $temperature, \"readErrors\": $read_errors, \"writeErrors\": $write_errors }"
    fi
done <<< "$diskutil_list"

status_label=""
if [ "$worst_status" = "danger" ]; then
    status_label="需更换"
elif [ "$worst_status" = "warning" ]; then
    status_label="磨损较高"
else
    status_label="良好"
fi

disk_count=$(echo "$disks_json" | grep -o '"name"' | wc -l | tr -d ' ')

cat << EOF
{
  "dimension": "ssd",
  "status": "$worst_status",
  "summary": "$disk_count 块磁盘，SSD 状态: $status_label",
  "details": {
    "disks": [$disks_json]
  }
}
EOF
