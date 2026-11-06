"""Agent side of a Windows walkthrough: send one batch of steps and wait for
the runner's screenshot.

  python3 .github/walkthrough/send.py '[["key","ctrl","o"],["wait",2]]'

Run it in a clone checked out on the channel branch. It writes the next
numbered walk/cmd/NNN.json, commits and pushes it, then pulls until
walk/out/NNN.png arrives (about 10 to 15 s), and prints that path and the
answer text.
"""
import json
import os
import subprocess
import sys
import time

steps = json.loads(sys.argv[1])
root = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip()
os.chdir(root)
branch = subprocess.run(["git", "branch", "--show-current"], capture_output=True, text=True, check=True).stdout.strip()
cmd_dir, out_dir = "walk/cmd", "walk/out"
os.makedirs(cmd_dir, exist_ok=True)
subprocess.run(["git", "pull", "-q", "--rebase", "origin", branch], check=True)
n = 1 + max([int(f[:3]) for f in os.listdir(cmd_dir) if f[:3].isdigit()] or [0])
name = "%03d" % n
with open(os.path.join(cmd_dir, name + ".json"), "w") as f:
    json.dump(steps, f)
subprocess.run(["git", "add", cmd_dir], check=True)
subprocess.run(["git", "commit", "-qm", "walkthrough step " + name], check=True)
for _ in range(5):
    if subprocess.run(["git", "push", "-q", "origin", branch]).returncode == 0:
        break
    subprocess.run(["git", "pull", "-q", "--rebase", "origin", branch])
deadline = time.time() + float(os.environ.get("WALK_WAIT", "300"))
png = os.path.join(out_dir, name + ".png")
while time.time() < deadline:
    subprocess.run(["git", "pull", "-q", "--rebase", "origin", branch], capture_output=True)
    if os.path.exists(png):
        print(os.path.abspath(png))
        print(open(os.path.join(out_dir, name + ".txt"), encoding="utf-8").read())
        sys.exit(0)
    time.sleep(3)
sys.exit("no answer to %s after %s s: is the session still running?" % (name, os.environ.get("WALK_WAIT", "300")))
