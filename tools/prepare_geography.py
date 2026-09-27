#!/usr/bin/env python3
"""Build a GeoNames gazetteer from downloaded dump files (CC BY 4.0).
Usage: python3 tools/prepare_geography.py work/geography
Required: cities500.txt, countryInfo.txt, admin1CodesASCII.txt, admin2Codes.txt,
hierarchy.txt. Optional country dumps (e.g. IN.txt) extend locality coverage.
No recommendation or user data is read. Output is areas.jsonl plus a manifest.
"""
import hashlib, json, re, sys, unicodedata
from pathlib import Path

root = Path(sys.argv[1])
def fold(s):
    s = ''.join(c for c in unicodedata.normalize('NFKD', s.lower()) if not unicodedata.combining(c))
    return ' '.join(re.findall(r'[^\W_]+', s))
rows = {}
admin = {}
countries = {}
files = [root/'cities500.txt', *sorted(root.glob('[A-Z][A-Z].txt'))]
for line in (root/'countryInfo.txt').read_text().splitlines():
    if not line or line.startswith('#'): continue
    c = line.split('\t')
    countries[c[0]] = c[16]
    rows[c[16]] = dict(id=c[16], name=c[4], aliases=[c[4]], country=c[0], feature='PCLI', population=int(c[7] or 0), parents=[])
for filename, feature in [('admin1CodesASCII.txt','ADM1'), ('admin2Codes.txt','ADM2')]:
    for line in (root/filename).read_text().splitlines():
        c=line.split('\t'); code=c[0]; admin[code]=c[3]
        parent=admin.get(code.rsplit('.',1)[0], countries.get(code.split('.')[0]))
        rows[c[3]]=dict(id=c[3],name=c[2] or c[1],aliases=c[1:3],country=code.split('.')[0],feature=feature,population=0,parents=[parent] if parent else [])
# Administrative IDs from detailed country dumps also link settlements correctly.
for file in files:
    for line in file.read_text().splitlines():
        c=line.split('\t')
        if c[6]=='A' and c[7] in ['ADM1','ADM2','ADM3','ADM4']:
            level=int(c[7][-1]); admin['.'.join([c[8],*c[10:10+level]])]=c[0]
for file in files:
    for line in file.read_text().splitlines():
        c=line.split('\t')
        # Exclude hotels/businesses/stations: this catalog identifies geography.
        # Fort/cantonment names are geographic destinations, never business identities.
        if not (c[6]=='P' and c[7] not in ['PPLH','PPLQ','PPLW'] or c[7] in ['ADM1','ADM2','ADM3','ADM4','PCLI','FT']): continue
        level=int(c[7][-1])-1 if c[7].startswith('ADM') else 4
        parents=[admin['.'.join([c[8],*c[10:10+i]])] for i in range(1,level+1) if '.'.join([c[8],*c[10:10+i]]) in admin]
        if c[8] in countries: parents.append(countries[c[8]])
        rows[c[0]]=dict(id=c[0],name=c[2] or c[1],aliases=[c[1],c[2],*c[3].split(',')],country=c[8],feature=c[7],population=int(c[14] or 0),parents=parents)
for line in (root/'hierarchy.txt').read_text().splitlines():
    p,c,*_=line.split('\t')
    if p in rows and c in rows and p!=c: rows[c]['parents'].append(p)
def ancestors(id):
    seen={id}
    pending=list(rows[id]['parents'])
    while pending:
        parent=pending.pop()
        if parent in rows and parent not in seen:
            seen.add(parent)
            pending.extend(rows[parent]['parents'])
    return seen-{id}
out=root/'areas.jsonl'
with out.open('w') as f:
    for id,r in rows.items():
        parents=ancestors(id)
        ordered=sorted(parents,key=lambda p: (0 if rows[p]['feature'].startswith('PPL') else 1 if rows[p]['feature']=='ADM1' else 2,p))
        region=next((rows[p]['name'] for p in ordered if rows[p]['feature'].startswith('PPL')),None) or next((rows[p]['name'] for p in ordered if rows[p]['feature']=='ADM1'),None)
        label=r['name'] if r['feature'].startswith('PPLA') or r['feature']=='PPLC' or not region or fold(region)==fold(r['name']) else r['name']+', '+region
        data=dict(id='geonames:'+id,name=r['name'],label=label,country=r['country'],feature=r['feature'],population=r['population'],aliases=sorted({fold(a) for a in r['aliases'] if len(fold(a))>=3}),ancestors=['geonames:'+p for p in ordered],hierarchy=[dict(id='geonames:'+p,name=rows[p]['name'],kind=rows[p]['feature']) for p in ordered])
        f.write(json.dumps(data,ensure_ascii=False)+'\n')
manifest=dict(source='https://download.geonames.org/export/dump/',license='https://creativecommons.org/licenses/by/4.0/',attribution='GeoNames',area_count=len(rows),files={p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in [*files, root/'hierarchy.txt',root/'countryInfo.txt',root/'admin1CodesASCII.txt',root/'admin2Codes.txt']},output_sha256=hashlib.sha256(out.read_bytes()).hexdigest())
(root/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print('Prepared',len(rows),'geographic records in',out)
