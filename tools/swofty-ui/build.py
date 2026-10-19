"""Build the one-file Swofty UI executable with PyInstaller.

    python -m pip install pyinstaller==6.22.3
    python tools/swofty-ui/build.py            # writes dist/swofty-ui/swofty-ui[.exe]
"""
import argparse
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
TOOLS = os.path.dirname(HERE)
REPO = os.path.dirname(TOOLS)


def main():
    ap = argparse.ArgumentParser(description='Build the Swofty UI executable.')
    ap.add_argument('--dist', default=os.path.join(REPO, 'dist', 'swofty-ui'))
    ap.add_argument('--work', default=os.path.join(REPO, 'target', 'swofty-ui-build'))
    args = ap.parse_args()
    if sys.version_info < (3, 14):
        raise SystemExit('build Swofty UI with Python 3.14 or newer: the executable needs compression.zstd')
    import PyInstaller.__main__

    opts = [os.path.join(HERE, 'serve.py'),
            '--name', 'swofty-ui',
            '--onefile', '--console', '--clean', '--noconfirm',
            '--paths', HERE, '--paths', TOOLS,
            '--add-data', f'{os.path.join(HERE, "static")}{os.pathsep}static',
            '--hidden-import', 'compression.zstd',
            '--hidden-import', 'selftest',
            '--exclude-module', 'tkinter',
            '--distpath', os.path.abspath(args.dist),
            '--workpath', os.path.abspath(args.work),
            '--specpath', os.path.abspath(args.work)]
    if sys.platform == 'win32':
        opts += ['--icon', os.path.join(HERE, 'res', 'icon.ico')]
    PyInstaller.__main__.run(opts)


if __name__ == '__main__':
    main()
