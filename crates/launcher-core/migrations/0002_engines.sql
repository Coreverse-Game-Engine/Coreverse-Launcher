-- Engine versions installed on this machine. The files live in
-- `<app data>/engines/<version>`; this table is the index over them.
CREATE TABLE installed_engines (
                                   version      TEXT PRIMARY KEY NOT NULL,
                                   status       TEXT NOT NULL,
                                   os           TEXT NOT NULL,
                                   architecture TEXT NOT NULL,
                                   sha256       TEXT NOT NULL,
                                   size_bytes   INTEGER NOT NULL,
                                   installed_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;
