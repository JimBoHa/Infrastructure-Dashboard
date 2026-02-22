# TICKET-0006: Client Requested Features (v2) — Overview

## Summary
This ticket captures an overview of the latest client-requested feature set. Each item below has a dedicated requirement ticket under `project_management/tickets/` that should be implemented and validated independently.

## Feature overview (source notes)
### Optional display on a Pi5 node (configurable from web UI)
- status of communication back to server
- network latency to server IP
- jitter (variability in ping over time)
- names and live values for sensors configured on that node (including solar production data)
- advanced (later): control outputs from the touch screen and view trend data

### Deployment options
- Preconfigured storage media with node software already loaded (Pi Imager / Pi 5 flasher profile)
- Deploy from server: boot default Pi OS on LAN, discover from server, server uses SSH to deploy node software
- Network boot: Pi 5 boots with blank media from LAN network boot server and is configured with node software

### Renogy Bluetooth one-click setup
- Button in the node sensor configuration UI to connect to a Renogy Bluetooth module and pull default data points
- Trend at 30s intervals
- Near one-click configuration for non-technical users

### Weather station one-click setup (WS-2902)
- Button in the node discovery UI to connect to a TCP/IP weather station (WS-2902) and pull default data points
- Trend at 30s intervals
- Near one-click configuration for non-technical users
  - temperature
  - wind speed
  - wind direction
  - rain sensor info
  - UV
  - solar radiation
  - barometric pressure

### WAN read-only web portal via AWS pull agent
- Lite agent in AWS periodically pulls data from the on-prem controller to support remote view-only access.
- AWS instance uses DuckDNS to track the on-prem IP.
- User provides DuckDNS credentials on the on-prem controller and selects NAT port (34343 default).
- Template-driven, non-CLI setup experience.
- Option: cache trend data in AWS vs pull on-demand to reduce cost.

## Detailed requirement tickets (imported)
- `project_management/archive/archive/tickets/TICKET-0007-feature-001-pi5-local-display-basic.md`
- `project_management/archive/archive/tickets/TICKET-0008-feature-002-pi5-local-display-advanced-controls.md`
- `project_management/archive/archive/tickets/TICKET-0009-feature-003-deployment-preconfigured-media.md`
- `project_management/archive/archive/tickets/TICKET-0010-feature-004-deployment-from-server-ssh.md`
- `project_management/archive/archive/tickets/TICKET-0011-feature-005-deployment-network-boot.md`
- `project_management/archive/archive/tickets/TICKET-0012-feature-006-renogy-bt-one-click-setup.md`
- `project_management/archive/archive/tickets/TICKET-0013-feature-007-ws-2902-weather-station-setup.md`
- `project_management/archive/archive/tickets/TICKET-0014-feature-008-wan-readonly-webpage-aws.md`

## Supporting reference docs
- `project_management/archive/archive/tickets/FEATURE-TICKETS-V2-REVIEW.md`
- `project_management/archive/archive/tickets/FEATURE-TICKETS-V2-INDEX.csv`

