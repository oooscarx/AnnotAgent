from pathlib import Path
from urllib.parse import unquote
import re,json,hashlib
R=Path(__file__).resolve().parents[3];P=R/'docs/product/readme'
missing=[]
for f in [R/'README.md',R/'README.en.md',R/'docs/product/README.md',*P.glob('*.md')]:
 for ref in re.findall(r'\]\(([^)]+)\)|(?:src|srcset)="([^"]+)"',f.read_text()):
  v=next(x for x in ref if x).split('#')[0]
  if not v or '://' in v:continue
  if not (f.parent/unquote(v)).exists():missing.append({'file':str(f.relative_to(R)),'target':v})
manifest=json.loads((P/'ASSET_MANIFEST.json').read_text())
bad=[a['file'] for a in manifest['assets'] if hashlib.sha256((R/a['file']).read_bytes()).hexdigest()!=a['sha256']]
loaded=sum((P/x).stat().st_size for x in ['brand-light.svg','flow-light.svg','workspace.png'])
result={'missing_local_links':missing,'hash_mismatches':bad,'default_light_image_bytes':loaded,'under_3MB':loaded<3000000,'root_license_text_present':any((R/x).exists() for x in ['LICENSE','LICENSE.md','LICENSE.txt'])}
(P/'checks.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2));raise SystemExit(bool(missing or bad))
