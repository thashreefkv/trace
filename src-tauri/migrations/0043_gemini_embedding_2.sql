-- Rebuild embeddings after moving from retired text-embedding-004 to
-- gemini-embedding-2 at 3072 dimensions.

DELETE FROM entity_embeddings
 WHERE model != 'gemini-embedding-2' OR dim != 3072;

DELETE FROM memory_embeddings
 WHERE model != 'gemini-embedding-2' OR dim != 3072;

DELETE FROM file_embeddings
 WHERE model != 'gemini-embedding-2' OR dim != 3072;

UPDATE entity_embeddings_pending
   SET attempts = 0,
       last_error = NULL
 WHERE last_error LIKE '%text-embedding-004%';
