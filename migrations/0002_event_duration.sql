-- How long each inbound callback took to process, for the provider health page.
alter table inbound_events add column duration_ms integer;
create index inbound_events_source_received_idx on inbound_events (source, received_at desc);
