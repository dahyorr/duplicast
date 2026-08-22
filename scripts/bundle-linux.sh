#!/usr/bin/env bash
# Packages a standalone Linux release: the release binary + frontend dist +
# just the GStreamer plugins/libraries this app actually uses, so end users
# don't need GStreamer installed system-wide.
#
# Deliberately excludes gst-libav (avdec_aac) - that single plugin pulls in a
# full FFmpeg build (libx264/libx265/libaom/Vulkan/font-shaping/etc, ~145MB of
# transitive deps) just to decode AAC audio for the WebRTC preview. Excluding
# it makes core/src/state.rs's existing avdec_aac->faad fallback pick faad
# instead - a dedicated small AAC decoder with no FFmpeg dependency - since
# ElementFactory::find only sees what's actually in GST_PLUGIN_PATH at runtime.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE_DIR="$ROOT_DIR/core"
OUT_DIR="${1:-$ROOT_DIR/dist-bundle-linux}"

BINARY="$CORE_DIR/target/release/duplicast-core"
if [ ! -f "$BINARY" ]; then
  echo "error: $BINARY not found - run 'cargo build --release' in core/ first" >&2
  exit 1
fi

# Plugins the app's pipelines actually construct (see pipeline.rs, state.rs's
# relay/webrtc attach functions). libav is intentionally excluded - see above.
#
# WebRTC preview is intentionally NOT bundled: webrtcbin also needs
# dtlssrtpenc/dtlssrtpdec (dtls plugin), nicesrc/nicesink (nice plugin), and
# sctpenc/sctpdec (sctp plugin), all separate plugin files from webrtcbin
# itself. Bundling those crashes the process during DTLS certificate
# generation - OpenSSL loads its crypto "providers" as their own separate
# dynamically-loaded modules (an analogous hidden-plugin problem to
# GStreamer's own, just one layer deeper), which a plain ldd-based copy
# doesn't capture, and reproducing OpenSSL's full provider setup per-OS
# reliably is a much bigger undertaking than this bundle is trying to solve.
# Since the app already defaults to FLV preview (see StreamPreview.tsx),
# losing WebRTC in the bundled build only affects an optional secondary mode -
# it fails cleanly (500 response) rather than crashing, and users who want
# WebRTC preview should use the native build with GStreamer installed.
PLUGIN_NAMES=(
  coreelements typefindfunctions app flv videoparsersbad audioparsers
  faad audioconvert audioresample opus rtp rtmp
)

GST_PLUGIN_DIR="$(pkg-config --variable=pluginsdir gstreamer-1.0)"

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR/lib/gstreamer-1.0"

cp "$BINARY" "$OUT_DIR/"
cp -r "$ROOT_DIR/client/dist" "$OUT_DIR/dist"

echo "==> Resolving plugin files"
plugin_files=()
for name in "${PLUGIN_NAMES[@]}"; do
  f="$GST_PLUGIN_DIR/libgst${name}.so"
  if [ -f "$f" ]; then
    plugin_files+=("$f")
  else
    echo "warning: plugin libgst${name}.so not found, skipping" >&2
  fi
done

echo "==> Computing full transitive shared-library closure"
python3 - "$OUT_DIR" "${plugin_files[@]}" "$BINARY" <<'PYEOF'
import subprocess, os, shutil, sys

out_dir = sys.argv[1]
seed_files = sys.argv[2:]

all_libs = set()
seen = set()
queue = list(seed_files)
while queue:
    f = queue.pop()
    if f in seen:
        continue
    seen.add(f)
    try:
        out = subprocess.run(["ldd", f], capture_output=True, text=True, timeout=10).stdout
    except Exception:
        continue
    for line in out.splitlines():
        line = line.strip()
        if "=>" in line:
            rhs = line.split("=>")[1].strip()
            if rhs.startswith("/"):
                path = rhs.split(" ")[0]
                if os.path.exists(path):
                    real = os.path.realpath(path)
                    if real not in all_libs:
                        all_libs.add(real)
                        queue.append(real)

lib_dir = os.path.join(out_dir, "lib")
plugin_dir = os.path.join(out_dir, "lib", "gstreamer-1.0")

for f in seed_files:
    if "/gstreamer-1.0/" in f:
        shutil.copy2(f, plugin_dir)

total = 0
for lib in sorted(all_libs):
    shutil.copy2(lib, lib_dir)
    total += os.path.getsize(lib)

print(f"Copied {len(all_libs)} shared libraries ({total/1024/1024:.1f} MB) + {len([f for f in seed_files if '/gstreamer-1.0/' in f])} plugins")
PYEOF

cat > "$OUT_DIR/duplicast-core.sh" <<'LAUNCHER'
#!/usr/bin/env bash
# Launches the bundled build with only the bundled GStreamer libs/plugins
# visible - not whatever (if anything) is installed system-wide - so behavior
# is reproducible regardless of the host system's GStreamer version.
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export LD_LIBRARY_PATH="$DIR/lib"
export GST_PLUGIN_PATH="$DIR/lib/gstreamer-1.0"
export GST_PLUGIN_SYSTEM_PATH_1_0=""
export DUPLICAST_STATIC_DIR="$DIR/dist"
exec "$DIR/duplicast-core" "$@"
LAUNCHER
chmod +x "$OUT_DIR/duplicast-core.sh"

echo "==> Done: $OUT_DIR"
du -sh "$OUT_DIR"
