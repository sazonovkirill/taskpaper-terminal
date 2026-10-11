#!/usr/bin/env python3
"""Take documentation screenshots of tpt by driving the real binary in tmux.

Usage: python3 shoot.py /path/to/tpt OUT_DIR [--only NAME[,NAME]]
(normally through `bash site/site.sh shoot`)

Each scene starts tpt fresh on demo.taskpaper.in (dates filled in relative to today, so the
chips always read "today", "tomorrow", ...), in an isolated HOME, under osc-proxy.py so the
truecolor Terminal Dark path (D-128) gets a terminal palette. The scene's keys are sent with
tmux send-keys, the screen is read with `tmux capture-pane -e`, turned into HTML cell by cell,
and rendered to PNG by Chromium (Node Playwright) in JetBrains Mono Nerd Font, downloaded once at
a pinned version and checksum into site/.cache/.
Scenes live in scenes/<group>.json and are written to OUT_DIR/<group>/<name>.png:

    {"file": "demo.taskpaper.in",
     "scenes": [{"name": "outline", "title": "What it shows", "keys": ["j", "text:abc", "wait:1"],
                 "cols": 110, "rows": 30, "config": {"theme": "nord"}, "plain": false}]}

A key is a tmux key name (j, C-g, Escape, Enter, M-x); "text:" types literal text and "wait:"
pauses. "config" adds settings to the app's config.toml; "plain" drops the window frame;
"file" (per file or per scene) names the .taskpaper template, whose {today+N} become dates.
--only takes scene or group names.
"""

import datetime
import hashlib
import html
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
CACHE = HERE.parent / ".cache"
FONT_URL = "https://github.com/ryanoasis/nerd-fonts/releases/download/v3.2.1/JetBrainsMono.tar.xz"
FONT_SHA256 = "6cf8822bc1ca18e34b06578c7499f380c019e6ffc883eed26df5f498dfcc4006"
FONT_STYLES = ("Regular", "Bold", "Italic", "BoldItalic")

# Terminal palette the proxy reports: Tomorrow Night.
FG, BG = "c5c8c6", "1d1f21"
ANSI = ("1d1f21 cc6666 b5bd68 f0c674 81a2be b294bb 8abeb7 c5c8c6 "
        "666666 d54e53 b9ca4a e7c547 7aa6da c397d8 70c0b1 eaeaea").split()

def fetch_fonts():
    """Puts the four JetBrains Mono Nerd Font Mono styles in CACHE / "fonts"."""
    fonts = CACHE / "fonts"
    wanted = [f"JetBrainsMonoNerdFontMono-{style}.ttf" for style in FONT_STYLES]
    if all((fonts / name).exists() for name in wanted):
        return
    fonts.mkdir(parents=True, exist_ok=True)
    archive = fonts / "JetBrainsMono.tar.xz"
    subprocess.run(["curl", "-fsSL", "-o", str(archive), FONT_URL], check=True)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != FONT_SHA256:
        archive.unlink()
        sys.exit(f"shoot.py: {FONT_URL} does not match its pinned checksum")
    with tarfile.open(archive) as tar:
        for name in [*wanted, "OFL.txt"]:
            (fonts / name).write_bytes(tar.extractfile(name).read())
    archive.unlink()


def fill_dates(text):
    today = datetime.date.today()
    return re.sub(
        r"\{today([+-]\d+)?\}",
        lambda m: (today + datetime.timedelta(days=int(m.group(1) or 0))).isoformat(),
        text,
    )


def tmux(*args):
    return subprocess.run(["tmux", *args], check=True, capture_output=True, text=True).stdout


def toml_value(value):
    if isinstance(value, bool):
        return "true" if value else "false"
    if isinstance(value, (int, float)):
        return str(value)
    return json.dumps(value)


