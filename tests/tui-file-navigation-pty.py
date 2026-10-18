"""File-only n/N navigation uses the visible tree, preserving diff search."""
import errno, fcntl, os, pathlib, select, struct, subprocess, termios, time
from vt_screen import Screen

root = pathlib.Path(__file__).resolve().parent.parent
binary = os.environ.get('LAZYREVIEW_BINARY', str(root / 'tui/target/release/lazyreview'))
master, slave = os.openpty()
fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 45, 180, 0, 0))
proc = subprocess.Popen([binary, '--demo'], stdin=slave, stdout=slave, stderr=slave,
                        env={**os.environ, 'TERM': 'xterm-256color'}, start_new_session=True)
os.close(slave)
screen = Screen(180, 45)

def read(seconds=.18):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        ready, _, _ = select.select([master], [], [], .02)
        if ready:
            try:
                data = os.read(master, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    break
                raise
            if data:
                screen.feed(data)

def send(keys):
    os.write(master, keys.encode())
    read()

def current(name):
    path = 'src/components/command-menu/' + name
    assert path in screen.text(), f'Missing diff path {path}\n{screen.text()}'

try:
    read(.6)
    # n starts on the first leaf, skipping the category and three directories.
    send('nn')
    current('use-keyboard-navigation.ts')
    send('N')
    current('command-menu.tsx')
    # A folded directory remains traversable: only target ancestry is expanded.
    send('ggjh')
    assert 'command-menu.tsx' not in '\n'.join(line[:34] for line in screen.text().splitlines())
    send('nn')
    current('use-keyboard-navigation.ts')
    # The same shortcut navigates files from Changes until a search is active.
    send('N\t')
    send('/useState\r')
    send('n')
    assert '2/3' in screen.text()
    current('command-menu.tsx')
    send('N')
    assert '1/3' in screen.text()
    send('\x1b')
    send('n')
    current('use-keyboard-navigation.ts')
    send('N')
    current('command-menu.tsx')
    send('\x1b[Z/filter-commands.ts\r')
    send('n')
    current('filter-commands.ts')
    send('nN')
    current('filter-commands.ts')
    send('q')
    proc.wait(timeout=3)
    assert proc.returncode == 0
    print('PASS terminal: n/N skips tree chrome, opens folded ancestry, respects file filter and preserves diff-search matches')
finally:
    if proc.poll() is None:
        proc.kill()
        proc.wait()
    os.close(master)
