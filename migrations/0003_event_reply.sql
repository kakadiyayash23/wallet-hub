-- Store what we answered, so the operator drawer can show request and reply side by side.
alter table inbound_events add column reply_status integer;
alter table inbound_events add column reply_body jsonb;
