-- Idempotent demo schema (H2 / Codex): CREATE TABLE IF NOT EXISTS + INSERT OR
-- IGNORE so a second run against a persisted demo.db succeeds and leaves exactly
-- the seeded rows. Seed values are free of embedded ';'.
CREATE TABLE IF NOT EXISTS books (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    author TEXT NOT NULL
);

INSERT OR IGNORE INTO books (id, title, author) VALUES (1, 'The Rust Programming Language', 'Steve Klabnik and Carol Nichols');
INSERT OR IGNORE INTO books (id, title, author) VALUES (2, 'Programming Rust', 'Jim Blandy and Jason Orendorff');
INSERT OR IGNORE INTO books (id, title, author) VALUES (3, 'Rust for Rustaceans', 'Jon Gjengset');
