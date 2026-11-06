"""Wesley probe driver: launch the app, take screenshots, send input, then
serve commands pushed to .github/wesley/cmd/ for a while (agent loop)."""
import json, os, subprocess, sys, time
import pyautogui

pyautogui.FAILSAFE = False
OUT = "wesley-shots"
os.makedirs(OUT, exist_ok=True)
log = open(os.path.join(OUT, "log.txt"), "a")


def say(*a):
    msg = " ".join(str(x) for x in a)
    print(msg, flush=True)
    log.write(msg + "\n")
    log.flush()


def shot(name):
    p = os.path.join(OUT, name + ".png")
    pyautogui.screenshot(p)
    say("shot", p, pyautogui.size())
    return p


app = subprocess.Popen([sys.argv[1]], stdout=open(os.path.join(OUT, "app-stdout.txt"), "w"),
                       stderr=open(os.path.join(OUT, "app-stderr.txt"), "w"))
say("launched pid", app.pid)
for i in range(6):
    time.sleep(5)
    say("t+%ds alive=%s" % (5 * (i + 1), app.poll() is None))
shot("01-launch")
w, h = pyautogui.size()
pyautogui.click(w // 2, h // 2)
time.sleep(1)
pyautogui.hotkey("ctrl", "o")
time.sleep(3)
shot("02-ctrl-o")
pyautogui.press("escape")
time.sleep(2)
pyautogui.click(30, 12)  # roughly the first menu
time.sleep(2)
shot("03-menu")
pyautogui.press("escape")

# Agent loop: poll the branch for numbered command files.
deadline = time.time() + int(os.environ.get("WESLEY_LOOP_SECONDS", "2400"))
branch = os.environ.get("GITHUB_REF_NAME", "")
subprocess.run(["git", "config", "user.name", "wesley-probe"])
subprocess.run(["git", "config", "user.email", "wesley-probe@users.noreply.github.com"])
done = set(f for f in os.listdir(".github/wesley/cmd")) if os.path.isdir(".github/wesley/cmd") else set()
while time.time() < deadline and app.poll() is None:
    subprocess.run(["git", "pull", "-q", "--rebase", "origin", branch], capture_output=True)
    d = ".github/wesley/cmd"
    new = sorted(f for f in (os.listdir(d) if os.path.isdir(d) else []) if f.endswith(".json") and f not in done)
    for f in new:
        done.add(f)
        tag = f[:-5]
        t0 = time.time()
        for step in json.load(open(os.path.join(d, f))):
            op = step[0]
            if op == "click": pyautogui.click(step[1], step[2], button=step[3] if len(step) > 3 else "left")
            elif op == "dclick": pyautogui.doubleClick(step[1], step[2])
            elif op == "drag": pyautogui.moveTo(step[1], step[2]); pyautogui.dragTo(step[3], step[4], duration=0.5)
            elif op == "scroll": pyautogui.moveTo(step[2], step[3]); pyautogui.scroll(step[1])
            elif op == "key": pyautogui.hotkey(*step[1:])
            elif op == "type": pyautogui.typewrite(step[1], interval=0.03)
            elif op == "wait": time.sleep(step[1])
            elif op == "shell": say(subprocess.run(step[1], shell=True, capture_output=True, text=True).stdout[-4000:])
            elif op == "stop": deadline = 0
        os.makedirs(".github/wesley/out", exist_ok=True)
        pyautogui.screenshot(".github/wesley/out/%s.png" % tag)
        open(".github/wesley/out/%s.txt" % tag, "w").write("alive=%s took=%.1fs\n" % (app.poll() is None, time.time() - t0))
        subprocess.run(["git", "add", ".github/wesley/out"])
        subprocess.run(["git", "commit", "-qm", "wesley probe out " + tag])
        for _ in range(3):
            subprocess.run(["git", "pull", "-q", "--rebase", "origin", branch], capture_output=True)
            if subprocess.run(["git", "push", "-q", "origin", "HEAD:" + branch]).returncode == 0:
                break
        say("served", tag, "%.1fs" % (time.time() - t0))
    time.sleep(3)
say("loop end, app alive=%s rc=%s" % (app.poll() is None, app.poll()))
shot("99-end")
app.kill()
