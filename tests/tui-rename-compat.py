"""Renamed launchers and legacy configuration; no CLI, model or network calls."""
import os, pathlib, subprocess, tempfile

root = pathlib.Path(__file__).resolve().parent.parent
binary = os.environ.get('LAZYREVIEW_BINARY', str(root / 'lazyreview'))
with tempfile.TemporaryDirectory(prefix='lazyreview-compat-') as temp:
    config = pathlib.Path(temp) / 'config'
    legacy = config / 'review-workbench/config.yml'
    legacy.parent.mkdir(parents=True)
    source = 'gui:\n  themePreset: blue\n  syntaxTheme: lunar\n  companionStyle: studio\nai:\n  enabled: false\n  profile: existing-account\n  autoPrepare: false\n'
    legacy.write_text(source)
    env = {**os.environ, 'XDG_CONFIG_HOME': str(config), 'XDG_STATE_HOME': str(pathlib.Path(temp) / 'state')}
    result = subprocess.run([binary, '--setup-ai'], env=env, capture_output=True, text=True, check=True)
    saved = (config / 'lazyreview/config.yml').read_text()
    for setting in ['themePreset: blue', 'syntaxTheme: lunar', 'companionStyle: studio', 'profile: existing-account', 'enabled: true', 'autoPrepare: false']:
        assert setting in saved, setting
    assert legacy.read_text() == source
    assert 'lazyreview/config.yml' in result.stdout
    assert subprocess.check_output([str(root / 'review'), '--help'], env=env, text=True).startswith('LazyReview\n')
    print('PASS rename: new launcher, legacy alias, old settings retained, new config written without moving existing files')
