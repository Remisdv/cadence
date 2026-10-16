-- Cadence 2 : le moteur d'agents est supprimé (actions → skills des plugins).
-- Son journal et la file d'écriture hors-ligne qui lui servait disparaissent.
DROP TABLE IF EXISTS agent_runs;
DROP TABLE IF EXISTS pending_actions;
