# TICKET-0027: Client feedback (2026-01-03..04) — ops, UX, integrations

**Source:** JCM feedback during production-environment test run(s).

**Context note:** A developer has been actively debugging and patching production on branch `fix/installer-admin-launcher` (do not modify that branch in this ticket).

## Verbatim notes (2026-01-03)

- [ ] username/password and read/write vs view only so some users
    - [ ] Admin should be able to add/remove users and configure permissions for each user
    - [ ] Operator should be able to view everything & change schedules and trigger outputs, but not change configurations/settings or add/remove devices
    - [ ] View only should be able to view everything but not make any changes
- [ ] Monitor jitter, ping uptime % over last 24 hours, ping time over last 30 min, on all Pi5 nodes and trend this data in dashboard
- [ ] Add visual feedback on buttons like “refresh” or “scan again” indicating the progress on the associated task.  For example:
    - [ ] “scan again” button could have a progress bar progress through the button.  Progress bar should take as long to move across the button as the scan takes to complete.
    - [ ] “Refresh” button should display a throbber immediately next to the button.  It should run while the refresh cycle is under way and stop when the refresh cycle is complete.  Once the server completes the refresh, have text appear in place of the button for 4 seconds saying “Complete.” Then the button returns to say “Refresh”
- [ ] Emporia meter setup appears to be requesting cloud API token.  This could probably be extrapolated from my HomeAssistant instance, but I dont see how to pull this data directly from the Emporia website.  Any reason not to use this to pull data using username and password? https://github.com/magico13/PyEmVue?tab=readme-ov-file
- [ ] Pi5 (and any attached Pico2?) tracking
    - [ ] CPU utilization (per core)
    - [ ] RAM utilization
- [ ] More menus for preconfigured devices in dropdown menus (JCM to make list of models to add and where)
- [ ] Pull data from UniFi protect
    - [ ] Motion last detected
    - [ ] Display thumbnails of AI detections (animals, line crossing)
- [ ] Add tab to dashboard for system topology including network infrastructure.  Pull network topology from UniFi? Use hostname or MAC of the Pi5s to associate node data to each Pi5
- [ ] Automatically power on pi5 nodes every 12 hours (write code into deployment to make sure pi5s power themselves on every 12 hours if they get powered down for some reason)

## Verbatim notes (2026-01-04)

- [ ] Add ability to modify users
    - [ ] Change/reset password for users
    - [ ] Users capabilities managed via selecting checkboxes rather than searching for and adding features
- [ ] Map with sensors, nodes, and other hardware placed onto it by users.
    - [ ] Use Google earth integration to pull satellite and street view that users can toggle between
    - [ ] Add a tool so admin can draw polygons as an extra layer (on top of) the Google earth background.  This will be used for outlining fields, highlighting drainage ditches, showing underground utility lines, basically whatever the user wants.
    - [ ] User should be able to zoom in to the map so the “eye altitude” is roughly 300’
    - [ ] Users should be able to overlay topographical data from their own survey or user a publicly available topo map such as https://sccgis.santacruzcountyca.gov/gisweb/

## Open questions for implementation planning

- **Operator role scope:** confirm the capability split for “operator” (schedules + outputs) vs “admin” (config + deployments + user mgmt) vs “view-only”.
- **Network metrics collection:** define the “ping target” (controller LAN IP? core-server port? MQTT broker?) and acceptable sampling cadence.
- **“Power on every 12 hours”:** clarify if this is intended as a periodic reboot/watchdog, or actual out-of-band power control (requires hardware like PoE smart switch/relay).
- **Map provider constraints:** confirm whether Google Earth licensing is acceptable and whether offline/low-connectivity operation is required for maps.

