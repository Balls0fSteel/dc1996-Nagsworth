-- Hands-on time: the part of active time with recent keyboard, mouse or
-- controller input (docs/HANDS_ON_TIME.md). Older rows read as 0.

ALTER TABLE segments ADD COLUMN input_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE daily_app_totals ADD COLUMN input_ms INTEGER NOT NULL DEFAULT 0;
ALTER TABLE daily_kind_totals ADD COLUMN input_ms INTEGER NOT NULL DEFAULT 0;
