#!/usr/bin/env bash
# Packages a standalone macOS release: the release binary + frontend dist +
# just the GStreamer plugins/libraries this app actually uses, so end users
# don't need GStreamer installed via Homebrew themselves.
#
# UNTESTED - ported from scripts/bundle-linux.sh (which was verified end-to-end
# on Linux, including a real crash discovered and fixed along the way) but I
# have no Mac to run this on. Expect to iterate based on whatever actually
# happens when you run it - that's normal, not a sign something's wrong with
# the approach.
#
# Requires dylibbundler (`brew install dylibbundler`) - a purpose-built tool
# for exactly this (collect a binary's dylib deps, copy them alongside it,
# rewrite install names to @executable_path-relative references). Hand-rolling
# otool -L parsing to handle @rpath/@loader_path resolution myself, untested,
# seemed like a worse bet than using the tool the macOS packaging community
# already built for this.
#
# Same plugin exclusions as the Linux script, for the same reasons:
# - No gst-libav (avdec_aac): pulls in a full FFmpeg build. Excluding it makes
#   core/src/state.rs's existing avdec_aac->faad fallback pick faad instead.
# - No webrtc/dtls/nice/sctp: on Linux, bundling these crashed the process
#   during DTLS certificate generation (OpenSSL's crypto "providers" are their
#   own hidden dynamically-loaded system, one layer deeper than GStreamer's).
#   This may or may not reproduce identically with macOS's LibreSSL/OpenSSL,
#   but the app already defaults to FLV preview, so WebRTC preview simply
#   isn't available in this bundled build - use the native build (with
#   GStreamer installed via Homebrew) if you need it. It should fail cleanly
#   (HTTP 500) rather than crash if this assumption turns out wrong too.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE_DIR="$ROOT_DIR/core"
OUT_DIR="${1:-$ROOT_DIR/dist-bundle-macos}"

BINARY="$CORE_DIR/target/release/duplicast-core"
if [ ! -f "$BINARY" ]; then
  echo "error: $BINARY not found - run 'cargo build --release' in core/ first" >&2
  exit 1
fi

if ! command -v dylibbundler >/dev/null 2>&1; then
  echo "error: dylibbundler not found - install it with 'brew install dylibbundler'" >&2
  exit 1
fi

PLUGIN_NAMES=(
  coreelements typefindfunctions app flv videoparsersbad audioparsers
  faad audioconvert audioresample opus rtp rtmp
)

GST_PLUGIN_DIR="$(pkg-config --variable=pluginsdir gstreamer-1.0)"
echo "GStreamer plugin dir: $GST_PLUGIN_DIR"

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/lib/gstreamer-1.0" "$OUT_DIR/libs"

cp "$BINARY" "$OUT_DIR/"
cp -r "$ROOT_DIR/client/dist" "$OUT_DIR/dist"

echo "==> Resolving plugin files"
plugin_files=()
for name in "${PLUGIN_NAMES[@]}"; do
  f="$GST_PLUGIN_DIR/libgst${name}.dylib"
  if [ -f "$f" ]; then
    cp "$f" "$OUT_DIR/lib/gstreamer-1.0/"
    plugin_files+=("$OUT_DIR/lib/gstreamer-1.0/libgst${name}.dylib")
  else
    echo "warning: plugin libgst${name}.dylib not found at $f, skipping" >&2
  fi
done

echo "==> Fixing up the main binary's dependencies"
dylibbundler -od -b \
  -x "$OUT_DIR/duplicast-core" \
  -d "$OUT_DIR/libs" \
  -p "@executable_path/libs/"

echo "==> Fixing up each plugin's dependencies (same shared libs/ destination)"
for f in "${plugin_files[@]}"; do
  dylibbundler -od -b \
    -x "$f" \
    -d "$OUT_DIR/libs" \
    -p "@executable_path/libs/"
done

cat > "$OUT_DIR/duplicast-core.sh" <<'LAUNCHER'
#!/usr/bin/env bash
# dylibbundler rewrites dylib install names to @executable_path-relative paths,
# so libs/ is found automatically. GST_PLUGIN_PATH still needs to be set
# explicitly - GStreamer's plugin discovery is dlopen-based, a separate
# mechanism from normal dylib linking that dylibbundler doesn't touch.
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export GST_PLUGIN_PATH="$DIR/lib/gstreamer-1.0"
export GST_PLUGIN_SYSTEM_PATH_1_0=""
export DUPLICAST_STATIC_DIR="$DIR/dist"
exec "$DIR/duplicast-core" "$@"
LAUNCHER
chmod +x "$OUT_DIR/duplicast-core.sh"

echo "==> Done: $OUT_DIR"
du -sh "$OUT_DIR"
