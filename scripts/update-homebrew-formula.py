from pathlib import Path
import os
import re
import sys


def replace_sha(text: str, asset: str, sha: str) -> str:
    pattern = (
        rf'(url "https://github.com/kshitizz36/Provn/releases/download/v#\{{version\}}/'
        rf'{re.escape(asset)}"\n\s*sha256 )"[^"]+"'
    )
    updated, count = re.subn(pattern, rf'\1"{sha}"', text, count=1)
    if count != 1:
        raise SystemExit(f"Could not update sha256 for {asset}")
    return updated


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: update-homebrew-formula.py <formula-path>")

    formula_path = Path(sys.argv[1])
    text = formula_path.read_text()

    version = os.environ["VERSION"]
    text, count = re.subn(r'version "[^"]+"', f'version "{version}"', text, count=1)
    if count != 1:
        raise SystemExit("Could not update formula version")

    replacements = {
        "provn-aarch64-apple-darwin.tar.gz": os.environ["SHA_AARCH64_MACOS"],
        "provn-x86_64-apple-darwin.tar.gz": os.environ["SHA_X86_64_MACOS"],
        "provn-aarch64-linux.tar.gz": os.environ["SHA_AARCH64_LINUX"],
        "provn-x86_64-linux.tar.gz": os.environ["SHA_X86_64_LINUX"],
    }

    for asset, sha in replacements.items():
        text = replace_sha(text, asset, sha)

    formula_path.write_text(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
