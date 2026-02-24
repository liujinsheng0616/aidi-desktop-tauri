#!/bin/bash
# Startup List Script for macOS - Gets all login items
# Returns JSON with startup programs list

items=""
count=0

# Get Login Items using osascript
login_items=$(osascript -e 'tell application "System Events" to get the name of every login item' 2>/dev/null | tr ',' '\n')

while IFS= read -r item; do
    item=$(echo "$item" | xargs) # trim whitespace
    if [ -n "$item" ]; then
        if [ -n "$items" ]; then
            items="$items,"
        fi
        items="$items{ \"name\": \"$item\", \"command\": \"\", \"source\": \"LoginItems\", \"enabled\": true, \"location\": \"Login Items\" }"
        count=$((count + 1))
    fi
done <<< "$login_items"

# Get LaunchAgents (user)
if [ -d "$HOME/Library/LaunchAgents" ]; then
    for plist in "$HOME/Library/LaunchAgents"/*.plist; do
        if [ -f "$plist" ]; then
            name=$(basename "$plist" .plist)
            if [ -n "$items" ]; then
                items="$items,"
            fi
            items="$items{ \"name\": \"$name\", \"command\": \"$plist\", \"source\": \"LaunchAgents\", \"enabled\": true, \"location\": \"$HOME/Library/LaunchAgents\" }"
            count=$((count + 1))
        fi
    done
fi

# Get global LaunchAgents
if [ -d "/Library/LaunchAgents" ]; then
    for plist in /Library/LaunchAgents/*.plist; do
        if [ -f "$plist" ]; then
            name=$(basename "$plist" .plist)
            if [ -n "$items" ]; then
                items="$items,"
            fi
            items="$items{ \"name\": \"$name\", \"command\": \"$plist\", \"source\": \"GlobalLaunchAgents\", \"enabled\": true, \"location\": \"/Library/LaunchAgents\" }"
            count=$((count + 1))
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
