#!/bin/zsh
# Can a launchd job write into Civ6.app? Run this after changing any privacy setting.
#
#   zsh ~/civvis-tcc-probe.sh
#
# ⚠ THIS CANNOT BE TESTED FROM A TERMINAL. macOS grants TCC per executable and per
# responsible process, and an interactive shell here already holds the grant — so
# probing from your own prompt answers "yes" while the automation is still blocked.
# It has to be a real launchd job, which is what this sets up and tears down.
#
# Why it matters: the CIVVIS mod lives INSIDE Civ6.app and `civ6_civvis_climb.py`
# re-installs it at the start of every attempt. If that write is refused, every
# attempt dies with `PermissionError: cannot install .../CivvisControl` -- and
# preflight still passes first, so the batch looks healthy right up until it isn't.
#
# Which permission: **App Management**, not Full Disk Access. Measured 2026-08-02 on
# the context that DOES work -- it can write another app's bundle but cannot read
# ~/Library/Safari or ~/Library/Application Support/com.apple.TCC, which is exactly
# App Management without FDA. (Full Disk Access is still worth trying because its
# pane has a "+" button and usually subsumes bundle writes, whereas App Management's
# list is normally populated only by apps that ask. Grant, then re-run this.)
#
# Exit 0 = launchd can write, so com.civvis.batchloop is viable.
# Exit 1 = still blocked; keep running the loop from a terminal (see
#          ~/civvis-batch-loop.sh) and treat the LaunchAgent as unavailable.

set -u
LABEL=com.civvis.tccprobe
WORK=$(mktemp -d /tmp/civvis-tccprobe.XXXXXX)
BUNDLE="$HOME/Library/Application Support/Steam/steamapps/common/Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl"

cat > $WORK/probe.zsh <<EOF
#!/bin/zsh
P="$BUNDLE/.civvis-tcc-probe"
if touch "\$P" 2>/dev/null; then rm -f "\$P"; print "ALLOWED"; else print "BLOCKED"; fi
EOF

cat > $WORK/$LABEL.plist <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>$LABEL</string>
<key>ProgramArguments</key><array><string>/bin/zsh</string><string>$WORK/probe.zsh</string></array>
<key>RunAtLoad</key><true/><key>KeepAlive</key><false/>
<key>StandardOutPath</key><string>$WORK/out</string>
<key>StandardErrorPath</key><string>$WORK/out</string>
</dict></plist>
EOF

launchctl bootout gui/501/$LABEL 2>/dev/null
launchctl bootstrap gui/501 $WORK/$LABEL.plist 2>/dev/null
# The job is one `touch`; give launchd a moment to run and reap it.
for _ in 1 2 3 4 5 6 7 8 9 10; do
  [[ -s $WORK/out ]] && break
  sleep 0.5
done
launchctl bootout gui/501/$LABEL 2>/dev/null

result=$(cat $WORK/out 2>/dev/null)
rm -rf $WORK

case $result in
  ALLOWED)
    print "launchd CAN write into Civ6.app."
    print "com.civvis.batchloop is viable. Switch the loop over with:"
    print "  pkill -f civvis-batch-loop.sh        # only when no batch is running"
    print "  launchctl enable gui/501/com.civvis.batchloop"
    print "  launchctl bootstrap gui/501 ~/Library/LaunchAgents/com.civvis.batchloop.plist"
    print "(enable FIRST -- bootstrap silently no-ops on a disabled label)"
    exit 0 ;;
  BLOCKED)
    print "launchd is STILL BLOCKED from writing into Civ6.app."
    print "Grant /bin/zsh App Management (or Full Disk Access, which usually covers it):"
    print "  System Settings > Privacy & Security > App Management"
    print "  System Settings > Privacy & Security > Full Disk Access > + > Cmd-Shift-G > /bin/zsh"
    print "Then run this again. Until it passes, keep the loop running from a terminal:"
    print "  unsetopt BG_NICE; nohup /bin/zsh ~/civvis-batch-loop.sh >> ~/civvis-civ6-runs/batch-loop.nohup.log 2>&1 &"
    exit 1 ;;
  *)
    print "the probe job produced no result -- launchd may have refused to load it"
    exit 1 ;;
esac
