#!/bin/bash
# Battery Health Report Script for macOS
# Uses pmset to get battery info
# Returns JSON with battery health info

# Get battery info via pmset
battery_json=$(pmset -g batt 2>/dev/null)

if [ -z "$battery_json" ] || ! echo "$battery_json" | grep -q "InternalBattery"; then
    cat << EOF
{
  "dimension": "battery",
  "status": "info",
  "summary": "无电池或未检测到电池",
  "details": {
    "hasBattery": false
  }
}
EOF
    exit 0
fi

# Extract charge percentage
charge_percent=$(echo "$battery_json" | grep "InternalBattery" | awk '{for(i=1;i<=NF;i++) if($i ~ /%;/) {gsub(/%;/,"",$i); print $i}}')
[ -z "$charge_percent" ] && charge_percent=0

# Get battery cycle count via system_profiler
cycle_count=$(system_profiler SPPowerDataType 2>/dev/null | grep "Cycle Count" | awk '{print $NF}')
[ -z "$cycle_count" ] && cycle_count=0

# Get battery condition
condition=$(system_profiler SPPowerDataType 2>/dev/null | grep "Condition" | cut -d: -f2 | xargs)
[ -z "$condition" ] && condition="Unknown"

# Determine health percent from condition
if echo "$condition" | grep -qi "Normal"; then
    health_percent=100
    status="good"
elif echo "$condition" | grep -qi "Service"; then
    health_percent=50
    status="danger"
else
    health_percent=80
    status="warning"
fi

status_label=""
if [ "$status" = "danger" ]; then
    status_label="需更换"
elif [ "$status" = "warning" ]; then
    status_label="健康度下降"
else
    status_label="良好"
fi

cat << EOF
{
  "dimension": "battery",
  "status": "$status",
  "summary": "电池健康度: ${health_percent}% ($status_label), 循环: ${cycle_count}次",
  "details": {
    "hasBattery": true,
    "batteries": [
      {
        "name": "InternalBattery",
        "chargePercent": ${charge_percent},
        "healthPercent": ${health_percent},
        "cycleCount": ${cycle_count},
        "condition": "$condition"
      }
    ]
  }
}
EOF
