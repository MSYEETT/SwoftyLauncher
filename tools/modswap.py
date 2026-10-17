import hashlib, json, os, shutil, subprocess, sys
from pathlib import Path

GAME = Path(os.environ.get('F1_25_DIR') or sys.exit('set F1_25_DIR to the game folder'))
ROOT = Path(__file__).resolve().parent.parent
PRISTINE = ROOT / 'work/pristine'
NATIVE_MANIFEST = GAME / 'swofty/.modswap-native.json'
ORIGINAL_MD5 = {
    '2025_asset_groups/ui_package/flow_frontend.erp': '37b6945216aadae5e4cc431dba9567a0',
    '2025_asset_groups/ui_package/flow_options.erp': 'c50bc12363d395250a99b0b01279f164',
    '2025_asset_groups/ui_package/persistent.erp': 'b421cc3c3aa37c50100f774e9d69c5b7',
    '2025_asset_groups/ui_package/databases.erp': 'bc91d17d3aa69264938a91bded446728',
    '2025_asset_groups/ui_package/screens/fe_home.erp': '463966f77392796953924adec85774da',
    '2025_asset_groups/ui_package/flow_persistent.erp': '33beffcbacc15848e7cb54f743c875e9',
    '2025_asset_groups/ui_package/screens/persistent.erp': 'bc2155641f3b501b21e33c8495689c87',
    'game_package/gamedatabases/gamedatabases.2025.bin': 'b0c8376256f69a79517db45d03a0e653',
    '2025_asset_groups/environment_package/tracks/bahrain/wep/bahrain_f1_26.erp': 'ceab3c172cff93dc0e61d5e6c03d0d63',
    '2025_asset_groups/environment_package/tracks/bahrain/wep/bahrain_f1_26_myteam.erp': '1c894db53dac0f6ce111b8398f1d3464',
    '2025_asset_groups/environment_package/tracks/bahrain/wep/bahrain_common.erp': 'bf371087a56323c9a69673f517e8ca77',
}


def md5(path):
    h = hashlib.md5()
    with open(path, 'rb') as f:
        for chunk in iter(lambda: f.read(1 << 20), b''):
            h.update(chunk)
    return h.hexdigest()


def copy_verified(src, dst):
    shutil.copy2(src, dst)
    if md5(src) != md5(dst):
        sys.exit(f'copy {src} -> {dst} failed verification')


def check_pristine():
    for rel, want in ORIGINAL_MD5.items():
        if md5(PRISTINE / rel) != want:
            sys.exit(f'pristine copy of {rel} is corrupt')


def restore_native():
    if not NATIVE_MANIFEST.exists():
        return
    installed = json.loads(NATIVE_MANIFEST.read_text())
    for rel, want in installed['files'].items():
        dst = GAME / rel
        if not dst.exists():
            continue
        if md5(dst) != want:
            print('left in place (changed since install):', rel)
            continue
        dst.unlink()
        print('removed', rel)
    NATIVE_MANIFEST.unlink()
    for rel in sorted(installed['dirs'], key=lambda d: -d.count('/')):
        d = GAME / rel
        if d.is_dir() and not any(d.iterdir()):
            d.rmdir()
            print('removed empty dir', rel)


def install_native(dist):
    base = Path(dist).resolve()
    files = sorted(p.relative_to(base).as_posix() for p in base.rglob('*') if p.is_file())
    dirs = sorted({p.relative_to(base).as_posix() for p in base.rglob('*') if p.is_dir()})
    if not files or set(files) & set(ORIGINAL_MD5):
        sys.exit(f'unexpected files in {base}: {files}')
    for rel in files:
        if (GAME / rel).exists():
            sys.exit(f'{rel} already exists in the game dir and was not installed by modswap; not touching it')
    installed = {'files': {}, 'dirs': dirs}
    for rel in dirs:
        (GAME / rel).mkdir(parents=True, exist_ok=True)
    NATIVE_MANIFEST.write_text(json.dumps(installed, indent=1))
    for rel in files:
        copy_verified(base / rel, GAME / rel)
        installed['files'][rel] = md5(GAME / rel)
        NATIVE_MANIFEST.write_text(json.dumps(installed, indent=1))
        print('installed', rel)


def restore():
    check_pristine()
    for rel, want in ORIGINAL_MD5.items():
        if md5(GAME / rel) != want:
            copy_verified(PRISTINE / rel, GAME / rel)
            print('restored', rel)
    restore_native()


def install(variants, native=None):
    if not variants and not native:
        sys.exit('usage: modswap.py install [<variant-dir>...] [--native <dist-dir>]')
    plan = {}
    for variant in variants:
        base = Path(variant).resolve()
        files = sorted(p.relative_to(base).as_posix() for p in base.rglob('*') if p.suffix in ('.erp', '.bin'))
        if not files or not set(files) <= set(ORIGINAL_MD5):
            sys.exit(f'unexpected files in {base}: {files}')
        for rel in files:
            if rel in plan:
                sys.exit(f'{rel} is in more than one variant')
            plan[rel] = base / rel
    restore()
    for rel, src in plan.items():
        copy_verified(src, GAME / rel)
        print('installed', rel)
    if native:
        install_native(native)


def status():
    for rel, want in ORIGINAL_MD5.items():
        got = md5(GAME / rel)
        print(f'{"original" if got == want else got:32} {rel}')
    if NATIVE_MANIFEST.exists():
        for rel, want in json.loads(NATIVE_MANIFEST.read_text())['files'].items():
            dst = GAME / rel
            state = 'missing' if not dst.exists() else 'native' if md5(dst) == want else 'changed'
            print(f'{state:32} {rel}')
    else:
        print(f'{"no native files":32} (version.dll proxy not installed)')


if __name__ == '__main__':
    cmd = sys.argv[1] if len(sys.argv) > 1 else 'status'
    if cmd != 'status' and subprocess.run(['pgrep', '-x', 'F1_25.exe'], capture_output=True).returncode == 0:
        sys.exit('F1 25 is running; close it first')
    if cmd == 'install':
        args = sys.argv[2:]
        native = None
        if '--native' in args:
            i = args.index('--native')
            if i + 1 >= len(args):
                sys.exit('--native needs a dist dir')
            native = args[i + 1]
            del args[i:i + 2]
        install(args, native)
    elif cmd == 'restore':
        restore()
    else:
        status()
