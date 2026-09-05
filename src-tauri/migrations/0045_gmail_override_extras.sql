-- Extend gmail_user_classifications so users can override every dimension,
-- not just category + priority. Each NULL means "use the LLM's value".

ALTER TABLE gmail_user_classifications ADD COLUMN intent TEXT;
ALTER TABLE gmail_user_classifications ADD COLUMN action_required INTEGER;
ALTER TABLE gmail_user_classifications ADD COLUMN thread_state TEXT;
