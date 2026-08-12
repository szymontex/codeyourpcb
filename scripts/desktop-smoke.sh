#!/bin/bash
set -e

# Does the desktop app start, and does it draw anything?
#
#   ./scripts/desktop-smoke.sh
#
# `cypcb-desktop` went uncompiled long enough to collect nine errors from the
# Tauri v1 to v2 move, and once it compiled the next question had no answer
# either: nothing had ever run it. This is the cheapest honest answer - start
# it on a virtual display, wait, photograph the screen, and look at what is
# there.
#
# What it proves: the binary starts, survives, and puts a window with content
# on the screen. What it does not: that the content is the right content, that
# the file picker works, or that a menu click reaches the frontend. Those need
# a person or a UI driver, and a smoke test that claimed them would be lying.
#
# Needs Xvfb and ImageMagick's `import`, both of which the container already
# has; `scripts/setup-dev.sh` installs neither, so this exits with a message
# rather than a stack trace when they are missing.

cd "$(dirname "$0")/.."

APP=target/debug/cypcb-desktop
FRONTEND=viewer/dist
SECONDS_UP=${SECONDS_UP:-12}
SHOT=${SHOT:-/tmp/cypcb-desktop-smoke.png}

for tool in xvfb-run import; do
    command -v "$tool" >/dev/null || {
        echo "[SKIP] $tool not found. apt-get install -y xvfb imagemagick"
        exit 0
    }
done

[ -x "$APP" ] || {
    echo "[ERROR] $APP is not built. cargo build -p cypcb-desktop"
    exit 1
}

# tauri.conf.json points `frontendDist` here. Without it the window opens onto
# nothing, which is a passing smoke test and a broken application - so the
# absence is an error rather than something to discover from a white screen.
[ -d "$FRONTEND" ] && [ -n "$(ls -A "$FRONTEND" 2>/dev/null)" ] || {
    echo "[ERROR] $FRONTEND is empty; the app would open onto nothing."
    echo "        cd viewer && npm run build"
    exit 1
}

echo "[1/2] starting $APP on a virtual display for ${SECONDS_UP}s"

RUNNER=$(mktemp)
cat > "$RUNNER" <<EOF
#!/bin/bash
"$PWD/$APP" &
APP_PID=\$!
sleep $SECONDS_UP
# Still there? A crash would have taken the pid with it.
kill -0 \$APP_PID 2>/dev/null || { echo "GONE"; exit 1; }
import -window root "$SHOT" 2>/dev/null
kill \$APP_PID 2>/dev/null
wait \$APP_PID 2>/dev/null || true
EOF
chmod +x "$RUNNER"

# Not through a pipe. `xvfb-run ... | grep -v` would report grep's status, and
# grep exits 1 when it filters everything out - which is what happens here,
# because the only output is two libEGL warnings about the container having no
# hardware acceleration. The first version of this script did exactly that and
# called a running application dead.
LOG=$(mktemp)
set +e
xvfb-run -a -s "-screen 0 1280x900x24" "$RUNNER" > "$LOG" 2>&1
STATUS=$?
set -e
grep -v "libEGL warning" "$LOG" || true
rm -f "$RUNNER"
if [ "$STATUS" -ne 0 ]; then
    echo "[FAIL] the app did not survive ${SECONDS_UP}s (exit $STATUS)"
    cat "$LOG"
    rm -f "$LOG"
    exit 1
fi
rm -f "$LOG"
echo "[OK] it was still running after ${SECONDS_UP}s"

echo "[2/2] reading what it drew: $SHOT"
python3 - "$SHOT" <<'PY'
import sys
from PIL import Image

shot = Image.open(sys.argv[1]).convert("RGB")
colours = shot.getcolors(maxcolors=1_000_000) or []
colours.sort(reverse=True)
total = shot.size[0] * shot.size[1]
black = next((n for n, rgb in colours if rgb == (0, 0, 0)), 0)

print(f"      {shot.size[0]}x{shot.size[1]}, {len(colours)} distinct colours")
for count, rgb in colours[:3]:
    print(f"      {rgb} covers {100 * count / total:.1f}%")

# An empty Xvfb root is one colour, black. A window with a page in it is
# hundreds. The thresholds are loose on purpose: this asks whether anything
# was drawn, not whether the right thing was.
if len(colours) < 10:
    raise SystemExit(
        f"[FAIL] {len(colours)} colours on screen: nothing was drawn"
    )
if black > 0.98 * total:
    raise SystemExit(
        f"[FAIL] {100 * black / total:.1f}% of the screen is the empty root"
    )
print("[OK] a window with content is on the screen")
PY

echo ""
echo "Smoke passed. It started and drew something; whether that something is"
echo "correct is a question this cannot answer."
