-- Spike 005 seed schema for the real SQLite path. Mock connectors model
-- their own dialect-styled schemas separately so the trait question
-- ("does schema introspection look uniform from the toolkit's POV?")
-- gets a real answer.

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
