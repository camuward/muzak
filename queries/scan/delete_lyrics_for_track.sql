DELETE FROM lyrics
WHERE track_id IN (
    SELECT id FROM track WHERE location = $1
);
