-- Players and accounts -------------------------------------------------------

create table players (
    id              uuid primary key,
    external_token  text not null unique,           -- what providers see as username / user_token
    display_name    text not null,
    locale          text not null default 'en',
    currency        text not null default 'EUR',
    created_at      timestamptz not null default now()
);

create table accounts (
    id          uuid primary key,
    player_id   uuid not null references players(id),
    currency    text not null,
    env         text not null check (env in ('live', 'sandbox')),
    balance     bigint not null default 0,          -- cached sum of ledger_entries, minor units
    created_at  timestamptz not null default now(),
    unique (player_id, currency, env)
);

-- Ledger ----------------------------------------------------------------------
-- A journal is one business event (a bet, a win, a deposit). It owns exactly two
-- ledger entries that sum to zero. (source, idempotency_key) is the dedupe key.

create table journals (
    id               uuid primary key,
    source           text not null,                 -- bigbang | stripe | sports | chaos
    idempotency_key  text not null,
    kind             text not null,                 -- bet | win | refund | net_round | deposit | withdrawal | rollback | tombstone | cashout
    player_id        uuid not null references players(id),
    account_id       uuid not null references accounts(id),
    amount           bigint not null,               -- signed, player's point of view
    round_id         text,
    round_end        boolean,
    reply_balance    bigint not null,               -- balance after this journal; returned on duplicates
    rolled_back      boolean not null default false,
    meta             jsonb not null default '{}'::jsonb,
    created_at       timestamptz not null default now(),
    unique (source, idempotency_key)
);

create index journals_player_created_idx on journals (player_id, created_at desc);
create index journals_round_idx on journals (source, round_id);

create table ledger_entries (
    id          bigserial primary key,
    journal_id  uuid not null references journals(id),
    account_id  uuid references accounts(id),       -- null for provider counter-accounts
    counter     text not null,                      -- 'player' or 'provider:<source>'
    amount      bigint not null,
    created_at  timestamptz not null default now()
);

create index ledger_entries_journal_idx on ledger_entries (journal_id);

create table rounds (
    source      text not null,
    round_id    text not null,
    player_id   uuid not null references players(id),
    net         bigint not null default 0,
    opened_at   timestamptz not null default now(),
    closed_at   timestamptz,
    primary key (source, round_id)
);

-- Inbound callbacks and webhooks: stored raw, before anything else runs ------

create table inbound_events (
    id            uuid primary key,
    source        text not null,
    received_at   timestamptz not null default now(),
    method        text not null,
    path          text not null,
    headers       jsonb not null default '{}'::jsonb,
    body          text not null,
    signature_ok  boolean,
    outcome       text not null default 'received', -- received | applied | duplicate | rejected | error
    detail        text,
    journal_id    uuid references journals(id),
    replay_of     uuid references inbound_events(id)
);

create index inbound_events_received_idx on inbound_events (received_at desc);

-- Payments ---------------------------------------------------------------------

create table payments (
    id            uuid primary key,
    player_id     uuid not null references players(id),
    provider      text not null,
    direction     text not null check (direction in ('deposit', 'withdrawal')),
    provider_ref  text,
    status        text not null,                    -- pending | completed | failed | pending_review | paid
    amount        bigint not null,
    currency      text not null,
    created_at    timestamptz not null default now(),
    updated_at    timestamptz not null default now()
);

create index payments_player_idx on payments (player_id, created_at desc);
create unique index payments_provider_ref_idx on payments (provider, provider_ref) where provider_ref is not null;

-- Sports bets ----------------------------------------------------------------

create table bets (
    id          uuid primary key,
    player_id   uuid not null references players(id),
    ticket_ref  text unique,
    event_name  text not null,
    market      text not null,
    selection   text not null,
    stake       bigint not null,
    odds_milli  integer not null,                   -- 2.10 -> 2100
    currency    text not null,
    status      text not null,                      -- open | won | lost | void | cashed_out | rejected
    payout      bigint,
    created_at  timestamptz not null default now(),
    settled_at  timestamptz
);

create index bets_player_idx on bets (player_id, created_at desc);

-- Game catalog ---------------------------------------------------------------

create table games (
    id                bigserial primary key,
    provider          text not null,                -- bigbang
    provider_game_id  text not null,
    name              text not null,                -- launch identifier
    title             text not null,
    category          text not null,
    thumbnail         text,
    game_type         text not null default 'slot',
    is_premium        boolean not null default false,
    synced_at         timestamptz not null default now(),
    unique (provider, provider_game_id)
);

create index games_category_idx on games (provider, category);
