# FEATURE-008: WAN Read-Only Web Portal via AWS Pull Agent

## Summary
Provide a “view-from-anywhere” read-only portal by deploying a small AWS-hosted agent that **pulls** data from the on-prem farm dashboard server and serves a **read-only** UI to remote users. The AWS deployment must be close to one-click (template-driven) and configurable for non-CLI users. The solution must not introduce a remote write path into the on-prem server.

## Business goal
Allow farm owners/operators to view status and trends remotely without exposing the on-prem server as a general-purpose public API.

## Raw inputs (from feature checklist)
- Create lite agent that runs in AWS and periodically pulls data from the server so users can log in from anywhere on the web.
  - DNS/domain name configured by user and pointed to an AWS IP address.
  - Interface is view-only; cannot push data to the server.
  - Can pull data upon request (trend, node status).
- AWS instance uses DuckDNS to track the IP of the farm dashboard server installed at the local site.
- User prompted to provide DuckDNS credentials on farm dashboard server and select NAT port to AWS (34343 default).
- AWS instance setup is near one-click; configurable by non-CLI user via template.
- Option: cache trend data in AWS vs pull on demand to save cost.

## Scope
### In scope
- AWS deployment template (CloudFormation or Terraform module):
  - compute (EC2 or container)
  - security group rules
  - optional storage for cached metrics (lightweight DB)
  - TLS termination (ACM + ALB or Caddy/Nginx with certbot)
- “Pull agent” service that:
  - resolves the on-prem endpoint via DuckDNS
  - pulls nodes/status + metrics/trends via read-only API token
  - stores cached copies if caching enabled
- Read-only web UI served from AWS:
  - node list/status
  - sensor trends (last 24h/7d)
  - health indicators (last sync time, errors)
- On-prem configuration UI/runbook:
  - how to configure DuckDNS updates
  - how to set port forwarding (34343 default)
  - how to generate a read-only API token for AWS agent

### Out of scope
- Full multi-tenant SaaS.
- Bi-directional sync or remote control.
- iOS app remote mode (separate effort).

## Recommended build strategy (reuse-first)
- Use an off-the-shelf reverse-proxy auth component in AWS instead of building auth:
  - OAuth2/OIDC proxy fronting the web UI.
- Prefer a single container that runs:
  - puller (cron/loop)
  - web UI server
  - reverse proxy/auth layer
  where feasible for MVP.

## Functional requirements
### FR1. Read-only guarantee
- The AWS agent uses a token with **read-only** capabilities.
- All AWS-side API routes must be read-only (no POST/PUT/DELETE that reaches on-prem).

### FR2. Connectivity model
- AWS agent resolves the on-prem host via DuckDNS.
- On-prem server exposes a single forwarded port to AWS (34343 default).
- AWS agent must tolerate on-prem IP changes; reconnect automatically.

### FR3. Pull modes
- Mode A: **Cache enabled**
  - AWS agent polls on-prem every X seconds (configurable; default 30s for node status, 5m for trends).
  - AWS stores cached status + metrics with retention controls.
- Mode B: **On-demand**
  - AWS only fetches on-prem data when a user loads a page or requests a range.
  - Minimal or no storage; lower cost.

### FR4. One-click AWS deployment UX
- Provide a template with a small parameter set:
  - domain name
  - DuckDNS hostname/subdomain
  - on-prem port (default 34343)
  - cache mode + retention (if enabled)
- After deployment, the user completes configuration via a web UI (not SSH):
  - paste in API token
  - confirm connectivity test

### FR5. Security
- All remote access must be HTTPS.
- Limit on-prem inbound exposure:
  - recommend restricting port-forward firewall rules to AWS egress IPs if feasible
  - log all pull attempts and failures
- Store secrets safely in AWS (SSM Parameter Store or Secrets Manager).

## Non-functional requirements
- AWS agent must be able to operate with intermittent on-prem connectivity (backoff, do not thrash).
- UI must load within 3 seconds on a typical broadband connection using cached mode.

## Repo boundaries (where work belongs)
- `infra/`
  - template/module for AWS resources.
- `apps/dashboard-web/` (or a dedicated `apps/wan-web/` if strictly necessary)
  - build a read-only bundle, ideally reusing existing dashboard components but locked to view-only.
- `apps/core-server/`
  - ensure there is a robust read-only token/capability that covers required endpoints.
- `docs/runbooks/`
  - WAN portal setup guide.

## Acceptance criteria
1) A non-technical user can deploy the AWS stack using a template without CLI access.
2) User can configure DuckDNS + port forwarding and validate connectivity from the AWS UI.
3) Remote portal loads and shows:
   - node status
   - sensor trends
   - last sync time + errors
4) No remote write operations are possible (attempted writes fail in AWS and do not reach on-prem).
5) Cache mode and on-demand mode both function as described.
6) Security posture is documented and includes recommended firewall restrictions.

## Test plan
- Integration test: mock on-prem server endpoints and validate pull/caching logic.
- Security test: attempt write calls through the AWS portal and confirm they are blocked.
- Manual: deploy a sandbox stack in AWS and connect to a dev on-prem instance.

## Dependencies
- Requires stable auth/capability model in core-server (read-only token).
- Requires on-prem environment where port forwarding is possible (or document alternatives).

## Risks / open questions
- Whether DuckDNS + port forwarding is acceptable for all farms (CGNAT environments may require an outbound tunnel alternative).
- Where to host the UI (single EC2 vs S3/CloudFront + API).
- Cost control for cached trend storage (retention, downsampling).
