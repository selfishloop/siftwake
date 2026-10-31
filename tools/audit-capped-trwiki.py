"""Audit supplied capped corpus; metadata/oracle only, private/ never accessed."""
import hashlib
import argparse
import importlib.util
import json
import struct
import time
import zlib
from pathlib import Path

R=Path(__file__).resolve().parent.parent;L=R/'docs/benchmarks';P='trwiki-cap200k-20261029-v1'
NEW=Path('D:/frunner-corpus/trwiki-100k-cap200k');OLD=Path('D:/frunner-corpus/trwiki-100k')
EXPECTED='8510408E6F943B3DB0CF5EC816739D4CF1FB52A3922EEAC6C01F281ABAC38467'
spec=importlib.util.spec_from_file_location('audit',R/'tools/audit-provided-trwiki.py')
base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)

def cases(path):
    result=[];groups={}
    for group,rows in json.loads(path.read_text(encoding='utf-8-sig')).items():
        if not isinstance(rows,list):continue
        for row in rows:
            q=row if isinstance(row,str) else row['text']
            assert q not in groups
            result.append(dict(text=q,offset=0,limit=50,base=len(result)));groups[q]=group
    assert len(result)==41
    return result,groups

def unique_object(pairs):
    result={}
    for k,v in pairs:
        assert k not in result,'Duplicate JSON key'
        result[k]=v
    return result

class Reader:
    def __init__(self,path):
        self.file=path.open('rb');self.sha=hashlib.sha256()
        h=self.take(20);magic,version,n,crc=struct.unpack('<8sIII',h)
        assert magic==b'PLOADB01' and version==1 and n==100000 and zlib.crc32(h[:16])==crc
    def take(self,n):
        value=self.file.read(n);assert len(value)==n;self.sha.update(value);return value
    def row(self,i):
        h=self.take(16);n,crc,ident,hcrc=struct.unpack('<IIII',h)
        assert ident==i and zlib.crc32(h[:12])==hcrc and 0<n<4*1024*1024
        offset=self.file.tell();raw=self.take(n);assert zlib.crc32(raw)==crc
        value=json.loads(raw,object_pairs_hook=unique_object)
        assert set(value)=={'document','version'} and value['version']==1
        doc=value['document'];assert set(doc)=={'key','title','text','fields','links'}
        assert doc['key']==dict(scope='shared',collection='texts',id=f'{i:010}')
        assert isinstance(doc['fields'],dict) and isinstance(doc['links'],list)
        assert all(set(k)=={'scope','collection','id'} and k['scope']=='shared' for k in doc['links'])
        return doc,dict(offset=offset,size=n,crc=crc)
    def finish(self):
        assert self.file.read(1)==b'';self.file.close();return self.sha.hexdigest().upper()

