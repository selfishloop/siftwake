"""Standalone SQLite comparison; never imported by the Rust FDB engine."""
import argparse
import json
import sqlite3
import time
from pathlib import Path


def report(**values):
    print(json.dumps(values, ensure_ascii=False), flush=True)


def samples(values):
    values = sorted(values)
    return dict(p50_ms=values[len(values)//2], p95_ms=values[min(len(values)-1,len(values)*95//100)], max_ms=values[-1])


def fold(text):
    return text.translate(str.maketrans('İıIŞşĞğÇçÖöÜü', 'iiiSsGgCcOoUu')).lower()


def connect(path):
    db = sqlite3.connect(path)
    db.execute('PRAGMA journal_mode=WAL')
    db.execute('PRAGMA synchronous=FULL')
    db.execute('PRAGMA cache_size=-2048')
    db.execute('PRAGMA mmap_size=0')
    db.execute('PRAGMA temp_store=FILE')
    return db


def document(number, body):
    return dict(key=dict(scope='shared', collection='contents', id=f'{number:010}'),
                title=f'rapor-{number:010}', text=body,
                fields=dict(day=number%30,source='synthetic'),links=[])


def payload(doc, version):
    return json.dumps(dict(document=doc,version=version),ensure_ascii=False,separators=(',',':'))


def timed_query(db, kind, expected, repeats, text=None, offset=0):
    if kind == 'text':
        source = 'search CROSS JOIN docs d ON d.rowid=search.rowid'
        condition = "search.folded LIKE ? AND d.scope=? AND d.collection=?"
        params = ('%'+fold(text)+'%', 'shared','contents')
    else:
        source = 'docs d'
        condition = 'd.scope=? AND d.collection=? AND d.source=?'
        params = ('shared','contents','synthetic')
        if kind == 'range':
            condition += ' AND d.day BETWEEN 2 AND 5'
    times=[]
    for _ in range(repeats):
        start=time.perf_counter()
        # One read transaction keeps count and the page on the same snapshot.
        db.execute('BEGIN')
        total=db.execute(f'SELECT count(*) FROM {source} WHERE {condition}',params).fetchone()[0]
        rows=db.execute(f'SELECT d.payload FROM {source} WHERE {condition} ORDER BY d.id LIMIT 50 OFFSET ?',params+(offset,)).fetchall()
        hits=[json.loads(r[0]) for r in rows]
        db.commit()
        times.append((time.perf_counter()-start)*1000)
        assert total==expected,(kind,total,expected)
        assert len(hits)==min(50,max(0,total-offset))
    return samples(times),hits


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('directory')
    parser.add_argument('--count',type=int,default=1_000_000)
    parser.add_argument('--text-bytes',type=int,default=256)
    args=parser.parse_args()
    root=Path(args.directory)
    root.mkdir(parents=True,exist_ok=False)
    path=root/'sqlite.db'
    count=args.count
    assert count>=1000
    body='Şirketin teknoloji yatırımları ve bilanço raporu. '*(args.text_bytes//20+1)
    while len(body.encode())>args.text_bytes:
        body=body[:-1]
    db=connect(path)
    db.executescript('''
        CREATE TABLE docs(scope TEXT NOT NULL, collection TEXT NOT NULL, id TEXT NOT NULL,
            source TEXT NOT NULL, day INTEGER NOT NULL, payload TEXT NOT NULL, folded TEXT NOT NULL,
            UNIQUE(scope,collection,id));
        CREATE INDEX source_idx ON docs(scope,collection,source,id);
        CREATE INDEX range_idx ON docs(scope,collection,source,day,id);
        CREATE VIRTUAL TABLE search USING fts5(folded, content='docs', content_rowid='rowid',
            tokenize='trigram', detail=none, columnsize=0);
        CREATE TRIGGER docs_ai AFTER INSERT ON docs BEGIN
            INSERT INTO search(rowid,folded) VALUES(new.rowid,new.folded);
        END;
        CREATE TRIGGER docs_ad AFTER DELETE ON docs BEGIN
            INSERT INTO search(search,rowid,folded) VALUES('delete',old.rowid,old.folded);
        END;
        CREATE TRIGGER docs_au AFTER UPDATE ON docs BEGIN
            INSERT INTO search(search,rowid,folded) VALUES('delete',old.rowid,old.folded);
            INSERT INTO search(rowid,folded) VALUES(new.rowid,new.folded);
        END;
    ''')
    report(phase='ingest',sqlite_version=sqlite3.sqlite_version,count=count,text_bytes=len(body.encode()),
           journal_mode=db.execute('PRAGMA journal_mode').fetchone()[0], synchronous=db.execute('PRAGMA synchronous').fetchone()[0],
           cache_kib=2048,mmap_bytes=0,fts='external-content trigram detail=none columnsize=0')
    batches=[]
    started=time.perf_counter()
    for first in range(0,count,500):
        rows=[]
        for n in range(first,min(count,first+500)):
            doc=document(n,body)
            rows.append(('shared','contents',doc['key']['id'],'synthetic',n%30,payload(doc,first//500+1),fold(doc['title']+'\n'+body)))
        start=time.perf_counter()
        with db:
            db.executemany('INSERT INTO docs(scope,collection,id,source,day,payload,folded) VALUES(?,?,?,?,?,?,?)',rows)
        batches.append((time.perf_counter()-start)*1000)
        if (first+500)%50_000==0:
            report(phase='ingest_progress',records=first+500,elapsed_s=time.perf_counter()-started)
    ingest_ms=(time.perf_counter()-started)*1000
    report(phase='maintenance',ingest_ms=ingest_ms,batches=samples(batches))
    start=time.perf_counter()
    with db:
        db.execute("INSERT INTO search(search) VALUES('optimize')")
    checkpoint=db.execute('PRAGMA wal_checkpoint(TRUNCATE)').fetchone()
    maintenance_ms=(time.perf_counter()-start)*1000
    db.close()
    start=time.perf_counter()
    db=connect(path)
    assert db.execute("SELECT payload FROM docs WHERE scope='shared' AND collection='contents' AND id=?",(f'{count//2:010}',)).fetchone()
    reopen_ms=(time.perf_counter()-start)*1000
    report(phase='queries',reopen_ms=reopen_ms,maintenance_ms=maintenance_ms,checkpoint=checkpoint)
    narrow,_=timed_query(db,'text',1,30,text=f'rapor-{count//2:010}')
    broad,_=timed_query(db,'exact',count,5)
    _,page=timed_query(db,'exact',count,1,offset=count-50)
    assert page[0]['document']['key']['id']==f'{count-50:010}'
    ranged,_=timed_query(db,'range',sum(1 for n in range(count) if 2<=n%30<=5),5)
    with db:
        for n in range(100):
            number=n*(count//100)
            if n%2==0:
                db.execute("DELETE FROM docs WHERE scope='shared' AND collection='contents' AND id=?",(f'{number:010}',))
            else:
                doc=document(number,'changed sentinel')
                db.execute("UPDATE docs SET payload=?,folded=? WHERE scope='shared' AND collection='contents' AND id=?",
                           (payload(doc,count//500+1),fold(doc['title']+'\n'+doc['text']),f'{number:010}'))
    db.execute('PRAGMA wal_checkpoint(TRUNCATE)')
    db.close()
    start=time.perf_counter()
    db=connect(path)
    assert db.execute('SELECT count(*) FROM docs').fetchone()[0]==count-50
    changed_reopen_ms=(time.perf_counter()-start)*1000
    for n in range(100):
        row=db.execute("SELECT payload FROM docs WHERE scope='shared' AND collection='contents' AND id=?",(f'{n*(count//100):010}',)).fetchone()
        if n%2==0:
            assert row is None
        else:
            assert json.loads(row[0])['document']['text']=='changed sentinel'
    report(phase='integrity')
    start=time.perf_counter()
    assert db.execute('PRAGMA integrity_check').fetchall()==[('ok',)]
    with db:
        db.execute("INSERT INTO search(search,rank) VALUES('integrity-check',1)")
    integrity_ms=(time.perf_counter()-start)*1000
    report(phase='result',count=count,text_bytes=len(body.encode()),logical_text_bytes=count*len(body.encode()),
           ingest_ms=ingest_ms,batches=samples(batches),maintenance_ms=maintenance_ms,reopen_ms=reopen_ms,
           changed_reopen_ms=changed_reopen_ms,integrity_ms=integrity_ms,narrow=narrow,broad=broad,range=ranged,
           documents=count-50,verified_updates=50,verified_deletes=50,
           query_plans={name:db.execute('EXPLAIN QUERY PLAN '+sql,params).fetchall() for name,sql,params in [
             ('text','SELECT count(*) FROM search CROSS JOIN docs d ON d.rowid=search.rowid WHERE search.folded LIKE ? AND d.scope=? AND d.collection=?',('%rapor-0000500000%','shared','contents')),
             ('range','SELECT count(*) FROM docs WHERE scope=? AND collection=? AND source=? AND day BETWEEN 2 AND 5',('shared','contents','synthetic'))]})
    db.close()


if __name__=='__main__':
    main()
