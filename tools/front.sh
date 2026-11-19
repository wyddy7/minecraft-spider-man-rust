#!/bin/zsh
# Bring the Horizon game window to the front (the game must be running).
pid=$(pgrep -f 'target/fast/horizon-client|MacOS/horizon-client' | tail -1)
[ -n "$pid" ] && osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $pid) to true"
