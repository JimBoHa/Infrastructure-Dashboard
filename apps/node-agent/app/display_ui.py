from __future__ import annotations

import json

from app.config import Settings


def render_display_page(settings: Settings) -> str:
    display = settings.display
    payload = {
        "enabled": bool(display.enabled),
        "ui_refresh_seconds": int(display.ui_refresh_seconds),
        "trend_ranges": list(display.trend_ranges or []),
    }
    return f"""<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>{settings.node_name} · Display</title>
    <style>
      :root {{
        --bg: #06121a;
        --panel: rgba(255,255,255,0.06);
        --panel2: rgba(255,255,255,0.10);
        --fg: rgba(255,255,255,0.92);
        --muted: rgba(255,255,255,0.60);
        --good: #22c55e;
        --warn: #f59e0b;
        --bad: #ef4444;
        --accent: #38bdf8;
        --radius: 18px;
      }}
      body {{
        margin: 0;
        background: radial-gradient(circle at 20% 20%, rgba(56,189,248,0.20), transparent 45%),
                    radial-gradient(circle at 80% 0%, rgba(34,197,94,0.10), transparent 50%),
                    var(--bg);
        color: var(--fg);
        font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, "Apple Color Emoji", "Segoe UI Emoji";
      }}
      header {{
        padding: 18px 20px;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
      }}
      .title {{
        display: flex;
        flex-direction: column;
        gap: 4px;
      }}
      .title h1 {{
        margin: 0;
        font-size: 22px;
        letter-spacing: 0.2px;
      }}
      .title p {{
        margin: 0;
        font-size: 13px;
        color: var(--muted);
      }}
      nav {{
        display: flex;
        gap: 10px;
        flex-wrap: wrap;
      }}
      nav button {{
        background: var(--panel);
        border: 1px solid rgba(255,255,255,0.10);
        color: var(--fg);
        border-radius: 999px;
        padding: 10px 14px;
        font-size: 14px;
        cursor: pointer;
      }}
      nav button.active {{
        background: rgba(56,189,248,0.20);
        border-color: rgba(56,189,248,0.45);
      }}
      main {{
        padding: 0 20px 22px;
        display: grid;
        gap: 14px;
      }}
      .panel {{
        background: var(--panel);
        border: 1px solid rgba(255,255,255,0.08);
        border-radius: var(--radius);
        padding: 16px;
      }}
      .grid {{
        display: grid;
        gap: 12px;
        grid-template-columns: repeat(2, minmax(0, 1fr));
      }}
      @media (min-width: 900px) {{
        .grid {{
          grid-template-columns: repeat(3, minmax(0, 1fr));
        }}
      }}
      .card {{
        background: var(--panel2);
        border: 1px solid rgba(255,255,255,0.10);
        border-radius: 16px;
        padding: 14px;
      }}
      .card h3 {{
        margin: 0 0 6px;
        font-size: 13px;
        color: var(--muted);
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.08em;
      }}
      .value {{
        font-size: 28px;
        font-weight: 700;
      }}
      .sub {{
        margin-top: 6px;
        font-size: 13px;
        color: var(--muted);
      }}
      .badge {{
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 6px 10px;
        border-radius: 999px;
        font-size: 13px;
        font-weight: 600;
        border: 1px solid rgba(255,255,255,0.12);
        background: rgba(255,255,255,0.06);
      }}
      .dot {{
        width: 10px;
        height: 10px;
        border-radius: 50%;
        background: var(--muted);
      }}
      .dot.good {{ background: var(--good); }}
      .dot.warn {{ background: var(--warn); }}
      .dot.bad {{ background: var(--bad); }}
      .hidden {{ display: none; }}
      table {{
        width: 100%;
        border-collapse: collapse;
      }}
      th, td {{
        padding: 10px 8px;
        border-bottom: 1px solid rgba(255,255,255,0.08);
        font-size: 14px;
      }}
      th {{
        color: var(--muted);
        text-align: left;
        font-weight: 600;
      }}
      .sparkline {{
        width: 100%;
        height: 44px;
      }}
      .row {{
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
      }}
      input {{
        background: rgba(0,0,0,0.25);
        border: 1px solid rgba(255,255,255,0.12);
        color: var(--fg);
        padding: 10px 12px;
        border-radius: 12px;
        font-size: 14px;
      }}
      .btn {{
        background: rgba(56,189,248,0.20);
        border: 1px solid rgba(56,189,248,0.45);
        color: var(--fg);
        padding: 10px 12px;
        border-radius: 12px;
        font-weight: 700;
        font-size: 14px;
        cursor: pointer;
      }}
      .btn.secondary {{
        background: rgba(255,255,255,0.08);
        border-color: rgba(255,255,255,0.12);
        font-weight: 600;
      }}
      .error {{
        color: var(--bad);
      }}
    </style>
  </head>
  <body>
    <header>
      <div class="title">
        <h1>{settings.node_name}</h1>
        <p id="subtitle">Node {settings.node_id} · waiting for data…</p>
      </div>
      <nav>
        <button data-page="status" class="active">Status</button>
        <button data-page="sensors">Sensors</button>
        <button data-page="trends">Trends</button>
        <button data-page="outputs" id="nav-outputs" class="hidden">Outputs</button>
      </nav>
    </header>
    <main>
      <div id="banner" class="panel hidden"></div>

      <section id="page-status" class="panel">
        <div class="grid">
          <div class="card">
            <h3>Core comms</h3>
            <div class="row">
              <div id="comms-badge" class="badge"><span class="dot"></span><span>Unknown</span></div>
            </div>
            <div id="comms-detail" class="sub">—</div>
          </div>
          <div class="card">
            <h3>Latency</h3>
            <div class="value"><span id="latency-ms">—</span><span class="sub"> ms</span></div>
            <div class="sub">Jitter <span id="jitter-ms">—</span> ms · N=<span id="latency-n">—</span> every <span id="latency-interval">—</span>s</div>
          </div>
          <div class="card">
            <h3>System</h3>
            <div class="sub">CPU <span id="cpu">—</span> · Mem <span id="mem">—</span></div>
            <div class="sub">Uptime <span id="uptime">—</span></div>
            <div class="sub">Spool backlog <span id="buffered">—</span> samples</div>
          </div>
        </div>
      </section>

      <section id="page-sensors" class="panel hidden">
        <table>
          <thead>
            <tr><th>Sensor</th><th>Value</th><th>Status</th></tr>
          </thead>
          <tbody id="sensors-body"></tbody>
        </table>
      </section>

      <section id="page-trends" class="panel hidden">
        <div class="row" style="flex-wrap: wrap;">
          <div class="sub">Range</div>
          <div id="trend-range-buttons" style="display:flex; gap: 8px; flex-wrap: wrap;"></div>
          <div style="flex:1;"></div>
          <button class="btn secondary" id="btn-refresh-trends">Refresh</button>
        </div>
        <div id="trends-body" style="margin-top: 12px; display:grid; gap: 12px;"></div>
      </section>

      <section id="page-outputs" class="panel hidden">
        <div class="row" style="flex-wrap: wrap;">
          <div class="sub">Bearer token (required for output commands)</div>
          <input id="token" placeholder="paste token once (stored locally)" style="flex:1; min-width: 260px;" />
          <input id="pin" placeholder="PIN (optional)" type="password" style="width: 160px;" />
          <button class="btn secondary" id="btn-save-token">Save</button>
        </div>
        <div class="sub" style="margin-top: 10px;">Tip: output controls are disabled unless enabled in the display profile.</div>
        <div id="outputs-body" style="margin-top: 14px; display:grid; gap: 12px;"></div>
      </section>
    </main>

    <script>
      const CONFIG = {json.dumps(payload)};
      const state = {{
        page: 'status',
        token: localStorage.getItem('farm_display_token') || '',
        range: (CONFIG.trend_ranges && CONFIG.trend_ranges[1]) || '6h',
        lastState: null,
      }};

      const setBanner = (message, isError=false) => {{
        const el = document.getElementById('banner');
        if (!message) {{
          el.classList.add('hidden');
          el.textContent = '';
          return;
        }}
        el.classList.remove('hidden');
        el.innerHTML = isError ? `<div class="error">${{message}}</div>` : message;
      }};

      function showPage(page) {{
        state.page = page;
        document.querySelectorAll('nav button[data-page]').forEach(btn => {{
          btn.classList.toggle('active', btn.dataset.page === page);
        }});
        ['status','sensors','trends','outputs'].forEach(key => {{
          document.getElementById(`page-${{key}}`).classList.toggle('hidden', key !== page);
        }});
        if (page === 'trends') refreshTrends();
        if (page === 'outputs') refreshOutputs();
      }}

      function formatDuration(seconds) {{
        const s = Math.max(Number(seconds || 0), 0);
        const h = Math.floor(s / 3600);
        const m = Math.floor((s % 3600) / 60);
        if (h) return `${{h}}h ${{m}}m`;
        return `${{m}}m`;
      }}

      function setComms(status, detail) {{
        const badge = document.getElementById('comms-badge');
        const dot = badge.querySelector('.dot');
        const text = badge.querySelector('span:last-child');
        dot.classList.remove('good','warn','bad');
        if (status === 'connected') dot.classList.add('good');
        else if (status === 'degraded') dot.classList.add('warn');
        else if (status === 'offline') dot.classList.add('bad');
        text.textContent = status || 'unknown';
        document.getElementById('comms-detail').textContent = detail || '—';
      }}

      function renderSensors(items) {{
        const body = document.getElementById('sensors-body');
        body.innerHTML = '';
        if (!items || !items.length) {{
          body.innerHTML = '<tr><td colspan="3" class="sub">No sensors configured.</td></tr>';
          return;
        }}
        for (const sensor of items) {{
          const tr = document.createElement('tr');
          const value = sensor.missing ? 'Missing' : (sensor.value != null ? `${{sensor.value}} ${{sensor.unit || ''}}` : '—');
          const status = sensor.missing ? 'missing' : (sensor.stale ? 'stale' : 'ok');
          tr.innerHTML = `<td>${{sensor.name || sensor.sensor_id}}</td><td>${{value}}</td><td class="${{status==='missing'?'error':''}}">${{status}}</td>`;
          body.appendChild(tr);
        }}
      }}

      function sparklineSvg(points) {{
        if (!points || points.length < 2) return '';
        const values = points.map(p => Number(p.value)).filter(v => Number.isFinite(v));
        if (values.length < 2) return '';
        const min = Math.min(...values);
        const max = Math.max(...values);
        const span = (max - min) || 1;
        const w = 300;
        const h = 44;
        const coords = values.map((v, i) => {{
          const x = (i / (values.length - 1)) * w;
          const y = h - ((v - min) / span) * (h - 6) - 3;
          return `${{x.toFixed(1)}},${{y.toFixed(1)}}`;
        }}).join(' ');
        return `<svg viewBox="0 0 ${{w}} ${{h}}" class="sparkline" preserveAspectRatio="none">
          <polyline fill="none" stroke="rgba(56,189,248,0.9)" stroke-width="3" points="${{coords}}" />
        </svg>`;
      }}

      async function fetchJson(path, opts={{}}) {{
        const resp = await fetch(path, opts);
        if (!resp.ok) {{
          const text = await resp.text();
          throw new Error(`${{resp.status}}: ${{text}}`);
        }}
        return await resp.json();
      }}

      async function refreshState() {{
        try {{
          const data = await fetchJson('/v1/display/state');
          state.lastState = data;
          document.getElementById('subtitle').textContent = `Node ${{data.node.node_id}} · ${{data.node.ip || 'ip unknown'}} · v${{data.node.service_version}}`;
          if (!data.display.enabled) {{
            setBanner('Display is disabled. Enable it from the main dashboard under the node “Display profile”.', true);
          }} else {{
            setBanner('');
          }}

          setComms(data.comms.status, data.comms.detail);
          document.getElementById('latency-ms').textContent = data.latency.last_latency_ms != null ? data.latency.last_latency_ms.toFixed(0) : '—';
          document.getElementById('jitter-ms').textContent = data.latency.jitter_ms != null ? data.latency.jitter_ms.toFixed(0) : '—';
          document.getElementById('latency-n').textContent = data.latency.sample_count || 0;
          document.getElementById('latency-interval').textContent = data.latency.interval_seconds || '—';
          document.getElementById('cpu').textContent = `${{(data.system.cpu_percent || 0).toFixed(0)}}%`;
          document.getElementById('mem').textContent = `${{(data.system.memory_percent || 0).toFixed(0)}}%`;
          document.getElementById('uptime').textContent = formatDuration(data.system.uptime_seconds);
          document.getElementById('buffered').textContent = data.comms.spool_backlog_samples != null ? data.comms.spool_backlog_samples : '—';

          renderSensors(data.sensors || []);
          const outputsVisible = Boolean(data.display.outputs_enabled);
          document.getElementById('nav-outputs').classList.toggle('hidden', !outputsVisible);
        }} catch (err) {{
          setBanner(`Unable to load display state: ${{err}}`, true);
        }}
      }}

      async function refreshTrends() {{
        const wrap = document.getElementById('trends-body');
        wrap.innerHTML = '';
        const cfg = state.lastState?.display;
        const trends = (cfg?.trends || []);
        if (!cfg?.enabled) {{
          wrap.innerHTML = '<div class="sub">Enable display mode to view trends.</div>';
          return;
        }}
        if (!trends.length) {{
          wrap.innerHTML = '<div class="sub">No trend sensors selected in the display profile.</div>';
          return;
        }}
        for (const t of trends) {{
          const card = document.createElement('div');
          card.className = 'card';
          card.innerHTML = `<h3>${{t.name || t.sensor_id}}</h3><div class="sub">Loading…</div>`;
          wrap.appendChild(card);
          try {{
            const url = `/v1/display/trends?sensor_id=${{encodeURIComponent(t.sensor_id)}}&range=${{encodeURIComponent(state.range)}}`;
            const headers = state.token ? {{ 'Authorization': `Bearer ${{state.token}}` }} : {{}};
            const series = await fetchJson(url, {{ headers }});
            const points = (series.series && series.series[0] && series.series[0].points) ? series.series[0].points : [];
            const last = points.length ? points[points.length - 1].value : null;
            card.innerHTML = `<h3>${{t.name || t.sensor_id}}</h3>
              <div class="row"><div class="value">${{last != null ? last.toFixed(2) : '—'}}</div><div class="sub">${{state.range}}</div></div>
              ${{sparklineSvg(points)}}\n`;
          }} catch (err) {{
            card.innerHTML = `<h3>${{t.name || t.sensor_id}}</h3><div class="error">Trend fetch failed: ${{err}}</div>`;
          }}
        }}
      }}

      async function refreshOutputs() {{
        const wrap = document.getElementById('outputs-body');
        wrap.innerHTML = '';
        const cfg = state.lastState?.display;
        if (!cfg?.enabled) {{
          wrap.innerHTML = '<div class="sub">Enable display mode to view outputs.</div>';
          return;
        }}
        if (!cfg?.outputs_enabled) {{
          wrap.innerHTML = '<div class="sub">Outputs page is disabled in the display profile.</div>';
          return;
        }}
        if (!state.token) {{
          wrap.innerHTML = '<div class="error">Bearer token required. Paste a token above and tap Save.</div>';
          return;
        }}

        try {{
          const headers = {{ 'Authorization': `Bearer ${{state.token}}` }};
          const outputs = await fetchJson('/v1/display/outputs', {{ headers }});
          if (!outputs || !outputs.length) {{
            wrap.innerHTML = '<div class="sub">No outputs configured.</div>';
            return;
          }}
          for (const out of outputs) {{
            const card = document.createElement('div');
            card.className = 'card';
            const states = out.supported_states && out.supported_states.length ? out.supported_states : ['off','on'];
            const buttons = states.map(s => `<button class="btn secondary" data-output="${{out.id}}" data-state="${{s}}">${{s}}</button>`).join(' ');
            card.innerHTML = `<h3>${{out.name}}</h3>
              <div class="row"><div class="value">${{out.state}}</div><div class="sub">${{out.id}}</div></div>
              <div style="margin-top: 10px; display:flex; gap: 8px; flex-wrap: wrap;">${{buttons}}</div>
              <div class="sub" id="out-msg-${{out.id}}"></div>`;
            wrap.appendChild(card);
          }}
        }} catch (err) {{
          wrap.innerHTML = `<div class="error">Unable to load outputs: ${{err}}</div>`;
        }}
      }}

      async function sendOutputCommand(outputId, desiredState) {{
        const msg = document.getElementById(`out-msg-${{outputId}}`);
        msg.textContent = 'Hold 2s then release to confirm…';
        msg.classList.remove('error');
        await new Promise(resolve => setTimeout(resolve, 2000));
        msg.textContent = 'Sending…';
        try {{
          const headers = {{ 'Content-Type': 'application/json', 'Authorization': `Bearer ${{state.token}}` }};
          const payload = {{ state: desiredState, reason: 'local_display', pin: document.getElementById('pin').value || null }};
          await fetchJson(`/v1/display/outputs/${{encodeURIComponent(outputId)}}/command`, {{
            method: 'POST',
            headers,
            body: JSON.stringify(payload)
          }});
          msg.textContent = 'Sent.';
          await refreshOutputs();
        }} catch (err) {{
          msg.textContent = `Failed: ${{err}}`;
          msg.classList.add('error');
        }}
      }}

      function buildTrendRangeButtons() {{
        const root = document.getElementById('trend-range-buttons');
        root.innerHTML = '';
        const ranges = (CONFIG.trend_ranges && CONFIG.trend_ranges.length) ? CONFIG.trend_ranges : ['1h','6h','24h'];
        for (const r of ranges) {{
          const btn = document.createElement('button');
          btn.className = `btn secondary`;
          btn.textContent = r;
          btn.addEventListener('click', () => {{
            state.range = r;
            refreshTrends();
          }});
          root.appendChild(btn);
        }}
      }}

      window.addEventListener('DOMContentLoaded', () => {{
        document.querySelectorAll('nav button[data-page]').forEach(btn => {{
          btn.addEventListener('click', () => showPage(btn.dataset.page));
        }});
        document.getElementById('btn-refresh-trends').addEventListener('click', refreshTrends);
        document.getElementById('btn-save-token').addEventListener('click', () => {{
          const value = document.getElementById('token').value.trim();
          state.token = value;
          localStorage.setItem('farm_display_token', value);
          refreshOutputs();
        }});
        document.getElementById('token').value = state.token;
        buildTrendRangeButtons();

        document.getElementById('outputs-body').addEventListener('click', (ev) => {{
          const btn = ev.target.closest('button[data-output]');
          if (!btn) return;
          sendOutputCommand(btn.dataset.output, btn.dataset.state);
        }});

        refreshState();
        setInterval(refreshState, Math.max(CONFIG.ui_refresh_seconds || 2, 1) * 1000);
      }});
    </script>
  </body>
</html>
"""
