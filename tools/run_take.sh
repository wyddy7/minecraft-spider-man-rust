#!/bin/zsh
# One recorded take in the NYC map, to compare frame by frame with the
# reference video: the reference's rhythm (shoot, swing, let go at the top,
# flip, fall, shoot again), then a zip. Usage: run_take.sh OUT.mp4 [x y z yaw pitch]
OUT=$1; shift
SPOT=${@:-"-470 70 -60 90 0"}
TOOLS=${0:a:h}
# The Horizon checkout beside this repo (override with HORIZON=...).
HORIZON=${HORIZON:-$TOOLS/../../horizon}
MOD=$TOOLS/../target/horizon-mod/swing.hmod
C() { $HORIZON/target/fast/horizon-ctl --timeout 60 "$@"; }
cd $HORIZON
pgrep -f 'target/fast/horizon-client' >/dev/null || { C launch --world NYC --wait --dev-mod $MOD >/dev/null; C wait-ready >/dev/null; C key f5 down >/dev/null; C key f5 up >/dev/null; }
C command "effect give @s minecraft:night_vision infinite 0 true" >/dev/null
C command "time set noon" >/dev/null
C command "gamemode creative" >/dev/null
# A local recording sign, if there is one (tools/rec_sign.py is not versioned).
[ -f $TOOLS/rec_sign.py ] && { pgrep -f rec_sign.py >/dev/null || (uv run --quiet --with pillow python $TOOLS/rec_sign.py >/dev/null 2>&1 &) }
$TOOLS/front.sh >/dev/null; sleep 1.5
C command "tp @s $SPOT" >/dev/null; sleep 0.3
ffmpeg -v error -y -f avfoundation -framerate 30 -capture_cursor 0 -i "Capture screen 0:none" -t 15 -c:v libx264 -preset ultrafast -crf 22 "$OUT" &
FF=$!
sleep 0.8
hold() { C key $1 down >/dev/null; sleep $2; C key $1 up >/dev/null; }
hold r 1.1; sleep 0.9
hold r 1.3; sleep 0.9
hold r 1.0; sleep 0.7
hold z 1.4; sleep 0.8
hold r 1.2; sleep 1.0
wait $FF
pkill -f rec_sign.py
C screenshot ${OUT:r}-ref.png >/dev/null
