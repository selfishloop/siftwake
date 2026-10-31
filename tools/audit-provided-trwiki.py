"""Audit USER-PROVIDED corpus; metadata/oracle only, no new document bodies."""
import hashlib
import json
import struct
import time
import zlib
from pathlib import Path

SOURCE=Path('D:/frunner-corpus/trwiki-100k')
DEST=Path(__file__).resolve().parent.parent/'docs/benchmarks'
P='trwiki-provided-20261028-v1'
MAP=str.maketrans({'I':'i','İ':'i','ı':'i','Ş':'s','ş':'s','Ğ':'g','ğ':'g','Ç':'c','ç':'c','Ö':'o','ö':'o','Ü':'u','ü':'u','Σ':'σ'})
def fold(s):return s.translate(MAP).lower()
def grams(s,n):return {s[i:i+n] for i in range(len(s)-n+1)}

def main():
    audit=DEST/f'{P}-audit.json';catalog=DEST/f'{P}-catalog.json'
    assert not audit.exists() and not catalog.exists(),'Evidence exists; do not replay'
    manifest=json.loads((SOURCE/'manifest.json').read_text(encoding='utf-8-sig'))
    provided=json.loads((SOURCE/'queries.json').read_text(encoding='utf-8-sig'))
    cases=[];groups={}
    for group,rows in provided.items():
        if not isinstance(rows,list):continue
        for row in rows:
            text=row if isinstance(row,str) else row['text']
            if text in groups:continue
            groups[text]=group
            cases.append(dict(text=text,offset=0,limit=50,base=len(cases)))
    assert len(cases)<=64
    terms={c['text']:fold(c['text']).split() for c in cases}
    gramsets={n:{q:set(g for t in ts for g in grams(t,min(n,len(t)))) if all(len(t)>=3 for t in ts) else set() for q,ts in terms.items()} for n in (3,4)}
    # For4-grams, words shorter than4 retain their existing trigram fallback.
    sizes=[];entries=[];bodies=set();headers=hashlib.sha256();text_hash=hashlib.sha256();query_counts={}
    stats={n:{'exact':{q:0 for q in terms},'candidates3':{q:0 for q in terms},'candidates4':{q:0 for q in terms}} for n in (10000,50000,100000)}
    at=time.monotonic();fields=set();link_count=0
    with (SOURCE/'corpus.payload-bench').open('rb') as f:
        h=f.read(20);headers.update(h)
        magic,version,count,crc=struct.unpack('<8sIII',h)
        assert magic==b'PLOADB01' and version==1 and count==100000 and zlib.crc32(h[:16])==crc
        for ident in range(count):
            h=f.read(16);headers.update(h);length,crc,record_id,hcrc=struct.unpack('<IIII',h)
            assert record_id==ident and zlib.crc32(h[:12])==hcrc and 0<length<4*1024*1024
            offset=f.tell();raw=f.read(length);headers.update(raw)
            assert len(raw)==length and zlib.crc32(raw)==crc
            stored=json.loads(raw);doc=stored['document'];body=doc['text'].encode('utf-8')
            assert doc['key']==dict(scope='shared',collection='texts',id=f'{ident:010}'),'Preserve original key/link identities; do not remap'
            assert all(k['scope']=='shared' for k in doc['links'])
            assert len(body)<=2*1024*1024
            bodies.add(hashlib.sha256(body).digest());sizes.append(len(body));fields.update(doc['fields']);link_count+=len(doc['links'])
            text_hash.update(doc['title'].encode('utf-8'));text_hash.update(b'\0');text_hash.update(body);text_hash.update(b'\0')
            title=fold(doc['title']);text=fold(doc['text']);mask=0
            exact={q:all(t in title or t in text for t in ts) for q,ts in terms.items()}
            candidates={n:{q:all(g in title or g in text for g in gs) for q,gs in byq.items()} for n,byq in gramsets.items()}
            for case in cases:
                if exact[case['text']]:mask|=1<<case['base']
            entries.append(dict(offset=offset,size=length,crc=crc,mask=mask,text_bytes=len(body)))
            for prefix,state in stats.items():
                if ident>=prefix:continue
                for q in terms:
                    state['exact'][q]+=exact[q]
                    state['candidates3'][q]+=candidates[3][q]
                    state['candidates4'][q]+=candidates[4][q]
            if (ident+1)%10000==0:print(json.dumps(dict(audited=ident+1,elapsed_s=time.monotonic()-at)),flush=True)
        assert f.read(1)==b''
    assert headers.hexdigest().upper()==manifest['payload_sha256']
    assert len(bodies)==manifest['unique_bodies']==100000
    catalog.write_text(json.dumps(dict(entries=entries,cases=cases),ensure_ascii=False,separators=(',',':')),encoding='utf-8')
    for n in stats:
        xs=sorted(sizes[:n]);stats[n]['text_bytes']=sum(xs);stats[n]['min']=xs[0];stats[n]['p50']=xs[(n-1)//2];stats[n]['p95']=xs[(95*n+99)//100-1];stats[n]['max']=xs[-1]
    result=dict(source=str(SOURCE/'corpus.payload-bench'),payload_sha256=headers.hexdigest().upper(),documents=count,unique_text_bodies=len(bodies),
        title_text_stream_sha256=text_hash.hexdigest().upper(),catalog=str(catalog),catalog_sha256=hashlib.sha256(catalog.read_bytes()).hexdigest().upper(),
        query_groups=groups,prefixes=stats,fields_count=len(fields),links=link_count,
        body_changes=False,new_corpus_generated=False,source_read_only=True,
        original_key_template_verified=True,
        preliminary_attempt='First audit finished CRC/SHA/unique-body checks but refused nonempty links before writing metadata. The no-links guard was too strict: all original keys match the existing numeric template, so no key/link remapping is needed. Re-audit explicitly verifies every original key and link scope. No DB was reset or populated during the refused preflight.',
        notes='Provided Turkish Wikipedia articles, not a code/document/file mix; DF counts use normalized substring AND, not provided word DF. Long originals are NOT truncated. All reads local; no body content logged.',
        elapsed_s=time.monotonic()-at)
    audit.write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(dict(documents=count,unique_bodies=len(bodies),queries=len(cases),sha256=result['payload_sha256'],oracle_catalog_written=True)),flush=True)

if __name__=='__main__':main()
