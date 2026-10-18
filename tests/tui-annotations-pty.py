"""Offline annotations smoke through real keys, bridge drafts, and AI consumption."""
import errno, fcntl, os, pathlib, select, struct, subprocess, tempfile, termios, time
from vt_screen import Screen

root = pathlib.Path(__file__).resolve().parent.parent
binary = os.environ.get('LAZYREVIEW_BINARY', str(root / 'tui/target/release/lazyreview'))
with tempfile.TemporaryDirectory(prefix='lazyreview-annotations-') as temporary:
    env = {**os.environ, 'TERM': 'xterm-256color', 'XDG_CONFIG_HOME': temporary + '/config', 'XDG_STATE_HOME': temporary + '/state'}
    master, slave = os.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 45, 150, 0, 0))
    process = subprocess.Popen([binary, '--demo', '--theme', 'blue'], stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True)
    os.close(slave)
    screen = Screen(150, 45)
    seen = bytearray()

    def read(seconds=.15):
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            if select.select([master], [], [], .02)[0]:
                try:
                    data = os.read(master, 65536)
                except OSError as error:
                    if error.errno == errno.EIO:
                        return
                    raise
                if not data:
                    return
                seen.extend(data)
                screen.feed(data)

    def send(keys):
        os.write(master, keys.encode())
        read()

    def wait(text, timeout=4):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            read(.04)
            if text in screen.text():
                return
        raise AssertionError('Missing ' + text + '\n' + screen.text())

    def capture(name):
        folder = os.environ.get('LAZYREVIEW_ANNOTATION_PTY_CAPTURE')
        if folder:
            pathlib.Path(folder).mkdir(parents=True, exist_ok=True)
            pathlib.Path(folder, name + '.txt').write_text(screen.text())

    try:
        wait('Changes · inline')
        send('/')
        send('command-menu.tsx\r')
        wait('Files · 1/18')
        send('\t')
        send('/const results\r')
        send('\x1b')
        send('vjt')
        wait('Private note')
        send('Explain this range\r')
        wait('◇1 ')
        assert any('const results' in ''.join(row) and '╭' in ''.join(row) for row in screen.rows)
        send('vjc')
        wait('Comment')
        send('Check the boundary\r')
        wait('●1◇1 ')
        capture('inline')
        send('s')
        wait('side by side')
        assert any('const results' in ''.join(row) and '╭' in ''.join(row) for row in screen.rows)
        capture('split')
        send('gc')
        send('i')
        send('Explain the queued question\r')
        wait('SAMPLE EXPLANATION')
        assert '◇1 ' not in screen.text(), screen.text()
        wait('●1 ')
        capture('consumed')
        # Return from companion to the changes pane and queue a new note.
        send('\t\t')
        send('t')
        wait('Private note')
        send('Temporary follow-up\r')
        wait('●1◇1 ')
        send('gc')
        send('d')
        send('\x1b')
        assert '◇1 ' not in screen.text(), screen.text()
        wait('●1 ')
        capture('removed')
        send('q')
        process.wait(timeout=4)
        assert process.returncode == 0
        assert b'panicked' not in seen and b'\x1b[?1049l' in seen
        print('PASS annotations PTY: authored draft + queued AI counters, inline/split brackets, consumption and removal, GitHub count retained')
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
