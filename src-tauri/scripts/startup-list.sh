#!/bin/bash
# Startup List Script for macOS - Gets all login items
# Returns JSON with startup programs list

items=""
count=0

json_string() {
    local object
    object=$(printf '{"v":""}' | /usr/bin/plutil -replace v -string "$1" -o - -- -) || return
    object=${object#'{"v":'}
    printf '%s' "${object%'}'}"
}

append_item() {
    local name command source location
    name=$(json_string "$1") || return
    command=$(json_string "$2") || return
    source=$(json_string "$3") || return
    location=$(json_string "$5") || return
    items="${items:+$items,}{ \"name\": $name, \"command\": $command, \"source\": $source, \"enabled\": $4, \"location\": $location }"
    count=$((count + 1))
}

# Get Login Items using sfltool (no AppleScript / Apple Events needed)
login_items=$(sfltool dumpgroup com.apple.LSSharedFileList.SessionLoginItems 2>/dev/null | grep "Name:" | sed 's/.*Name: //')

while IFS= read -r item; do
    if [ -n "$item" ]; then
        append_item "$item" "" "LoginItems" true "Login Items" || exit 1
    fi
done <<< "$login_items"

# Get LaunchAgents (user)
if [ -d "$HOME/Library/LaunchAgents" ]; then
    if ! launch_states=$(launchctl print-disabled "gui/$(id -u)" 2>/dev/null); then
        printf '%s\n' '{"dimension":"startup","status":"error","summary":"无法读取 macOS 启动代理状态","details":{"count":0,"items":[]}}'
        exit 0
    fi
    for plist in "$HOME/Library/LaunchAgents"/*.plist; do
        if [ -f "$plist" ]; then
            name=$(basename "$plist" .plist)
            label=$(/usr/bin/plutil -extract Label raw -expect string -o - -- "$plist" 2>/dev/null)
            [[ -n "$label" && "$label" != */* ]] || continue
            override=$(printf '%s\n' "$launch_states" | awk -F '"' -v label="$label" '
                $2 == label && $3 ~ /=>/ {
                    sub(/^[[:space:]]*=>[[:space:]]*/, "", $3)
                    sub(/[[:space:]]*$/, "", $3)
                    print $3
                    exit
                }
            ')
            enabled=true
            case "$override" in
                disabled|true) enabled=false ;;
                enabled|false) ;;
                *)
                    disabled=$(/usr/bin/plutil -extract Disabled raw -expect bool -o - -- "$plist" 2>/dev/null)
                    if [[ "$disabled" == true ]]; then enabled=false; fi
                    ;;
            esac
            append_item "$name" "$plist" "LaunchAgents" "$enabled" "$HOME/Library/LaunchAgents" || exit 1
        fi
    done
fi

# Get global LaunchAgents
if [ -d "/Library/LaunchAgents" ]; then
    for plist in /Library/LaunchAgents/*.plist; do
        if [ -f "$plist" ]; then
            name=$(basename "$plist" .plist)
            append_item "$name" "$plist" "GlobalLaunchAgents" true "/Library/LaunchAgents" || exit 1
        fi
    done
fi

# Determine status
if [ $count -lt 15 ]; then
    status="good"
elif [ $count -lt 25 ]; then
    status="warning"
else
    status="danger"
fi

# Output JSON
cat << EOF
{
  "dimension": "startup",
  "status": "$status",
  "summary": "$count 个启动项",
  "details": {
    "count": $count,
    "items": [$items]
  }
}
EOF
