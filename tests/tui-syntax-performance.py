"""Read-only time from first diff frame to syntax, then a cached palette return.

Uses an isolated terminal; prints timings only, never PR contents or credentials.
"""
import argparse, errno, fcntl, json, os, pathlib, select, struct, subprocess, termios, time
from vt_screen import Screen

parser = argparse.ArgumentParser()
parser.add_argument('--binary', default=str(pathlib.Path(__file__).resolve().parent.parent / 'tui/target/release/lazyreview'))
parser.add_argument('--pr')
parser.add_argument('--live', action='store_true')
args = parser.parse_args()
if bool(args.pr) != args.live:
    parser.error('Live reads require both --live and --pr URL')
master, slave = os.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 45, 150, 0, 0))
env = {k: v for k, v in os.environ.items() if k != 'NO_COLOR'}
env.update(TERM='xterm-256color', COLORTERM='truecolor')
proc = subprocess.Popen([args.binary, *(['--pr', args.pr] if args.live else ['--demo'])], stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True)
os.close(slave)
screen = Screen()
def wait_for(predicate):
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], .01)
        if ready:
            try:
                data = os.read(master, 65536)
            except OSError as error:
                if error.errno == errno.EIO: break
                raise
            if not data: break
            screen.feed(data)
            if predicate(screen.text()): return time.monotonic()
    raise AssertionError('Expected syntax state was not rendered')
def ready(text, palette):
    return palette in text and palette + ' …' not in text and 'plain fallback' not in text
try:
    first_frame = wait_for(lambda text: 'GitHub Dark' in text)
    colored = time.monotonic() if ready(screen.text(), 'GitHub Dark') else wait_for(lambda text: ready(text, 'GitHub Dark'))
    os.write(master, b'H')
    wait_for(lambda text: ready(text, 'Lunar') and 'GitHub Dark' not in text)
    started = time.monotonic()
    os.write(master, b'H')
    cached = wait_for(lambda text: ready(text, 'GitHub Dark'))
    os.write(master, b'q')
    proc.wait(timeout=3)
    assert proc.returncode == 0
    print(json.dumps({'firstDiffToSyntaxMs': round((colored-first_frame)*1000), 'cachedPaletteMs': round((cached-started)*1000), 'writes': 0}))
finally:
    if proc.poll() is None: proc.kill(); proc.wait()
    os.close(master)