def shoot(tpt, scene, document):
    """Runs one scene and returns the screen as tmux captured it, with its escape codes."""
    home = Path(tempfile.mkdtemp(prefix="tpt-shot-"))
    config = home / "config" / "taskpaper-terminal"
    config.mkdir(parents=True)
    settings = {"weather": False, "animations": "off", **scene.get("config", {})}
    (config / "config.toml").write_text(
        "".join(f"{key} = {toml_value(value)}\n" for key, value in settings.items()))
    doc = home / scene.get("save_as", "Launch.taskpaper")
    doc.write_text(fill_dates(document.read_text()))
    hexrgb = lambda h: f"rgb:{h[0:2]*2}/{h[2:4]*2}/{h[4:6]*2}"
    env = {
        "HOME": str(home), "XDG_CONFIG_HOME": str(home / "config"),
        "XDG_DATA_HOME": str(home / "data"), "XDG_STATE_HOME": str(home / "state"),
        "LANG": "C.UTF-8", "TERM": "xterm-256color", "COLORTERM": "truecolor",
        "PALETTE": f"{hexrgb(FG)},{hexrgb(BG)}", "ANSI": ",".join(ANSI),
    }
    session = f"shot-{os.getpid()}-{scene['name']}"
    subprocess.run(["tmux", "kill-session", "-t", session], capture_output=True)
    command = ["env", *(f"{k}={v}" for k, v in env.items()),
               sys.executable, str(HERE / "osc-proxy.py"), tpt, str(doc)]
    tmux("new-session", "-d", "-s", session, "-x", str(scene["cols"]), "-y", str(scene["rows"]),
         " ".join(map(shlex.quote, command)) + "; sleep 60")
    time.sleep(1.5)
    for step in scene.get("keys", []):
        if step.startswith("text:"):
            tmux("send-keys", "-t", session, "-l", step[5:])
        elif step.startswith("wait:"):
            time.sleep(float(step[5:]))
        else:
            tmux("send-keys", "-t", session, step)
        time.sleep(0.15 if step != "Escape" else 0.3)
    time.sleep(0.8)
    screen = tmux("capture-pane", "-p", "-e", "-N", "-t", session)
    tmux("kill-session", "-t", session)
    shutil.rmtree(home, ignore_errors=True)
    return screen


