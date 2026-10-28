#!/bin/zsh
# The dev Horizon: puts native/dev-reload into a Horizon commit on top of BASE
# (without touching the Horizon checkout's files), points branch
# dev-reload/spider-man at it and pins the dev hooks (native/dev/) to it.
# Usage: socket-commit.sh [BASE]   (default: the shipped pin, native/compatibility.json)
set -e
NATIVE=${0:a:h:h}
HORIZON=${HORIZON:-$NATIVE/../../horizon}
BASE=${1:-$(python3 -c "import json;print(json.load(open('$NATIVE/compatibility.json'))['horizon_revision'])")}
WORK=$(mktemp -d)
trap 'rm -rf $WORK' EXIT
export PATH=$HOME/.cargo/bin:$PATH
git -C $HORIZON archive $BASE | tar -x -C $WORK
mkdir -p $WORK/crates/horizon-dev-reload
cp -R $NATIVE/dev-reload/Cargo.toml $NATIVE/dev-reload/src $WORK/crates/horizon-dev-reload/
python3 - $WORK <<'P'
import sys
w=sys.argv[1]
p=w+'/Cargo.toml'; s=open(p).read(); a='    "crates/horizon-threads",\n'
assert a in s; s=s.replace(a,a+'    "crates/horizon-dev-reload",\n',1); open(p,'w').write(s)
p=w+'/crates/horizon-client/Cargo.toml'; s=open(p).read(); a='horizon-threads = { path = "../horizon-threads" }\n'
assert a in s
s=s.replace(a,a+"# Development only: a native mod's logic reloaded from a rebuilt library.\nhorizon-dev-reload = { path = \"../horizon-dev-reload\", features = [\"host\"] }\n",1)
open(p,'w').write(s)
P
(cd $WORK && cargo metadata --offline --format-version 1 >/dev/null)
export GIT_INDEX_FILE=$WORK/.index
cd $HORIZON
git read-tree $BASE
for f in Cargo.toml Cargo.lock crates/horizon-client/Cargo.toml $(cd $WORK && find crates/horizon-dev-reload -type f); do
  git update-index --add --cacheinfo 100644,$(git hash-object -w $WORK/$f),$f
done
COMMIT=$(git commit-tree $(git write-tree) -p $BASE \
  -m "dev-reload: a movement plug's tick through a reloadable library (development only)")
unset GIT_INDEX_FILE
git branch -f dev-reload/spider-man $COMMIT
SHA=$(git show "${COMMIT}:crates/horizon-client/src/game.rs" | shasum -a 256 | cut -d' ' -f1)
cd $NATIVE/dev
python3 - $COMMIT $SHA <<'P'
import json,re,sys,glob
full,sha=sys.argv[1:]
c=json.load(open("compatibility.json")); c["horizon_revision"]=full
c["source_sha256"]={"crates/horizon-client/src/game.rs":sha}
open("compatibility.json","w").write(json.dumps(c,indent=2)+"\n")
for p in glob.glob("mixins/*/horizon-mixin.toml"):
    s=open(p).read(); s=re.sub(r'horizon_revision = "[^"]*"',f'horizon_revision = "{full}"',s); open(p,"w").write(s)
P
echo "socket commit $COMMIT"
