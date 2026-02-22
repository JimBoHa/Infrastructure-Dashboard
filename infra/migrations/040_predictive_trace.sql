-- Predictive trace history (best-effort).
-- Used by GET /api/predictive/trace to show bootstrap/worker diagnostics.

create table if not exists predictive_trace (
    id bigserial primary key,
    recorded_at timestamptz not null default now(),
    code text not null,
    output text not null,
    model text
);

create index if not exists predictive_trace_recorded_at_idx
    on predictive_trace (recorded_at desc);
