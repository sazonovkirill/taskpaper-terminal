import os, pty, sys, select, re, tty, termios, signal, struct, fcntl
# Copy of scenario-testing/osc-proxy.py that also takes the 16 colors from env ANSI (comma-separated hex).
# env PALETTE: fg,bg  e.g. "rgb:0000/0000/0000,rgb:ffff/ffff/ffff"; TERMINATOR bel|st; SKIP comma list of osc10,osc11,osc4,da1
fg, bg = os.environ.get("PALETTE", "rgb:dddd/dddd/dddd,rgb:1e1e/1e1e/1e1e").split(",")
term = "\x07" if os.environ.get("TERMINATOR", "st") == "bel" else "\x1b\\"
skip = os.environ.get("SKIP", "").split(",")
ansi = os.environ["ANSI"].split(",") if os.environ.get("ANSI") else ["000000","cd0000","00cd00","cdcd00","0000ee","cd00cd","00cdcd","e5e5e5","7f7f7f","ff0000","00ff00","ffff00","5c5cff","ff00ff","00ffff","ffffff"]
log = open(os.environ.get("PLOG", "/dev/null"), "a")
pid, fd = pty.fork()
if pid == 0:
    os.execvp(sys.argv[1], sys.argv[1:])
sz = fcntl.ioctl(0, termios.TIOCGWINSZ, b"\0"*8); fcntl.ioctl(fd, termios.TIOCSWINSZ, sz)
def winch(*a):
    s = fcntl.ioctl(0, termios.TIOCGWINSZ, b"\0"*8); fcntl.ioctl(fd, termios.TIOCSWINSZ, s); os.kill(pid, signal.SIGWINCH)
signal.signal(signal.SIGWINCH, winch)
tty.setraw(0)
buf = b""
while True:
    try:
        r, _, _ = select.select([0, fd], [], [])
    except InterruptedError:
        continue
    if 0 in r:
        d = os.read(0, 4096)
        if not d: break
        os.write(fd, d)
    if fd in r:
        try: d = os.read(fd, 65536)
        except OSError: break
        if not d: break
        s = d.decode("latin1")
        log.write(repr(s[:300]) + "\n"); log.flush()
        out = ""
        if "osc10" not in skip and "\x1b]10;?" in s: out += "\x1b]10;" + fg + term
        if "osc11" not in skip and "\x1b]11;?" in s: out += "\x1b]11;" + bg + term
        if "osc4" not in skip:
            for m in re.finditer(r"\x1b\]4;(\d+);\?", s):
                i = int(m.group(1)); h = ansi[i]
                out += "\x1b]4;%d;rgb:%s/%s/%s%s" % (i, h[0:2]*2, h[2:4]*2, h[4:6]*2, term)
        if "da1" not in skip and "\x1b[c" in s: out += "\x1b[?62;22c"
        if out:
            import time; time.sleep(float(os.environ.get('DELAY','0'))); os.write(fd, out.encode())
        os.write(1, d)