def color256(n):
    if n < 16:
        return ANSI[n]
    if n < 232:
        n -= 16
        steps = [0, 95, 135, 175, 215, 255]
        return "".join(f"{steps[c]:02x}" for c in (n // 36, n // 6 % 6, n % 6))
    v = 8 + (n - 232) * 10
    return f"{v:02x}" * 3


def blend(fg, bg, t=0.5):
    f, b = (tuple(int(h[i:i + 2], 16) for i in (0, 2, 4)) for h in (fg, bg))
    return "".join(f"{round(x * t + y * (1 - t)):02x}" for x, y in zip(f, b))


def to_html(screen, height):
    """Turns tmux's SGR-escaped capture into rows of styled spans."""
    rows = []
    for line in screen.split("\n")[:height]:
        st = {"fg": None, "bg": None, "b": False, "d": False, "i": False, "u": False,
              "r": False, "s": False}
        spans, width = [], 0
        for part in re.split(r"(\x1b\[[0-9;:]*m)", line):
            if part.startswith("\x1b["):
                codes = [int(c or 0) for c in re.split("[;:]", part[2:-1])] or [0]
                k = 0
                while k < len(codes):
                    c = codes[k]
                    if c == 0:
                        st = {key: (None if key in ("fg", "bg") else False) for key in st}
                    elif c in (38, 48) and k + 1 < len(codes):
                        key = "fg" if c == 38 else "bg"
                        if codes[k + 1] == 2:
                            st[key] = "".join(f"{v:02x}" for v in codes[k + 2:k + 5]); k += 4
                        else:
                            st[key] = color256(codes[k + 2]); k += 2
                    elif 30 <= c <= 37: st["fg"] = ANSI[c - 30]
                    elif 90 <= c <= 97: st["fg"] = ANSI[c - 82]
                    elif 40 <= c <= 47: st["bg"] = ANSI[c - 40]
                    elif 100 <= c <= 107: st["bg"] = ANSI[c - 92]
                    elif c == 39: st["fg"] = None
                    elif c == 49: st["bg"] = None
                    elif c == 1: st["b"] = True
                    elif c == 2: st["d"] = True
                    elif c == 3: st["i"] = True
                    elif c == 4: st["u"] = True
                    elif c == 7: st["r"] = True
                    elif c == 9: st["s"] = True
                    elif c == 22: st["b"] = st["d"] = False
                    elif c == 23: st["i"] = False
                    elif c == 24: st["u"] = False
                    elif c == 27: st["r"] = False
                    elif c == 29: st["s"] = False
                    k += 1
                continue
            if not part:
                continue
            fg, bg = st["fg"] or FG, st["bg"] or BG
            if st["r"]:
                fg, bg = bg, fg
            if st["d"]:
                fg = blend(fg, bg)
            css = [f"color:#{fg}", f"background:#{bg}"]
            if st["b"]: css.append("font-weight:700")
            if st["i"]: css.append("font-style:italic")
            deco = [d for d, on in (("underline", st["u"]), ("line-through", st["s"])) if on]
            if deco: css.append("text-decoration:" + " ".join(deco))
            text = "".join(html.escape(c) if ord(c) < 0x2500 else f'<i class="c">{html.escape(c)}</i>'
                           for c in part)
            spans.append(f'<span style="{";".join(css)}">{text}</span>')
            width += len(part)
        rows.append("".join(spans))
    return "\n".join(rows)


PAGE = """<!doctype html><html><head><meta charset="utf-8"><style>
@font-face{{font-family:T;src:url("fonts/JetBrainsMonoNerdFontMono-Regular.ttf")}}
@font-face{{font-family:T;font-weight:700;src:url("fonts/JetBrainsMonoNerdFontMono-Bold.ttf")}}
@font-face{{font-family:T;font-style:italic;src:url("fonts/JetBrainsMonoNerdFontMono-Italic.ttf")}}
@font-face{{font-family:T;font-weight:700;font-style:italic;src:url("fonts/JetBrainsMonoNerdFontMono-BoldItalic.ttf")}}
html,body{{margin:0;background:{outer}}}
#shot{{display:inline-block;padding:{pad}}}
.win{{border-radius:10px;overflow:hidden;background:#{bg};{shadow}}}
.bar{{height:30px;background:#2b2d30;display:{bar};align-items:center;padding-left:12px;gap:8px;
  font:13px -apple-system,system-ui,sans-serif;color:#9a9da0}}
.dot{{width:12px;height:12px;border-radius:50%}}
.title{{flex:1;text-align:center;margin-right:68px}}
pre{{margin:0;padding:{inner};font:15px/1.3 T;color:#{fg};background:#{bg};
  font-variant-ligatures:none;width:{cols}ch}}
.c{{display:inline-block;width:1ch;font-style:inherit;overflow:visible;white-space:pre}}
</style></head><body><div id="shot"><div class="win"><div class="bar">
<span class="dot" style="background:#ff5f57"></span><span class="dot" style="background:#febc2e"></span>
<span class="dot" style="background:#28c840"></span><span class="title">{title}</span>
</div><pre>{body}</pre></div></div></body></html>"""

RENDER = """
const { chromium } = require('playwright');
(async () => {
  let browser;
  try { browser = await chromium.launch(); }
  catch (e) { browser = await chromium.launch({ executablePath: '/opt/pw-browsers/chromium' }); }
  const page = await browser.newPage({ deviceScaleFactor: 2, viewport: { width: 1400, height: 900 } });
  for (const [src, out] of JSON.parse(process.argv[1])) {
    await page.goto('file://' + src);
    await page.evaluate(() => document.fonts.ready);
    await (await page.$('#shot')).screenshot({ path: out, omitBackground: true });
  }
  await browser.close();
})();
"""


def load_scenes():
    """Every scene of every scenes/*.json file, with its group (the file's name) and document."""
    for path in sorted((HERE / "scenes").glob("*.json")):
        spec = json.loads(path.read_text())
        for scene in spec["scenes"]:
            scene = {"cols": 110, "rows": 30, **scene}
            document = HERE / scene.get("file", spec.get("file", "demo.taskpaper.in"))
            yield path.stem, scene, document


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    only = None
    if "--only" in sys.argv:
        only = set(sys.argv[sys.argv.index("--only") + 1].split(","))
        args.remove(sys.argv[sys.argv.index("--only") + 1])
    tpt = str(Path(args[0]).resolve())
    out_dir = Path(args[1]).resolve()
    fetch_fonts()
    jobs = []
    for group, scene, document in load_scenes():
        name = scene["name"]
        if only and name not in only and group not in only:
            continue
        body = to_html(shoot(tpt, scene, document), scene["rows"])
        plain = scene.get("plain", False)
        page = CACHE / f"{group}-{name}.html"
        page.write_text(PAGE.format(
            body=body, fg=FG, bg=BG, cols=scene["cols"],
            title=html.escape(scene.get("window_title", f"tpt — {scene.get('save_as', 'Launch.taskpaper')}")),
            outer="transparent", pad="0" if plain else "28px",
            inner="6px 8px" if plain else "10px 12px",
            shadow="" if plain else "box-shadow:0 12px 32px rgba(0,0,0,.35)",
            bar="none" if plain else "flex"))
        (out_dir / group).mkdir(parents=True, exist_ok=True)
        jobs.append((str(page), str(out_dir / group / f"{name}.png")))
        print(f"{group}/{name}: {scene.get('title', '')}")
    subprocess.run(["node", "-e", RENDER, json.dumps(jobs)], check=True,
                   env={**os.environ, "NODE_PATH": subprocess.run(
                       ["npm", "root", "-g"], capture_output=True, text=True).stdout.strip()})
    for page, out in jobs:
        Path(page).unlink()
        print(out)


if __name__ == "__main__":
    main()
