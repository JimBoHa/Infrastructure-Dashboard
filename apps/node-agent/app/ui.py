from __future__ import annotations

from app.config import Settings


def script_bundle() -> str:
    return """
    <script>
    const state = {
      token: localStorage.getItem('farm_node_token') || ''
    };

    function authHeaders(extra = {}) {
      const headers = { ...extra };
      if (state.token) {
        headers['Authorization'] = `Bearer ${state.token}`;
      }
      return headers;
    }

    async function loadConfig() {
      if (!state.token) {
        document.getElementById('config-error').textContent = 'Bearer token required. Paste a token above and tap Save.';
        return;
      }
      const response = await fetch('/v1/config', { headers: authHeaders() });
      if (response.status === 401) {
        document.getElementById('config-error').textContent = 'Unauthorized. Check the bearer token and try again.';
        return;
      }
      document.getElementById('config-error').textContent = '';
      const data = await response.json();
      document.getElementById('node-name').value = data.node.node_name;
      document.getElementById('wifi-ssid').value = data.wifi_hints?.ssid || '';
      document.getElementById('wifi-password').value = '';
      const applyStatus = data.wifi_hints?.apply_status;
      document.getElementById('wifi-apply-status').textContent = applyStatus ? `${applyStatus.state} (${applyStatus.message || 'ok'})` : 'not applied';
      const sensorsContainer = document.getElementById('sensors-list');
      sensorsContainer.innerHTML = '';
      data.sensors.forEach(sensor => {
        const card = document.createElement('div');
        card.className = 'card';
        card.innerHTML = `
          <h3>${sensor.name} <span class="muted">(${sensor.sensor_id})</span></h3>
          <label>Name <input type="text" id="sensor-name-${sensor.sensor_id}" value="${sensor.name}" /></label>
          <label>Interval (s) <input type="number" min="0" id="sensor-interval-${sensor.sensor_id}" value="${sensor.interval_seconds}" /></label>
          <p class="muted">Set interval to 0 to publish only on change-of-value (COV).</p>
          <label>Rolling avg (s) <input type="number" min="0" id="sensor-rolling-${sensor.sensor_id}" value="${sensor.rolling_average_seconds}" /></label>
          <label>Unit <input type="text" id="sensor-unit-${sensor.sensor_id}" value="${sensor.unit}" /></label>
          <label>Location <input type="text" id="sensor-location-${sensor.sensor_id}" value="${sensor.location ?? ''}" /></label>
          <button onclick="updateSensor('${sensor.sensor_id}')">Save sensor</button>
        `;
        sensorsContainer.appendChild(card);
      });
      const outputsContainer = document.getElementById('outputs-list');
      outputsContainer.innerHTML = '';
      data.outputs.forEach(output => {
        const card = document.createElement('div');
        card.className = 'card';
        card.innerHTML = `
          <h3>${output.name} <span class="muted">(${output.output_id})</span></h3>
          <label>Name <input type="text" id="output-name-${output.output_id}" value="${output.name}" /></label>
          <label>Default state <input type="text" id="output-state-${output.output_id}" value="${output.default_state}" /></label>
          <label>Command topic <input type="text" id="output-topic-${output.output_id}" value="${output.command_topic ?? ''}" /></label>
          <button onclick="updateOutput('${output.output_id}')">Save output</button>
        `;
        outputsContainer.appendChild(card);
      });
    }

    async function refreshProvisioning() {
      try {
        if (!state.token) return;
        const response = await fetch('/v1/provisioning/sessions', { headers: authHeaders() });
        if (response.status >= 400) return;
        const data = await response.json();
        const list = document.getElementById('provisioning-sessions');
        list.innerHTML = '';
        if (!data.sessions || !data.sessions.length) {
          list.innerHTML = '<p class="muted">No active sessions</p>';
          return;
        }
        data.sessions.forEach(session => {
          const card = document.createElement('div');
          card.className = 'card';
          card.innerHTML = `
            <div class="badge">${session.status}</div>
            <h3>${session.device_name}</h3>
            <p class="muted">Session ${session.session_id}</p>
            <p>${session.message || 'pending'}</p>
            <p class="muted">Requested ${session.requested_at}</p>
          `;
          list.appendChild(card);
        });
      } catch (err) {
        console.warn('Unable to load provisioning sessions', err);
      }
    }

    async function refreshMesh() {
      try {
        const response = await fetch('/v1/mesh');
        const data = await response.json();
        document.getElementById('mesh-health').textContent = data.summary.health;
        document.getElementById('mesh-nodes').textContent = data.summary.node_count;
        document.getElementById('mesh-channel').textContent = data.config.channel;
        document.getElementById('mesh-pan').textContent = data.config.pan_id;
        document.getElementById('mesh-epan').textContent = data.config.extended_pan_id;
        const list = document.getElementById('mesh-topology');
        list.innerHTML = '';
        if (!data.topology.length) {
          list.innerHTML = '<p class="muted">No mesh devices discovered yet.</p>';
          return;
        }
        data.topology.forEach(device => {
          const diag = device.diagnostics || {};
          const neighbors = device.metadata?.neighbors || [];
          const card = document.createElement('div');
          card.className = 'card';
          card.innerHTML = `
            <div class="badge">${diag.lqi ?? 'n/a'} LQI</div>
            <h3>${device.ieee}</h3>
            <p class="muted">Last seen ${device.last_seen || 'n/a'}</p>
            <p>RSSI ${diag.rssi ?? 'n/a'} &middot; Battery ${diag.battery_percent ?? 'n/a'}%</p>
            <p class="muted">Neighbors: ${neighbors.map(n => n.ieee).join(', ') || 'none'}</p>
          `;
          list.appendChild(card);
        });
      } catch (err) {
        console.warn('Unable to load mesh status', err);
      }
    }

    async function openMeshJoin() {
      const duration = Number(document.getElementById('mesh-join-seconds').value) || 120;
      const resp = await fetch('/v1/mesh/join', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ duration_seconds: duration })
      });
      const data = await resp.json();
      showToast(data.status || 'mesh join');
      await refreshMesh();
    }

    async function removeMeshDevice() {
      const ieee = document.getElementById('mesh-remove-ieee').value;
      if (!ieee) {
        showToast('Enter an IEEE to remove');
        return;
      }
      const resp = await fetch('/v1/mesh/remove', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ ieee })
      });
      const data = await resp.json();
      showToast(data.status || 'mesh remove');
      await refreshMesh();
    }

    async function startProvisionSession(startOnly = false) {
      if (!state.token) {
        showToast('Bearer token required');
        return;
      }
      const payload = {
        device_name: document.getElementById('node-name').value,
        wifi_ssid: document.getElementById('wifi-ssid').value,
        wifi_password: document.getElementById('wifi-password').value,
        adoption_token: document.getElementById('controller-token').value,
        start_only: startOnly,
      };
      const resp = await fetch('/v1/provisioning/session', {
        method: 'POST',
        headers: authHeaders({ 'Content-Type': 'application/json' }),
        body: JSON.stringify(payload)
      });
      const data = await resp.json();
      const statusText = data.status ? `${data.status}: ${data.message || ''}` : 'Session created';
      showToast(statusText);
      await loadConfig();
      await refreshProvisioning();
    }

    async function refreshStatus() {
      const statusResponse = await fetch('/v1/status');
      const statusData = await statusResponse.json();
      document.getElementById('status-json').textContent = JSON.stringify(statusData, null, 2);
    }

    async function updateNode() {
      if (!state.token) {
        showToast('Bearer token required');
        return;
      }
      const payload = {
        node_name: document.getElementById('node-name').value,
        heartbeat_interval_seconds: Number(document.getElementById('node-heartbeat').value),
        telemetry_interval_seconds: Number(document.getElementById('node-telemetry').value)
      };
      await fetch('/v1/node', {
        method: 'PATCH',
        headers: authHeaders({ 'Content-Type': 'application/json' }),
        body: JSON.stringify(payload)
      });
      showToast('Node settings saved');
    }

    async function updateSensor(id) {
      if (!state.token) {
        showToast('Bearer token required');
        return;
      }
      const payload = {
        name: document.getElementById(`sensor-name-${id}`).value,
        interval_seconds: Number(document.getElementById(`sensor-interval-${id}`).value),
        rolling_average_seconds: Number(document.getElementById(`sensor-rolling-${id}`).value),
        unit: document.getElementById(`sensor-unit-${id}`).value,
        location: document.getElementById(`sensor-location-${id}`).value
      };
      await fetch(`/v1/sensors/${id}`, {
        method: 'PATCH',
        headers: authHeaders({ 'Content-Type': 'application/json' }),
        body: JSON.stringify(payload)
      });
      showToast(`Sensor ${id} updated`);
    }

    async function updateOutput(id) {
      if (!state.token) {
        showToast('Bearer token required');
        return;
      }
      const payload = {
        name: document.getElementById(`output-name-${id}`).value,
        default_state: document.getElementById(`output-state-${id}`).value,
        command_topic: document.getElementById(`output-topic-${id}`).value
      };
      await fetch(`/v1/outputs/${id}`, {
        method: 'PATCH',
        headers: authHeaders({ 'Content-Type': 'application/json' }),
        body: JSON.stringify(payload)
      });
      showToast(`Output ${id} updated`);
    }

    function saveToken() {
      const value = document.getElementById('node-token').value.trim();
      state.token = value;
      localStorage.setItem('farm_node_token', value);
      showToast('Token saved');
      loadConfig();
      refreshProvisioning();
    }

    function showToast(message) {
      const toast = document.getElementById('toast');
      toast.textContent = message;
      toast.classList.add('visible');
      setTimeout(() => toast.classList.remove('visible'), 2400);
    }

    window.addEventListener('DOMContentLoaded', async () => {
      document.getElementById('node-token').value = state.token;
      document.getElementById('btn-save-token').addEventListener('click', saveToken);
      await loadConfig();
      await refreshStatus();
      await refreshProvisioning();
      await refreshMesh();
      setInterval(refreshProvisioning, 5000);
      setInterval(refreshMesh, 7000);
    });
    </script>
    """


