#!/bin/sh
mydir=$(dirname "$(readlink -f "$0")")
cd "$mydir"

log="$mydir/holyland.log"

export HOME="$mydir"
export LD_LIBRARY_PATH="$mydir:/mnt/SDCARD/.tmp_update/lib/parasyte:/customer/lib:/customer/lib/parasyte:$LD_LIBRARY_PATH"

# Onion's libSDL2 mmiyoo backend is opt-in via env vars (per PICO-8 / Drastic).
export SDL_VIDEODRIVER=mmiyoo
export EGL_VIDEODRIVER=mmiyoo

echo "launch: starting Holy Land from $mydir" >> "$log"

./holyland >> "$log" 2>&1
status=$?
echo "launch: Holy Land exited status=$status" >> "$log"
sync
exit "$status"
