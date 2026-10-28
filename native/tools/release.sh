#!/bin/zsh
# The shipped native mod: regenerates native/mixins/spider-man/ from the mod
# (src/) and the dev hooks, then stages and builds it on the clean Horizon
# commit in native/compatibility.json. `--generate-only` skips the build.
set -e
NATIVE=${0:a:h:h}
export PATH=$HOME/.cargo/bin:$PATH MOD_ROOT=$NATIVE
REV=$(python3 -c "import json;print(json.load(open('$NATIVE/compatibility.json'))['horizon_revision'])")
uv run --quiet --python 3.12 python $NATIVE/tools/release.py --dev-bundle $NATIVE/dev/mixins/spidey-socket \
  --mod-src $NATIVE/../src --out $NATIVE --horizon-revision $REV
[[ $1 == --generate-only ]] && exit 0
export HORIZON_FEATURES=${HORIZON_FEATURES-audio}
rm -rf $MOD_ROOT/target/mixins/modded
# A Cargo target directory to reuse (a full Horizon build is several GB).
TARGET=${HORIZON_TARGET:-$MOD_ROOT/target/horizon-cargo}
START=$(date +%s)
uv run --quiet --python 3.12 python $NATIVE/tools/horizon.py build --target-dir $TARGET > $MOD_ROOT/target-build.json 2>&1 || {
  tail -c 3000 $MOD_ROOT/target-build.json; grep -E '^error' -A12 $MOD_ROOT/target/horizon-build.log | head -80; exit 1; }
echo "built in $(( $(date +%s) - START ))s: $TARGET/fast/horizon-client"
