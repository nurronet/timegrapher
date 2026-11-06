"""Runs on the Windows runner: serves walkthrough commands from the channel
branch and answers each with a screenshot.

  driver.py CHANNEL_DIR HEAD_EXE [BASE_EXE]

Commands are JSON files the agent pushes to walk/cmd/ on the channel branch;
each answer is walk/out/<name>.png plus <name>.txt, pushed back to the same
branch. Command files already on the branch when the session starts are
skipped. The step list is in .github/walkthrough/README.md.
"""
import base64
import json
import os
import subprocess
import sys
import time

import pyautogui

pyautogui.FAILSAFE = False
CHANNEL, EXES = sys.argv[1], {"head": sys.argv[2]}
if len(sys.argv) > 3:
    EXES["base"] = sys.argv[3]
BRANCH = os.environ["WALK_CHANNEL"]
TOKEN = os.environ.pop("WALK_TOKEN")
# The app never sees the token: it gets a copy of the environment without it.
APP_ENV = dict(os.environ)
BASIC = base64.b64encode(("x-access-token:" + TOKEN).encode()).decode()
AUTH = ["-c", "http.extraheader=AUTHORIZATION: basic " + BASIC]
# No hooks: nothing in the channel clone runs while the token is in use.
NO_HOOKS = ["-c", "core.hooksPath=" + os.devnull]
CMD = os.path.join(CHANNEL, "walk", "cmd")
OUT = os.path.join(CHANNEL, "walk", "out")
LOGS = os.path.abspath("walk-logs")
os.makedirs(LOGS, exist_ok=True)


def git(*args, auth=False):
    pre = ["git", "-C", CHANNEL] + NO_HOOKS + (AUTH if auth else [])
    return subprocess.run(pre + list(args), capture_output=True, text=True)


class App:
    def __init__(self):
        self.proc, self.which, self.n = None, None, 0

    def launch(self, which="head", args=()):
        self.kill()
        self.n += 1
        name = os.path.join(LOGS, "%s-%d" % (which, self.n))
        self.err = name + "-stderr.txt"
        self.proc = subprocess.Popen([EXES[which]] + list(args), env=APP_ENV,
                                     stdout=open(name + "-stdout.txt", "w"),
                                     stderr=open(self.err, "w"))
        self.which = which

    def kill(self):
        if self.proc and self.proc.poll() is None:
            self.proc.kill()
            self.proc.wait()

    def state(self):
        if not self.proc:
            return "app: not running"
        rc = self.proc.poll()
        s = "app: %s %s" % (self.which, "running" if rc is None else "exited, code %d" % rc)
        tail = open(self.err, errors="replace").read()[-1500:]
        return s + ("\nstderr tail:\n" + tail if tail.strip() else "")


def run(step, app, notes):
    op, a = step[0], step[1:]
    if op == "click":
        pyautogui.click(a[0], a[1], button=a[2] if len(a) > 2 else "left")
    elif op == "dclick":
        pyautogui.doubleClick(a[0], a[1])
    elif op == "move":
        pyautogui.moveTo(a[0], a[1])
    elif op == "drag":
        pyautogui.moveTo(a[0], a[1])
        pyautogui.dragTo(a[2], a[3], duration=0.5)
    elif op == "scroll":
        pyautogui.moveTo(a[1], a[2])
        pyautogui.scroll(a[0])
    elif op == "key":
        pyautogui.hotkey(*a)
    elif op == "type":
        pyautogui.typewrite(a[0], interval=0.02)
    elif op == "wait":
        time.sleep(a[0])
    elif op == "launch":
        app.launch(a[0] if a else "head", a[1:])
        time.sleep(5)
    elif op == "kill":
        app.kill()
    elif op == "shell":
        r = subprocess.run(a[0], shell=True, capture_output=True, text=True, env=APP_ENV, timeout=600)
        notes.append("$ %s\n%s%s" % (a[0], r.stdout[-6000:], r.stderr[-2000:]))
    elif op == "stop":
        return False
    else:
        notes.append("unknown step: %r" % (step,))
    return True


def answer(name, notes, app, t0):
    os.makedirs(OUT, exist_ok=True)
    pyautogui.screenshot(os.path.join(OUT, name + ".png"))
    text = "\n".join([app.state(), "took %.1f s" % (time.time() - t0)] + notes)
    open(os.path.join(OUT, name + ".txt"), "w", encoding="utf-8").write(text + "\n")
    git("add", "walk/out")
    git("commit", "-qm", "walkthrough answer " + name)
    for _ in range(5):
        git("pull", "-q", "--rebase", "origin", BRANCH, auth=True)
        if git("push", "-q", "origin", "HEAD:" + BRANCH, auth=True).returncode == 0:
            return
        time.sleep(2)
    print("could not push", name, flush=True)


def main():
    git("config", "user.name", "windows-walkthrough")
    git("config", "user.email", "windows-walkthrough@users.noreply.github.com")
    done = set(os.listdir(CMD)) if os.path.isdir(CMD) else set()
    deadline = time.time() + 60 * int(os.environ.get("WALK_MINUTES", "120"))
    app = App()
    app.launch("head")
    time.sleep(25)
    answer("ready-" + os.environ.get("GITHUB_RUN_ID", "local"), [], app, time.time())
    going = True
    while going and time.time() < deadline:
        git("pull", "-q", "--rebase", "origin", BRANCH, auth=True)
        new = sorted(f for f in (os.listdir(CMD) if os.path.isdir(CMD) else [])
                     if f.endswith(".json") and f not in done)
        for f in new:
            done.add(f)
            t0, notes = time.time(), []
            try:
                for step in json.load(open(os.path.join(CMD, f), encoding="utf-8")):
                    going = run(step, app, notes)
                    if not going:
                        break
            except Exception as e:  # a bad step is reported, the session goes on
                notes.append("error: %r" % (e,))
            answer(f[:-5], notes, app, t0)
            print("answered", f, flush=True)
        time.sleep(2)
    print(app.state(), flush=True)
    app.kill()


main()
