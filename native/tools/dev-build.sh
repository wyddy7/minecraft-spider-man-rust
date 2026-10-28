#!/bin/zsh
# Development build: the dev hooks (native/dev/) staged on the dev Horizon
# commit (tools/socket-commit.sh makes it), sound on. Then run the game with
# HORIZON_DEV_PLUG pointing at the plug and tools/watch.sh in a terminal.
# macOS: horizon-mixins can't replace an existing stage, so it is removed first.
set -e
NATIVE=${0:a:h:h}
export PATH=$HOME/.cargo/bin:$PATH MOD_ROOT=$NATIVE/dev
export HORIZON_FEATURES=${HORIZON_FEATURES-audio}
rm -rf $MOD_ROOT/target/mixins/modded
# A Cargo target directory to reuse (a full Horizon build is several GB).
TARGET=${HORIZON_TARGET:-$MOD_ROOT/target/horizon-cargo}
START=$(date +%s)
uv run --quiet --python 3.12 python $NATIVE/tools/horizon.py build --target-dir $TARGET > $MOD_ROOT/target-build.json 2>&1 || {
  tail -c 3000 $MOD_ROOT/target-build.json; grep -E '^error' -A12 $MOD_ROOT/target/horizon-build.log | head -80; exit 1; }
echo "built in $(( $(date +%s) - START ))s: $TARGET/fast/horizon-client"
