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
# What it loads: this tree's `viewer/dist`, embedded in the binary. A debug
# build without Tauri's `custom-protocol` feature loads `devUrl` instead -
# http://localhost:4321, whatever dev server answers there - and on 2026-09-23
# that is what "Smoke passed" had been photographing while a dev server ran on
# the same machine. The build below turns the feature on, and Tauri then
# refuses to compile without `viewer/dist`. The page also has to prove where it
# came from: the bundle dials its WebSocket on CYPCB_SMOKE_WS_PORT (default
# 4329), this script listens there, and a window that never dialled fails. A
# shipped desktop build dials nothing, so the bundle is built with
# CYPCB_DESKTOP_DEV_SOCKET=1, the developer's switch that turns the dial on.
#
# Needs Xvfb and ImageMagick's `import`, both of which the container already
# has; `scripts/setup-dev.sh` installs neither, so this exits with a message
# rather than a stack trace when they are missing.

cd "$(dirname "$0")/.."

APP=target/debug/cypcb-desktop
FRONTEND=viewer/dist
SECONDS_UP=${SECONDS_UP:-12}
SHOT=${SHOT:-/tmp/cypcb-desktop-smoke.png}
WS_PORT=${CYPCB_SMOKE_WS_PORT:-4329}

# A binary and a bundle are only as new as the last build, and this script
# photographs both. On 2026-09-05 the tree had `viewer/dist` from 2026-08-27
# and `viewer/src` from 2026-09-03: a hand run would have started today's
# binary onto a frontend a week old and called the result a passing smoke
# test. The same trap has cost this project two measurements already, both
# recorded in docs/TRACKER.md - a DRC reading from a stale `target/release`,
# and a `corner` the shipped grammar accepted and the built binary refused.
newest_mtime() {
    find "$@" -type f -printf '%T@\n' 2>/dev/null | sort -rn | head -1 | cut -d. -f1
}

fresher_than() {
    # $1 artifact, $2 what to call it, rest: the sources it is built from
    local artifact="$1" name="$2"
    shift 2
    local built sources
    built=$(newest_mtime "$artifact")
    sources=$(newest_mtime "$@")
    [ -n "$built" ] && [ -n "$sources" ] || return 0
    [ "$built" -ge "$sources" ]
}

# tauri.conf.json points `frontendDist` here. Without it the window opens onto
# nothing, which is a passing smoke test and a broken application - so the
# absence is an error rather than something to discover from a white screen.
# It comes before the check for the display tools, so a machine without them
# still hears about a missing bundle rather than a skip.
[ -d "$FRONTEND" ] && [ -n "$(ls -A "$FRONTEND" 2>/dev/null)" ] || {
    echo "[ERROR] $FRONTEND is empty; the app would open onto nothing."
    echo "        cd viewer && CYPCB_DESKTOP_DEV_SOCKET=1 CYPCB_WS_PORT=$WS_PORT npm run build"
    exit 1
}

fresher_than "$FRONTEND" "the frontend bundle" viewer/src viewer/index.html || {
    echo "[ERROR] $FRONTEND is older than viewer/src; the window would show"
    echo "        a build nobody wrote today."
    echo "        cd viewer && CYPCB_DESKTOP_DEV_SOCKET=1 CYPCB_WS_PORT=$WS_PORT npm run build"
    exit 1
}

for tool in xvfb-run import; do
    command -v "$tool" >/dev/null || {
        echo "[SKIP] $tool not found. apt-get install -y xvfb imagemagick"
        exit 0
    }
done

# Somebody else on the port would take the dial this script counts.
if (exec 3<>"/dev/tcp/127.0.0.1/$WS_PORT") 2>/dev/null; then
    echo "[ERROR] something already listens on $WS_PORT; set CYPCB_SMOKE_WS_PORT"
    exit 1
fi

# The binary is built here, not trusted from whoever built it last: without
# `custom-protocol` it would open the dev server's page, not this tree's.
echo "[0/2] building $APP with Tauri's custom-protocol feature"
if ! BUILD_OUT=$(cargo build -p cypcb-desktop --features tauri/custom-protocol 2>&1); then
    echo "$BUILD_OUT" | tail -20
    echo "[FAIL] $APP did not build against $FRONTEND"
    exit 1
fi

echo "[1/2] starting $APP on a virtual display for ${SECONDS_UP}s"

# The temporary files, removed however this script ends. The two `rm -f "$LOG"`
# lines further down cover the paths that reach them; `set -e` is on, so any
# failure before them exits without cleaning up, and an interrupted run never
# reaches them either. A trap runs on the way out whatever the way out is, and
# the runner script had no `rm` on those paths at all.
RUNNER=$(mktemp)
LOG=$(mktemp)
DIALS=$(mktemp)
LISTENER=
trap 'rm -f "$RUNNER" "$LOG" "$DIALS"; [ -z "$LISTENER" ] || kill "$LISTENER" 2>/dev/null' EXIT

# Counts connections on the port the bundle was built to dial. It accepts and
# hangs up; the page reconnects, and one connection is all the count needs.
python3 - "$WS_PORT" "$DIALS" <<'PY' &
import socket, sys, threading
port, out = int(sys.argv[1]), sys.argv[2]
count, lock = 0, threading.Lock()
def serve(family, host):
    global count
    try:
        server = socket.socket(family, socket.SOCK_STREAM)
        server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        server.bind((host, port))
        server.listen(16)
    except OSError:
        return
    while True:
        conn, _ = server.accept()
        conn.close()
        with lock:
            count += 1
            with open(out, "w") as f:
                f.write(str(count))
for family, host in ((socket.AF_INET, "127.0.0.1"), (socket.AF_INET6, "::1")):
    threading.Thread(target=serve, args=(family, host), daemon=True).start()
threading.Event().wait()
PY
LISTENER=$!
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
set +e
xvfb-run -a -s "-screen 0 1280x900x24" "$RUNNER" > "$LOG" 2>&1
STATUS=$?
set -e
grep -v "libEGL warning" "$LOG" || true
if [ "$STATUS" -ne 0 ]; then
    echo "[FAIL] the app did not survive ${SECONDS_UP}s (exit $STATUS)"
    cat "$LOG"
    rm -f "$LOG"
    exit 1
fi
rm -f "$LOG"
echo "[OK] it was still running after ${SECONDS_UP}s"

DIALLED=$(cat "$DIALS" 2>/dev/null)
if [ -z "$DIALLED" ]; then
    echo "[FAIL] the window never dialled $WS_PORT: it did not show this tree's"
    echo "       $FRONTEND built with CYPCB_DESKTOP_DEV_SOCKET=1 CYPCB_WS_PORT=$WS_PORT"
    exit 1
fi
echo "[OK] the window dialled $WS_PORT ($DIALLED connections): its page is a bundle built for it"

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
