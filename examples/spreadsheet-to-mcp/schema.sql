-- Typed schema for the Acme sales spreadsheet (customers + quotes sheets).
--
-- This file has TWO jobs:
--   1. `sqlite3 company.db < schema.sql` creates the typed tables BEFORE the CSV
--      import, so columns get real affinity (INTEGER ids, REAL amounts) instead
--      of everything landing as TEXT.
--   2. Passed as `--schema` to pmcp-sql-server, it becomes the Code Mode schema
--      resource — the description the model reads to generate correct SQL.
--
-- Explicit types are what turn a "mechanical wrapper" into a well-designed
-- server: REAL amounts compare and SUM correctly; ISO-8601 date TEXT sorts
-- and ranges correctly.

CREATE TABLE customers (
    id           INTEGER PRIMARY KEY,
    company_name TEXT    NOT NULL,
    contact_name TEXT    NOT NULL,
    email        TEXT    NOT NULL,
    country      TEXT    NOT NULL,           -- ISO 3166 alpha-2
    created_at   TEXT    NOT NULL            -- ISO-8601 date (YYYY-MM-DD)
);

CREATE TABLE quotes (
    id          INTEGER PRIMARY KEY,
    customer_id INTEGER NOT NULL REFERENCES customers(id),
    amount      REAL    NOT NULL,
    currency    TEXT    NOT NULL DEFAULT 'USD',
    status      TEXT    NOT NULL,            -- draft | sent | accepted | rejected | expired
    issued_date TEXT    NOT NULL,            -- ISO-8601 date
    valid_until TEXT                         -- ISO-8601 date, nullable
);

CREATE INDEX idx_quotes_customer ON quotes(customer_id);
CREATE INDEX idx_quotes_status   ON quotes(status);
