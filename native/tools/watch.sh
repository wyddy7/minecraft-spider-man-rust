#!/bin/zsh
# Rebuilds the plug whenever the mod's sources change, so a running dev game
# (HORIZON_DEV_PLUG=native/target/debug/libspider_man_plug.dylib) picks the edit up within about
# a second: edit a number in spidey.rs, save, look at the game.
NATIVE=${0:a:h:h}
SRC=${MOD_SRC:-$NATIVE/../src}
export PATH=$HOME/.cargo/bin:$PATH
stamp() { stat -f '%m %N' $SRC/*.rs | md5 -q; }
last=$(stamp)
echo "watching $SRC"
while sleep 0.3; do
  now=$(stamp)
  [[ $now == $last ]] && continue
  last=$now
  if (cd $NATIVE && cargo build --offline -q -p spider-man-plug 2>&1 | grep -E '^(error|warning)' -A6); then
    echo "$(date +%T) build has messages above"
  else
    echo "$(date +%T) plug rebuilt: the game reloads it"
  fi
done
