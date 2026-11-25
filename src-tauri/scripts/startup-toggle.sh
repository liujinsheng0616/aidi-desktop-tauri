#!/bin/bash
# Startup Toggle Script for macOS - Enable/disable a startup item
# Input: JSON with name, location, and enabled status

input="$1"

read_input() {
    printf '%s' "$input" | /usr/bin/plutil -extract "$1" raw -expect "$2" -o - -- - 2>/dev/null
}

source=$(read_input source string)
enabled=$(read_input enabled bool)
command=$(read_input command string)

success=false
message=""

case "$source" in
    "LoginItems")
        message="请在「系统设置 → 通用 → 登录项与扩展」管理登录项"
        ;;
    "LaunchAgents")
        if [[ "$enabled" != true && "$enabled" != false ]]; then
            message="缺少有效的自启状态"
        elif [[ "${command%/*}" != "$HOME/Library/LaunchAgents" || "$command" != *.plist || ! -f "$command" ]]; then
            message="找不到启动代理配置"
        else
            label=$(/usr/bin/plutil -extract Label raw -expect string -o - -- "$command" 2>/dev/null)
            if [[ -z "$label" || "$label" == */* ]]; then
                message="无法读取启动代理标签"
            else
                action=disable
                if [[ "$enabled" == true ]]; then action=enable; fi
                # Change the next login's setting without unloading a running agent.
                if output=$(launchctl "$action" "gui/$(id -u)/$label" 2>&1); then
                    success=true
                    message="已更新登录自启状态"
                else
                    message="${output:-macOS 拒绝更新登录自启状态}"
                fi
            fi
        fi
        ;;
    "GlobalLaunchAgents")
        message="Modifying global launch agents requires administrator privileges"
        ;;
    *)
        message="Unknown source: $source"
        ;;
esac

printf '{"success":%s}' "$success" | /usr/bin/plutil -insert message -string "$message" -o - -- -
