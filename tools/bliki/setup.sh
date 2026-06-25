#!/usr/bin/env bash
# Set up the Bliki engine (Java MediaWiki -> HTML parser) as a comparison
# baseline. Resolves the full classpath with coursier, then compiles the harness.
#
# Requires a JDK (java/javac). Downloads the coursier launcher if `cs` is absent.
set -euo pipefail
cd "$(dirname "$0")"

cs="$(command -v cs || command -v coursier || true)"
if [ -z "$cs" ]; then
  echo "fetching coursier launcher..."
  curl -fsSLo .cs https://github.com/coursier/launchers/raw/master/coursier
  chmod +x .cs
  cs=./.cs
fi

mkdir -p lib out
rm -f lib/*.jar
"$cs" fetch info.bliki.wiki:bliki-core:3.1.0 2>/dev/null | grep '\.jar$' | while read -r j; do
  cp "$j" lib/
done
javac -cp "lib/*" -d out BlikiBench.java

echo "OK: Bliki ready ($(ls lib | wc -l | tr -d ' ') jars). Run: cargo xtask bench-bliki"
