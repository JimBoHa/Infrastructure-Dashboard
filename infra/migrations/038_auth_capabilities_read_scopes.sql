-- Add read-scoped capabilities for existing users.
--
-- This is an additive migration to prevent upgrades from breaking older
-- controllers where users were created before read-scoped capabilities existed.
-- It does not remove any existing capabilities.

-- View role: allow viewing core surfaces needed for the dashboard.
update users
set capabilities = (
    select jsonb_agg(distinct cap order by cap)
    from (
        select jsonb_array_elements_text(users.capabilities) as cap
        union all select 'nodes.view'
        union all select 'sensors.view'
        union all select 'outputs.view'
        union all select 'schedules.view'
        union all select 'metrics.view'
        union all select 'alerts.view'
        union all select 'analytics.view'
    ) caps
)
where lower(trim(role)) in ('view', 'viewer', 'readonly', 'read-only', 'read_only')
  and not (
      capabilities @> '["nodes.view", "sensors.view", "outputs.view", "schedules.view", "metrics.view", "alerts.view", "analytics.view"]'::jsonb
  );

-- Operator role: includes view access plus control-plane actions.
update users
set capabilities = (
    select jsonb_agg(distinct cap order by cap)
    from (
        select jsonb_array_elements_text(users.capabilities) as cap
        union all select 'nodes.view'
        union all select 'sensors.view'
        union all select 'outputs.view'
        union all select 'schedules.view'
        union all select 'metrics.view'
        union all select 'alerts.view'
        union all select 'analytics.view'
    ) caps
)
where lower(trim(role)) in ('operator', 'control')
  and not (
      capabilities @> '["nodes.view", "sensors.view", "outputs.view", "schedules.view", "metrics.view", "alerts.view", "analytics.view"]'::jsonb
  );

-- Admin role: ensure broad read access exists (additive only).
update users
set capabilities = (
    select jsonb_agg(distinct cap order by cap)
    from (
        select jsonb_array_elements_text(users.capabilities) as cap
        union all select 'nodes.view'
        union all select 'sensors.view'
        union all select 'outputs.view'
        union all select 'schedules.view'
        union all select 'metrics.view'
        union all select 'backups.view'
        union all select 'setup.credentials.view'
    ) caps
)
where lower(trim(role)) in ('admin')
  and not (
      capabilities @> '["nodes.view", "sensors.view", "outputs.view", "schedules.view", "metrics.view", "backups.view", "setup.credentials.view"]'::jsonb
  );
