# Shared read-only Finder snapshot for macOS trash scanning and cleanup verification.
read_finder_trash() {
    /usr/bin/osascript - "$1" 2>/dev/null <<'APPLESCRIPT'
on run argv
    set includeDetails to item 1 of argv is "details"
    with timeout of 10 seconds
        tell application "Finder"
            set totalBytes to 0
            set itemCount to 0
            set detailLines to {}
            set trashItems to get every item of trash
            repeat with itemRef in trashItems
                set entry to contents of itemRef
                set itemSize to my measureItem(entry)
                set totalBytes to totalBytes + itemSize
                set itemCount to itemCount + 1
                if includeDetails then
                    set itemName to my singleLine(name of entry)
                    set end of detailLines to ((itemSize as text) & tab & itemName)
                end if
            end repeat
        end tell
    end timeout
    set summaryLine to (totalBytes as text) & ":" & (itemCount as text)
    if not includeDetails or itemCount is 0 then return summaryLine
    set AppleScript's text item delimiters to linefeed
    return summaryLine & linefeed & (detailLines as text)
end run

on measureItem(entry)
    tell application "Finder"
        set itemSize to physical size of entry
        if itemSize is missing value then
            if class of entry is folder then
                set itemSize to 0
                set folderItems to get every item of entry
                repeat with childRef in folderItems
                    set itemSize to itemSize + my measureItem(contents of childRef)
                end repeat
            else
                set itemSize to size of entry
            end if
        end if
    end tell
    if itemSize is missing value then error "Trash item size unavailable"
    return itemSize
end measureItem

on singleLine(itemName)
    set AppleScript's text item delimiters to {tab, return, linefeed}
    set nameParts to text items of itemName
    set AppleScript's text item delimiters to " "
    return nameParts as text
end singleLine
APPLESCRIPT
}

trash_snapshot() {
    local result line newline=$'\n'
    result=$(read_finder_trash "${1:-summary}") || return 1
    line=${result%%"$newline"*}
    [[ "$line" =~ ^[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?:[0-9]+$ ]] || return 1
    trash_bytes=$(printf '%s\n' "${line%:*}" | awk '{printf "%.0f", $1}')
    trash_items_count=${line##*:}
    trash_details=
    if [[ "$result" == *"$newline"* ]]; then
        trash_details=${result#*"$newline"}
    fi
}
