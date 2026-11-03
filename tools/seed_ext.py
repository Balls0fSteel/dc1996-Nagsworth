"""Copies a built extension and prepends a bootstrap that pairs with a code.

Used by tools/ext_e2e.ps1.

Test harness only: a headless browser cannot type the code into the
options page, so the copy calls the same /pair endpoint itself.
Usage: python seed_ext.py <src dir> <dst dir> <code> <port>
"""
import shutil, sys
from pathlib import Path

src, dst, code, port = Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3], sys.argv[4]
shutil.rmtree(dst, ignore_errors=True)
shutil.copytree(src, dst)
bg = dst / "background.js"
boot = (
    "(globalThis.browser ?? chrome).storage.local.get('token').then(s => s.token ? null :"
    f" fetch('http://127.0.0.1:{port}/v1/pair', {{method: 'POST', headers: {{'Content-Type': 'application/json'}},"
    f" body: JSON.stringify({{code: '{code}'}})}}).then(r => r.json())"
    ".then(j => (globalThis.browser ?? chrome).storage.local.set({token: j.token})));\n"
)
bg.write_text(boot + bg.read_text(encoding="utf-8"), encoding="utf-8")
print(dst)