def main():
    global P,EXPECTED
    args=argparse.ArgumentParser()
    args.add_argument('--expected-sha',default=EXPECTED)
    args.add_argument('--prefix',default=P)
    options=args.parse_args();P=options.prefix;EXPECTED=options.expected_sha.upper()
    output=L/f'{P}-audit.json';catalog=L/f'{P}-catalog.json'
    assert not output.exists() and not catalog.exists(),'Preserve existing audit'
    manifest=json.loads((NEW/'manifest.json').read_text(encoding='utf-8-sig'))
    assert manifest['payload_sha256']==EXPECTED and manifest['documents']==100000
    new_cases,new_groups=cases(NEW/'queries.json');old_cases,old_groups=cases(OLD/'queries.json')
    queries=list(dict.fromkeys([c['text'] for c in new_cases+old_cases]));terms={q:base.fold(q).split() for q in queries}
    stats={n:dict(exact={q:0 for q in queries}) for n in [10000,50000,100000]}
    entries=[];old_masks=[];sizes=[];declared_sizes=[];bodies=set();new_titles=set();old_titles=set();links=0;bytes_equal=0
    identity=dict(keys=0,titles=0,text_prefixes=0,links=0,fields_except_bytes=0)
    text_sha=hashlib.sha256();started=time.monotonic();longest=[];changed_text_ids=[]
    new=Reader(NEW/'corpus.payload-bench');old=Reader(OLD/'corpus.payload-bench')
    for i in range(100000):
        doc,entry=new.row(i);previous,_=old.row(i)
        body=doc['text'].encode('utf-8');size=len(body);assert size<=200000
        digest=hashlib.sha256(body).digest();bodies.add(digest);sizes.append(size)
        title_hash=lambda d:hashlib.sha256(d['title'].encode('utf-8')).digest()
        new_titles.add(title_hash(doc));old_titles.add(title_hash(previous))
        identity['keys']+=doc['key']==previous['key'];identity['titles']+=doc['title']==previous['title']
        identity['text_prefixes']+=previous['text'].startswith(doc['text']);identity['links']+=doc['links']==previous['links']
        if doc['text']!=previous['text']:changed_text_ids.append(i)
        identity['fields_except_bytes']+={k:v for k,v in doc['fields'].items() if k!='bytes'}=={k:v for k,v in previous['fields'].items() if k!='bytes'}
        declared_sizes.append(doc['fields'].get('bytes'))
        bytes_equal+=doc['fields'].get('bytes')==size;links+=len(doc['links'])
        text_sha.update(doc['title'].encode('utf-8'));text_sha.update(b'\0');text_sha.update(body);text_sha.update(b'\0')
        title=base.fold(doc['title']);text=base.fold(doc['text'])
        exact={q:all(t in title or t in text for t in ts) for q,ts in terms.items()}
        entry.update(mask=sum(1<<c['base'] for c in new_cases if exact[c['text']]),text_bytes=size)
        entries.append(entry);old_masks.append(sum(1<<c['base'] for c in old_cases if exact[c['text']]))
        for n,state in stats.items():
            if i<n:
                for q in queries:state['exact'][q]+=exact[q]
        longest.append((size,i));longest.sort(reverse=True);del longest[20:]
        if (i+1)%10000==0:print(json.dumps(dict(audited=i+1,elapsed_s=time.monotonic()-started)),flush=True)
    sha=new.finish();old_sha=old.finish()
    assert sha==EXPECTED and len(bodies)==manifest.get('unique_bodies',100000)==100000
    assert sum(sizes)==manifest['text_bytes'] and max(sizes)==manifest['max_text_bytes']<=200000
    reference=json.loads((L/'trwiki-provided-20261028-v1-audit.json').read_text(encoding='utf-8'))
    assert old_sha==reference['payload_sha256']
    catalog.write_text(json.dumps(dict(entries=entries,cases=new_cases),ensure_ascii=False,separators=(',',':')),encoding='utf-8')
    (L/f'{P}-old-query-masks.json').write_text(json.dumps(dict(cases=old_cases,masks=old_masks),ensure_ascii=False,separators=(',',':')),encoding='utf-8')
    for n,state in stats.items():
        xs=sorted(sizes[:n]);state.update(text_bytes=sum(xs),min=xs[0],p50=xs[(n-1)//2],p95=xs[(95*n+99)//100-1],max=xs[-1])
    result=dict(source=str(NEW/'corpus.payload-bench'),payload_sha256=sha,documents=100000,unique_text_bodies=len(bodies),
        catalog=str(catalog),catalog_sha256=hashlib.sha256(catalog.read_bytes()).hexdigest().upper(),query_cases=new_cases,query_groups=new_groups,
        old_query_cases=old_cases,old_query_groups=old_groups,distinct_queries=len(queries),query_overlap=len(set(new_groups)&set(old_groups)),
        prefixes=stats,original_key_template_verified=True,links=links,bytes_field_equals_text=bytes_equal,
        longest=[dict(id=i,text_bytes=n,same_length_documents=sizes.count(n),bytes_field=declared_sizes[i],same_bytes_field_documents=declared_sizes.count(declared_sizes[i])) for n,i in longest],
        identity_at_same_ordinal=identity,title_hash_intersection=len(old_titles&new_titles),old_reference_sha256=old_sha,
        same_order_trim_only_verified=identity['titles']==identity['text_prefixes']==100000,
        changed_text_ids=changed_text_ids,
        title_text_stream_sha256=text_sha.hexdigest().upper(),source_read_only=True,private_read=False,
        corpus_generated=False,body_changes_by_agent=False,elapsed_s=time.monotonic()-started,
        original_request_sha256='8510408E6F943B3DB0CF5EC816739D4CF1FB52A3922EEAC6C01F281ABAC38467',
        measurement_authorization='Must resolve the source replacement with the user before ingest')
    output.write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps({k:result[k] for k in ['documents','unique_text_bodies','payload_sha256','query_overlap','identity_at_same_ordinal','same_order_trim_only_verified','longest']},ensure_ascii=False),flush=True)

if __name__=='__main__':main()
