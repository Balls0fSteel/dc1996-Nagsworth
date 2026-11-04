-- Break exercises (docs/EXERCISES.md): the routine suggested with a raised
-- break nag or a started break, as JSON. Older rows read as NULL.

ALTER TABLE nag_events ADD COLUMN routine TEXT;
