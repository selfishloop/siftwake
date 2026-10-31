"""Read existing bodies; estimate hypothetical filters, never build/store one."""
import collections
import hashlib
import json
import time
import zlib
from pathlib import Path

ROOT=Path(__file__).resolve().parent.parent
LOGS=ROOT/'docs/benchmarks'
P='block2-prefilter-estimates-50k-20261028-v1'
TRANSLATE=str.maketrans({'I':'i','İ':'i','ı':'i','Ş':'s','ş':'s','Ğ':'g','ğ':'g','Ç':'c','ç':'c','Ö':'o','ö':'o','Ü':'u','ü':'u'})
QUERIES=['harpoon','presumption','darcy','transubstantiation','holmes elementary']

def folded(s):
    return s.translate(TRANSLATE).lower()

def windows(s,n):
    # Same character-window/Unicode-whitespace rule as index::grams.
    for token in s.split():
        for i in range(len(token)-n+1):
            yield token[i:i+n]

def main():
    dest=LOGS/f'{P}.json'
    if dest.exists():
        raise RuntimeError('Do not overwrite evidence')
    modelpath=Path(json.loads((LOGS/'block2-scale-20261028-v1-slot-fdb.json').read_text(encoding='utf-8'))['state'])
    model=json.loads(modelpath.read_text(encoding='utf-8'))
    entries=json.loads((LOGS/'block2-scale-20261028-v1-corpus-index.json').read_text(encoding='utf-8'))['entries']
    grams={n:{q:set(g for word in q.split() for g in windows(word,n)) for q in QUERIES} for n in (3,4,5)}
    counts={n:{q:0 for q in QUERIES} for n in grams}
    exact={q:0 for q in QUERIES}
    sample=collections.Counter()
    vocabulary={3:set(),4:set()}
    at=time.monotonic()
    with (ROOT/'data/rkyv-corpus-20261009/json.payload-bench').open('rb') as f:
        for ordinal,(key,row) in enumerate(sorted(model['rows'].items(),key=lambda x:int(x[0]))):
            e=entries[row['body']]
            f.seek(e['offset']); raw=f.read(e['size'])
            assert zlib.crc32(raw)==e['crc']
            doc=json.loads(raw)['document']
            title=folded(doc['title'])
            body=folded(doc['text'].encode('utf-8')[:51140].decode('utf-8',errors='ignore')+f" zqv{int(key):010}r{row['revision']:010}zq")
            for q in QUERIES:
                exact[q]+=all(w in title or w in body for w in q.split())
                for n,byq in grams.items():
                    counts[n][q]+=all(g in title or g in body for g in byq[q])
            if ordinal%50==0:
                sample['documents']+=1
                for n in (3,4):
                    unique=set()
                    for field in (title,body):
                        previous={}
                        # Offsets measured in folded Unicode scalars; field IDs
                        # and all list headers still need a future codec design.
                        for pos in range(len(field)-n+1):
                            g=field[pos:pos+n]
                            if any(c.isspace() for c in g):
                                continue
                            unique.add(g)
                            delta=pos-previous.get(g,0)
                            previous[g]=pos
                            sample[f'occurrences{n}']+=1
                            sample[f'position_delta_varint_bytes{n}']+=max(1,(delta.bit_length()+6)//7)
                    sample[f'df{n}']+=len(unique)
                    vocabulary[n].update(unique)
            if (ordinal+1)%10000==0:
                print(json.dumps(dict(documents=ordinal+1,elapsed_s=time.monotonic()-at)),flush=True)
    result=dict(corpus='existing50k, current sequence104 ledger; no new bodies, no DB/filter written',
        model_sha256=hashlib.sha256(modelpath.read_bytes()).hexdigest(),documents=len(model['rows']),
        candidate_counts=counts,exact_counts=exact,
        sample=dict(sample),sample_rule='every50th live row in numeric key order,1000docs',
        sample_vocabulary={n:len(v) for n,v in vocabulary.items()},
        estimates_only=True,elapsed_s=time.monotonic()-at,
        limits='sample vocabulary is a lower bound; no ingest runtime or new codec measured; positions/list headers/field IDs and dictionary bytes excluded')
    dest.write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
    print(json.dumps(result,ensure_ascii=False),flush=True)

if __name__=='__main__':
    main()
