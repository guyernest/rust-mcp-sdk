-- Spike 004 sample schema: a tiny employee directory.
-- The toolkit's user provides this file alongside their config.toml.
-- The toolkit treats the schema as data (not as a thing-to-introspect-and-
-- auto-tool-ify) — it's used (a) to materialize an in-memory DB for the
-- demo, and (b) to populate the code-mode bootstrap prompt body so the
-- LLM knows the long-tail surface.

CREATE TABLE employees (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    department  TEXT NOT NULL,
    salary      INTEGER NOT NULL
);

INSERT INTO employees (id, name, department, salary) VALUES
    (1, 'Ada Lovelace',     'Research',     185000),
    (2, 'Grace Hopper',     'Research',     192000),
    (3, 'Alan Turing',      'Research',     210000),
    (4, 'Margaret Hamilton','Engineering',  175000),
    (5, 'Linus Torvalds',   'Engineering',  165000),
    (6, 'Donald Knuth',     'Research',     220000);
