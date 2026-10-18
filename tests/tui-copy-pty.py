"""Transcript selection through real terminal events; isolated clipboard stub only.

--baseline reproduces the old blocked composer and coalesced-motion cases.
Every case preserves neighboring panes and never touches the system clipboard.
"""
import errno, fcntl, os, pathlib, select, struct, subprocess, tempfile, termios, time, sys
from vt_screen import Screen

root = pathlib.Path(__file__).resolve().parent.parent
binary = os.environ.get('LAZYREVIEW_BINARY', str(root / 'tui/target/release/lazyreview'))
baseline = '--baseline' in sys.argv

with tempfile.TemporaryDirectory(prefix='rw-copy-') as directory:
    directory = pathlib.Path(directory)
    stub, copied = directory / 'pbcopy', directory / 'clipboard.txt'
    stub.write_text('#!/usr/bin/env python3\nimport sys,pathlib,os\npathlib.Path(os.environ["LAZYREVIEW_CLIP_TEST"]).write_text(sys.stdin.read())\n')
    stub.chmod(0o700)

    def run(style):
        master, slave = os.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 45, 150, 0, 0))
        proc = subprocess.Popen(
            [binary, '--preview-ui', 'ai', '--companion-style', style],
            stdin=slave, stdout=slave, stderr=slave,
            env={**os.environ, 'PATH': str(directory) + os.pathsep + os.environ['PATH'],
                 'LAZYREVIEW_CLIP_TEST': str(copied), 'TERM': 'xterm-256color'}, start_new_session=True)
        os.close(slave)
        screen = Screen()

        def read(seconds=.15):
            end = time.monotonic() + seconds
            while time.monotonic() < end:
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

        def mouse(code, x, y, end='M'):
            send(f'\x1b[<{code};{x+1};{y+1}{end}')

        def drag(label='Why reset', motion=True, reverse=False, expect=True):
            copied.unlink(missing_ok=True)
            lines = screen.text().splitlines()
            row = next(i for i, line in enumerate(lines) if label in line)
            column = lines[row].index(label)
            start, end = (column + len(label) - 1, column) if reverse else (column, column + len(label) - 1)
            mouse(0, start, row)
            if motion:
                mouse(32, end, row)
            mouse(0, end, row, 'm')
            read(.25)
            if expect:
                assert copied.exists(), f'{style}: no clipboard write\n{screen.text()}'
                assert copied.read_text() == label, repr(copied.read_text())
                assert 'Copied selection' in screen.text(), screen.text()
            else:
                assert not copied.exists(), 'Expected baseline failure to remain observable'

        try:
            read(.6)
            drag()
            # Mouse selection must not depend on the pane that had keyboard focus.
            send('\t')
            drag()
            drag(motion=False, reverse=True, expect=not baseline)
            send('iDraft stays')
            assert 'Draft stays' in screen.text()
            drag(expect=not baseline)
            assert 'Draft stays' in screen.text(), 'Selecting transcript destroyed the draft'
            send('\x1b')
            if baseline:
                print('BASELINE: plain drag works; composer-open and coalesced-motion selections produce no clipboard writes')
            else:
                # Keyboard-only selection remains available when terminal/multiplexer
                # modifiers intercept mouse gestures. The pane body excludes borders.
                copied.unlink(missing_ok=True)
                send('\x1b')
                send('ggv$y')
                read(.25)
                assert copied.exists() and copied.read_text().strip()
                assert not any(c in copied.read_text() for c in '│╭╮╰╯')
                send('\x1b')
                # A new geometry must refresh selection coordinates and source rows.
                fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack('HHHH', 45, 180, 0, 0))
                screen = Screen(180, 45)
                read(.4)
                send('gg')
                drag()
                print(f'PASS {style}: drag, unfocused pane, coalesced reverse drag, composer/draft, keyboard copy, resized viewport')
            send('q')
            read(.2)
            proc.wait(timeout=3)
            assert proc.returncode == 0
        finally:
            if proc.poll() is None:
                proc.kill()
                proc.wait()
            os.close(master)

    for style in ['paper'] if baseline else ['paper', 'ledger', 'studio']:
        run(style)
