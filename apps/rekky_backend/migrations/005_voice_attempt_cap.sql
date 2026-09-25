ALTER TABLE voice_transcription_jobs
  ADD COLUMN attempts integer NOT NULL DEFAULT 1 CHECK (attempts BETWEEN 1 AND 3);
