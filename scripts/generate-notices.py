import json,hashlib,re
from pathlib import Path
import argparse
parser=argparse.ArgumentParser(description='Generate release attribution from locked dependency metadata and installed license files.')
parser.add_argument('--metadata',default='.work/cargo-metadata.json')
args=parser.parse_args()
meta=json.loads(Path(args.metadata).read_text(encoding='utf-8-sig'))
entries=[];texts={};missing=[]
def add(name,version,license,source,folder,authors=()):
 folder=Path(folder)
 candidates=[]
 for item in folder.iterdir():
  if item.is_file() and re.match(r'(?i)^(licen[cs]e|copying|notice)([._-]|$)',item.name): candidates.append(item)
  elif item.is_dir() and item.name.lower() in ['licenses','license','licences']:
   candidates.extend(p for p in item.iterdir() if p.is_file())
 refs=[]
 for file in sorted(candidates):
  if file.stat().st_size>200000: continue
  try: text=file.read_text(encoding='utf-8').strip()
  except (UnicodeError,OSError):continue
  digest=hashlib.sha256(text.encode()).hexdigest()[:16]
  texts[digest]=text;refs.append(digest)
 if not refs:missing.append(name)
 entries.append((name,version,license,source,refs,authors))
for pkg in sorted(meta['packages'],key=lambda p:(p['name'],p['version'])):
 if pkg['name']=='reponiq':continue
 add(pkg['name'],pkg['version'],pkg['license'] or 'See source',f"https://crates.io/crates/{pkg['name']}/{pkg['version']}",Path(pkg['manifest_path']).parent,pkg.get('authors',[]))
lock=json.loads(Path('package-lock.json').read_text(encoding='utf-8'))
# All installed JS packages are conservatively attributed, including build tools.
for key,pkg in sorted(lock['packages'].items()):
 if not key:continue
 folder=Path(key)
 if not (folder/'package.json').exists():continue
 data=json.loads((folder/'package.json').read_text(encoding='utf-8'))
 add(data['name'],data['version'],data.get('license','See source'),pkg.get('resolved',data.get('homepage','See package source')),folder)
version=json.loads(Path('package.json').read_text(encoding='utf-8'))['version']
lines=[f'Reponiq v{version} — third-party notices','', 'Reponiq source and original artwork: MIT; see LICENSE.', 'Dependencies remain under their respective licenses. Dependency sources are unmodified.', 'This inventory conservatively includes build-time and target-specific packages; not all are linked into each installer.', 'MPL-2.0 dependency source files are available at the exact versioned crate source links below.', 'Operating-system components, Git, WebView2 and system WebKitGTK are separate platform prerequisites under their vendors’ terms.', '','DEPENDENCIES','']
for name,version,license,source,refs,authors in entries:
 lines.extend([f'{name} {version}',f'License: {license}',f'Source: {source}'])
 if authors:lines.append('Authors: '+'; '.join(authors))
 lines.extend(['License texts: '+(', '.join(refs) if refs else 'See versioned upstream source.'),''])
lines+=['PLATFORM MARKS', '', Path('assets/platforms/README.md').read_text(encoding='utf-8'), '']
for asset_license in ['GITEA-LICENSE.txt', 'CC0-LICENSE.txt']:
 lines += [asset_license, Path('assets/platforms', asset_license).read_text(encoding='utf-8'), '']
lines+=['LICENSE TEXTS','']
for digest,text in sorted(texts.items()):lines.extend(['----- '+digest+' -----',text,''])
Path('THIRD_PARTY_NOTICES.txt').write_text('\n'.join(lines),encoding='utf-8')
Path('.work').mkdir(exist_ok=True)
Path('.work/license-inventory.json').write_text(json.dumps({'packages':len(entries),'texts':len(texts),'missing_license_files':missing},indent=2))
print(json.dumps({'packages':len(entries),'unique_license_texts':len(texts),'missing_license_files':missing}))
