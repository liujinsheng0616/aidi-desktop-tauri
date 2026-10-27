#!/bin/bash
# Startup Toggle Script for macOS - Enable/disable a startup item
# Input: JSON with name, location, and enabled status

input="$1"

name=$(echo "$input" | grep -o '"name":"[^"]*"' | cut -d'"' -f4)
source=$(echo "$input" | grep -o '"source":"[^"]*"' | cut -d'"' -f4)
enabled=$(echo "$input" | grep -o '"enabled":[^,}]*' | cut -d':' -f2)
command=$(echo "$input" | grep -o '"command":"[^"]*"' | cut -d'"' -f4)

success=false
message=""

case "$source" in
    "LoginItems")
        if [ "$enabled" = "true" ]; then
            message="无法通过脚本重新启用登录项，请前往「系统设置 → 通用 → 登录项」手动添加"
        else
            # Remove login item using sfltool (no AppleScript / Apple Events needed)
            sfltool remove com.apple.LSSharedFileList.SessionLoginItems "$name" 2>/dev/null
            if [ $? -eq 0 ]; then
                success=true
                message="Disabled login item: $name"
            else
                message="Failed to disable login item: $name"
            fi
        fi
        ;;
    "LaunchAgents")
        if [ -f "$command" ]; then
            if [ "$enabled" = "true" ]; then
                launchctl load "$command" 2>/dev/null
                success=true
                message="Enabled launch agent: $name"
            else
                launchctl unload "$command" 2>/dev/null
                success=true
                message="Disabled launch agent: $name"
            fi
        else
            message="Launch agent not found: $command"
        fi
        ;;
    "GlobalLaunchAgents")
        message="Modifying global launch agents requires administrator privileges"
        ;;
    *)
        message="Unknown source: $source"
        ;;
esac

cat << EOF
{
  "success": $success,
  "message": "$message"
}
EOF