def render_landing_page(settings: Settings, *, ble_available: bool) -> str:
    return f"""
        <html>
        <head>
            <title>{settings.node_name} · Node Agent</title>
            <style>
                body {{ font-family: Arial, sans-serif; background: #f5f5f5; margin: 0; padding: 0; }}
                header {{ background: #0f766e; color: white; padding: 1.5rem; }}
                main {{ padding: 1.5rem; display: grid; gap: 1.5rem; }}
                section {{ background: white; border-radius: 1rem; padding: 1.5rem; box-shadow: 0 12px 20px -12px rgba(15, 118, 110, 0.4); }}
                h1 {{ margin: 0; font-size: 1.8rem; }}
                h2 {{ margin-top: 0; }}
                label {{ display: flex; flex-direction: column; font-size: 0.85rem; margin-bottom: 0.75rem; color: #334155; }}
                input {{ padding: 0.6rem; border-radius: 0.6rem; border: 1px solid #94a3b8; }}
                button {{ padding: 0.6rem 1rem; border-radius: 0.6rem; border: none; background: #0f766e; color: white; cursor: pointer; margin-top: 0.4rem; }}
                button:hover {{ background: #0e5e57; }}
                button.ghost {{ background: #e2e8f0; color: #0f172a; }}
                button.ghost:hover {{ background: #cbd5e1; }}
                ul {{ padding-left: 1.2rem; }}
                .card {{ border: 1px solid #e2e8f0; border-radius: 1rem; padding: 1rem; margin-bottom: 1rem; }}
                .muted {{ color: #94a3b8; font-size: 0.8rem; }}
                pre {{ background: #0f172a; color: #f1f5f9; padding: 1rem; border-radius: 1rem; overflow-x: auto; }}
                #toast {{ position: fixed; bottom: 2rem; right: 2rem; background: #0f766e; color: white; padding: 0.8rem 1.2rem; border-radius: 999px; opacity: 0; transition: opacity 0.3s ease; }}
                #toast.visible {{ opacity: 1; }}
                .badge {{ display: inline-block; padding: 0.2rem 0.6rem; border-radius: 999px; background: #ecfeff; color: #0f766e; font-size: 0.75rem; margin-bottom: 0.4rem; }}
            </style>
        </head>
        <body data-ble-available="{str(ble_available).lower()}">
            <header>
                <h1>{settings.node_name}</h1>
                <p>Node ID: {settings.node_id} &middot; MQTT: {settings.mqtt_url}</p>
            </header>
            <main>
                <section>
                    <h2>Authorization</h2>
                    <p class="muted">Paste a bearer token to access config/provisioning endpoints (required).</p>
                    <label>Bearer token <input type="password" id="node-token" placeholder="Bearer token" /></label>
                    <button id="btn-save-token" class="ghost">Save token</button>
                    <p class="muted" id="config-error"></p>
                </section>
                <section>
                    <h2>Node settings</h2>
                    <label>Node name <input type="text" id="node-name" value="{settings.node_name}" /></label>
                    <label>Heartbeat interval (s) <input type="number" id="node-heartbeat" min="1" value="{settings.heartbeat_interval_seconds}" /></label>
                    <label>Telemetry interval (s) <input type="number" id="node-telemetry" min="1" value="{settings.telemetry_interval_seconds}" /></label>
                    <button onclick="updateNode()">Save node settings</button>
                </section>
                <section>
                    <h2>Provisioning & network</h2>
                    <p class="muted">BLE provisioning: <strong id="ble-status">{'ready' if ble_available else 'disabled'}</strong></p>
                    <label>Wi-Fi SSID <input type="text" id="wifi-ssid" placeholder="FarmWiFi" /></label>
                    <label>Wi-Fi password <input type="password" id="wifi-password" placeholder="•••••••" /></label>
                    <label>Controller token (optional) <input type="text" id="controller-token" placeholder="paste controller-issued token" /></label>
                    <p class="muted">Wi-Fi apply status: <span id="wifi-apply-status">unknown</span></p>
                    <div style="display:flex; gap: 0.5rem; flex-wrap: wrap;">
                        <button onclick="startProvisionSession(false)">Apply credentials now</button>
                        <button class="ghost" onclick="startProvisionSession(true)">Stage session only</button>
                    </div>
                    <h3>Active sessions</h3>
                    <div id="provisioning-sessions"></div>
                </section>
                <section>
                    <h2>Mesh networking</h2>
                    <p class="muted">Health: <strong id="mesh-health">{settings.mesh_summary.health}</strong> &middot; Nodes: <strong id="mesh-nodes">{settings.mesh_summary.node_count}</strong></p>
                    <p class="muted">Channel <span id="mesh-channel">{settings.mesh.channel}</span> &middot; PAN <span id="mesh-pan">{settings.mesh.pan_id}</span> &middot; EPAN <span id="mesh-epan">{settings.mesh.extended_pan_id}</span></p>
                    <div style="display:flex; gap: 0.5rem; flex-wrap: wrap;">
                        <label style="flex: 1;">Join window (s) <input type="number" min="5" id="mesh-join-seconds" value="120" /></label>
                        <button onclick="openMeshJoin()">Open join window</button>
                    </div>
                    <div style="display:flex; gap: 0.5rem; flex-wrap: wrap; margin-top: 0.5rem;">
                        <label style="flex: 1;">Remove IEEE <input type="text" id="mesh-remove-ieee" placeholder="00:11:22:33:44:55:66:77" /></label>
                        <button class="ghost" onclick="removeMeshDevice()">Remove device</button>
                        <button class="ghost" onclick="refreshMesh()">Refresh mesh</button>
                    </div>
                    <div id="mesh-topology"></div>
                </section>
                <section>
                    <h2>Sensors</h2>
                    <div id="sensors-list"></div>
                </section>
                <section>
                    <h2>Outputs</h2>
                    <div id="outputs-list"></div>
                </section>
                <section>
                    <h2>Status preview</h2>
                    <pre id="status-json">loading…</pre>
                </section>
            </main>
            <div id="toast"></div>
            {script_bundle()}
        </body>
        </html>
        """
