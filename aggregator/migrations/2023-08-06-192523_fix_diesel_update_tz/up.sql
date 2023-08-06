CREATE OR REPLACE FUNCTION diesel_set_updated_at() RETURNS trigger AS $$
BEGIN
    IF (
        NEW IS DISTINCT FROM OLD AND
        NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at
    ) THEN
        NEW.updated_at := CURRENT_TIMESTAMP AT TIME ZONE 'UTC';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
