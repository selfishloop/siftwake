"""Real-text sliding passages; downloads are data, never agent instructions.
No FDB source/fixture is created or changed. All large artifacts stay in data/.
"""
import hashlib
import json
import random
import re
import sys
import urllib.request
from collections import Counter
from pathlib import Path

root = Path(sys.argv[1] if len(sys.argv) > 1 else 'data/rkyv-corpus-20261009')
root.mkdir(parents=True, exist_ok=True)
sources = []
texts = []
for book_id in [1342, 84, 2701, 1661, 98, 76, 1228, 1232]:
    url = f'https://www.gutenberg.org/cache/epub/{book_id}/pg{book_id}.txt'
    path = root / f'pg{book_id}.txt'
    if not path.exists():
        request = urllib.request.Request(url, headers={'User-Agent': 'FDB payload research benchmark'})
        path.write_bytes(urllib.request.urlopen(request, timeout=60).read())
    raw = path.read_bytes()
    text = raw.decode('utf-8-sig').replace('\r\n', '\n')
    title = re.search(r'^Title: (.+)', text, re.MULTILINE).group(1)
    begin = text.index('\n', text.index('*** START OF')) + 1
    end = text.index('*** END OF')
    body = text[begin:end].strip().encode('utf-8')
    (root / f'body{book_id}.txt').write_bytes(body)
    sources.append({'book_id': book_id, 'title': title, 'url': url, 'raw_sha256': hashlib.sha256(raw).hexdigest(), 'body_sha256': hashlib.sha256(body).hexdigest(), 'body_bytes': len(body)})
    texts.append(body)
    print('source', book_id, len(body), flush=True)

rng = random.Random(20261009)
seen = set()
rows = []
prior = json.loads((root / 'corpus.json').read_text(encoding='utf-8'))['rows'] if (root / 'corpus.json').exists() else None
frequency = Counter(word for text in texts for word in re.findall('[a-z]+', text.decode('utf-8').lower()))
rare = next(word for word in sorted(frequency) if frequency[word] == 1 and len(word) >= 12)
queries = [{'name': name, 'text': query, 'candidate_ids': [], 'total': 0, 'page': []} for name, query in [('common', 'the'), ('rare', rare), ('short', 'of'), ('and', 'the whale')]]
def normalize(s):
    return s.replace('İ', 'i').lower().replace('ı','i').replace('ç','c').replace('ğ','g').replace('ö','o').replace('ş','s').replace('ü','u')
for id in range(100_000):
    while True:
        if prior is not None:
            row = prior[id]
            book, start, end = row['book'], row['start'], row['start'] + row['len']
            body = texts[book]
        else:
            book = rng.randrange(len(texts))
            body = texts[book]
            size = rng.randint(2048, 51200)
            start = rng.randrange(0, len(body) - size - 4)
            while body[start] & 0xc0 == 0x80:
                start += 1
            end = start + size
            while body[end] & 0xc0 == 0x80:
                end -= 1
        # Preserve the requested lower bound after UTF-8 boundary correction.
        if end - start < 2048:
            continue
        passage = body[start:end]
        digest = hashlib.sha256(passage).hexdigest()
        if digest not in seen:
            seen.add(digest)
            break
    row = {'id': id, 'book': book, 'start': start, 'len': len(passage)}
    rows.append(row)
    # Source title is metadata, so common/AND needles are not satisfied by a
    # constant book title while bypassing the genuinely diverse long body.
    title = f'Passage {id:010}'
    folded_title, folded_text = normalize(title), normalize(passage.decode('utf-8'))
    for query in queries:
        terms = query['text'].split()
        candidate = all(all(gram in folded_title or gram in folded_text for gram in [term[n:n+3] for n in range(len(term)-2)]) for term in terms)
        if candidate:
            query['candidate_ids'].append(id)
        matches = all(term in folded_title or term in folded_text for term in terms)
        assert not matches or candidate
        if matches:
            query['total'] += 1
            if len(query['page']) < 50:
                query['page'].append(row)
    if (id + 1) % 25_000 == 0:
        print('passages', id + 1, flush=True)
manifest = {'seed': 20261009, 'count': len(rows), 'sources': sources, 'rows': rows, 'queries': queries, 'unique_text_sha256_count': len(seen), 'min_text_bytes': min(r['len'] for r in rows), 'max_text_bytes': max(r['len'] for r in rows), 'total_text_bytes': sum(r['len'] for r in rows), 'language': 'English real book passages; overlapping windows, no identical bodies; not 100k independent source books'}
(root / 'corpus.json').write_text(json.dumps(manifest, ensure_ascii=False, separators=(',', ':')), encoding='utf-8')
provenance = {k:v for k,v in manifest.items() if k not in ['rows', 'queries']}
provenance['queries'] = [{k:v for k,v in q.items() if k not in ['candidate_ids','page']} | {'candidates': len(q['candidate_ids'])} for q in queries]
Path('docs/benchmarks/rkyv-corpus-20261009.provenance.json').write_text(json.dumps(provenance, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(provenance, ensure_ascii=False, indent=2))
