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
