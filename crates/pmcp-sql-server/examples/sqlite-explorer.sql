-- sqlite-explorer.sql — demo schema + seed for the config-driven SQLite MCP server.
--
-- Dual purpose:
--   1. Seed a demo database:  sqlite3 /tmp/pmcp-sqlite-explorer.db < examples/sqlite-explorer.sql
--   2. Served verbatim as the --schema code-mode resource (so agents authoring
--      ad-hoc SQL via Code Mode see the table shape).
--
-- Idempotent (CREATE TABLE IF NOT EXISTS + INSERT OR IGNORE) so re-seeding is safe
-- and leaves exactly the rows below.

CREATE TABLE IF NOT EXISTS books (
    id     INTEGER PRIMARY KEY,
    title  TEXT NOT NULL,
    author TEXT NOT NULL,
    year   INTEGER NOT NULL,
    genre  TEXT NOT NULL
);

INSERT OR IGNORE INTO books (id, title, author, year, genre) VALUES (1, 'The Rust Programming Language', 'Steve Klabnik and Carol Nichols', 2018, 'Programming');
INSERT OR IGNORE INTO books (id, title, author, year, genre) VALUES (2, 'Programming Rust', 'Jim Blandy and Jason Orendorff', 2021, 'Programming');
INSERT OR IGNORE INTO books (id, title, author, year, genre) VALUES (3, 'Rust for Rustaceans', 'Jon Gjengset', 2021, 'Programming');
INSERT OR IGNORE INTO books (id, title, author, year, genre) VALUES (4, 'The Pragmatic Programmer', 'Andrew Hunt and David Thomas', 1999, 'Software');
INSERT OR IGNORE INTO books (id, title, author, year, genre) VALUES (5, 'Designing Data-Intensive Applications', 'Martin Kleppmann', 2017, 'Systems');
