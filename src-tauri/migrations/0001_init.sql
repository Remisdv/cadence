-- Cache local de Cadence. Une base par dépôt.
-- Chaque table GitHub garde la charge JSON brute : les vues lisent des colonnes
-- dénormalisées, la charge sert à reconstruire une colonne sans refaire d'appel.
-- Pas de clé étrangère entre ressources : chacune se synchronise à son rythme
-- (une issue peut arriver avant son milestone).

CREATE TABLE milestones (
    number      INTEGER PRIMARY KEY,
    title       TEXT    NOT NULL,          -- semver : 0.1.2
    state       TEXT    NOT NULL,          -- open | closed
    due_on      TEXT,                      -- NULL = palier
    etag        TEXT,
    updated_at  TEXT    NOT NULL,
    raw         TEXT    NOT NULL
);

CREATE TABLE issues (
    number           INTEGER PRIMARY KEY,
    title            TEXT    NOT NULL,
    state            TEXT    NOT NULL,
    milestone_number INTEGER,
    labels           TEXT    NOT NULL DEFAULT '[]',  -- JSON
    assignee         TEXT,
    etag             TEXT,
    updated_at       TEXT    NOT NULL,
    raw              TEXT    NOT NULL
);
CREATE INDEX issues_milestone ON issues (milestone_number);

CREATE TABLE pull_requests (
    number       INTEGER PRIMARY KEY,
    title        TEXT    NOT NULL,
    state        TEXT    NOT NULL,   -- draft | open | merged | closed
    head         TEXT    NOT NULL,
    base         TEXT    NOT NULL,
    linked_issue INTEGER,
    etag         TEXT,
    updated_at   TEXT    NOT NULL,
    raw          TEXT    NOT NULL
);

CREATE TABLE reviews (
    id          INTEGER PRIMARY KEY,
    pr_number   INTEGER NOT NULL,
    reviewer    TEXT    NOT NULL,
    state       TEXT    NOT NULL,
    updated_at  TEXT    NOT NULL,
    raw         TEXT    NOT NULL
);
CREATE INDEX reviews_pr ON reviews (pr_number);

CREATE TABLE labels (
    name        TEXT PRIMARY KEY,
    color       TEXT NOT NULL,
    etag        TEXT,
    updated_at  TEXT NOT NULL,
    raw         TEXT NOT NULL
);

CREATE TABLE releases (
    id          INTEGER PRIMARY KEY,
    tag         TEXT NOT NULL UNIQUE,
    draft       INTEGER NOT NULL DEFAULT 0,
    etag        TEXT,
    updated_at  TEXT NOT NULL,
    raw         TEXT NOT NULL
);

-- Curseur de sync incrémentale par ressource (since = dernier updated_at vu).
CREATE TABLE sync_state (
    resource       TEXT PRIMARY KEY,
    last_synced_at TEXT,
    last_seen      TEXT,
    etag           TEXT
);

-- Journal des exécutions d'agent.
CREATE TABLE agent_runs (
    id          TEXT PRIMARY KEY,
    agent       TEXT NOT NULL,          -- cto | ceo | dev | deploy
    state       TEXT NOT NULL,
    input       TEXT NOT NULL,          -- JSON
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    actions     TEXT NOT NULL DEFAULT '[]'  -- JSON : actions proposées et leur sort
);

-- File d'écriture hors-ligne, rejouée au retour du réseau.
-- La clôture de version et la publication d'une release n'y passent jamais.
CREATE TABLE pending_actions (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    idempotency_key TEXT NOT NULL UNIQUE,
    kind            TEXT NOT NULL,
    payload         TEXT NOT NULL,
    expected_state  TEXT,                -- état attendu, pour détecter un conflit
    created_at      TEXT NOT NULL,
    attempts        INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT
);
