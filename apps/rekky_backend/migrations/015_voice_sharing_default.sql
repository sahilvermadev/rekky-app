-- Preserve the audience promised to work already accepted by the server.
ALTER TABLE voice_uploads ADD COLUMN desired_visibility text NOT NULL DEFAULT 'private'
  CHECK (desired_visibility IN ('friends','private'));
ALTER TABLE voice_transcription_jobs ADD COLUMN desired_visibility text NOT NULL DEFAULT 'private'
  CHECK (desired_visibility IN ('friends','private'));
-- New work snapshots Friends; legacy captures/items are deliberately untouched.
ALTER TABLE voice_uploads ALTER COLUMN desired_visibility SET DEFAULT 'friends';
ALTER TABLE voice_transcription_jobs ALTER COLUMN desired_visibility SET DEFAULT 'friends';
