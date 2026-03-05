#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
UPSTREAM_DIR="${1:-/tmp/verus-upstream}"
GUIDE_DIR="$UPSTREAM_DIR/source/docs/guide"
OUT_FILE="$ROOT_DIR/rust/proofs/verus/reference/verus-guide-print.md"

if [[ ! -f "$GUIDE_DIR/src/SUMMARY.md" ]]; then
  echo "missing Verus guide sources at: $GUIDE_DIR" >&2
  echo "hint: git clone --depth 1 https://github.com/verus-lang/verus.git /tmp/verus-upstream" >&2
  exit 2
fi

ROOT_DIR="$ROOT_DIR" UPSTREAM_DIR="$UPSTREAM_DIR" OUT_FILE="$OUT_FILE" python3 - <<'PY'
import os
import re
import subprocess
from pathlib import Path

root = Path(os.environ['ROOT_DIR'])
upstream = Path(os.environ['UPSTREAM_DIR'])
out = Path(os.environ['OUT_FILE'])
guide = upstream / 'source' / 'docs' / 'guide'
summary = guide / 'src' / 'SUMMARY.md'

text = summary.read_text(encoding='utf-8')
pat = re.compile(r'\[([^\]]+)\]\(([^)]+)\)')
seen = set()
chapters = []
for line in text.splitlines():
    for _title, rel in pat.findall(line):
        rel = rel.strip()
        if not rel or rel.startswith('http') or rel.startswith('#') or not rel.endswith('.md'):
            continue
        path = (guide / 'src' / rel).resolve()
        if not path.exists():
            continue
        key = str(path)
        if key in seen:
            continue
        seen.add(key)
        chapters.append((rel, path))

commit = subprocess.check_output(['git','-C',str(upstream),'rev-parse','HEAD'], text=True).strip()
out.parent.mkdir(parents=True, exist_ok=True)

with out.open('w', encoding='utf-8') as f:
    f.write('# Verus Guide (Printable Snapshot)\n\n')
    f.write('Generated from upstream Verus guide sources.\n\n')
    f.write('- Upstream repo: `https://github.com/verus-lang/verus`\n')
    f.write(f'- Upstream commit: `{commit}`\n')
    f.write('- Source summary: `source/docs/guide/src/SUMMARY.md`\n\n')
    f.write('---\n')
    for rel, path in chapters:
        f.write(f"\n\n<!-- source: {rel} -->\n\n")
        f.write(path.read_text(encoding='utf-8').rstrip())
        f.write('\n')

print(f'wrote {out} with {len(chapters)} chapters')
PY
