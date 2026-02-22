# TICKET-0077: Infrastructure Dashboard rebrand + commercial integrations (last 10 years)

**Source:** User request (2026-02-21).

## Summary

Rebrand Farm Dashboard into “Infrastructure Dashboard” and extend functionality to support commercially available device integrations while preserving custom sensor support. Integrations must import all available points for supported devices manufactured in the last 10 years, using publicly available documentation (TCP/IP protocols only).

## Requirements (verbatim + consolidated)

- Copy/retain all functionality from Farm Dashboard as the base framework, including validation tests.
- macOS deployable app delivered via a `.dmg` installer for non-technical users.
- Support automatic point mapping/import for the following device families (last 10 years of models):
  - Setra Power Meters (all generations)
  - APC UPSs & PDUs (all generations)
  - Metasys HVAC controls: Ethernet controllers, NAEs, and Metasys server data
  - Megatron Water Treatment Controller
  - Lutron controllers (all generations)
  - Generator and automatic transfer switch controllers
  - PowerLogic PM8000 meters
  - CPS solar inverters
  - Tridium server
  - Multistack HRC (ethernet)
- Maintain the ability for users to add custom sensors.
- Use publicly available documentation to define point lists and automatic mapping.
- Run all tests required to validate the code before committing/pushing.

## Implementation notes (initial direction)

- Add an external device integration framework in core-server (protocol drivers + catalog-driven point mapping).
- Support Modbus TCP, SNMP, HTTP JSON, and BACnet/IP (BACnet for HVAC/chiller families).
- Extend Setup Center UI to add/configure external devices from the catalog.
- Provide device profile catalog + per-model point lists under `shared/device_profiles/`.
- Integrate new device families into analytics/overview where applicable (power, water, HVAC).

## Open questions

- Clarify any vendor/model constraints for generator/ATS controllers (manufacturers and model names).
- Confirm whether Niagara/Haystack integration is acceptable for Tridium server access (vs. proprietary N4 APIs).
- Confirm whether Lutron LEAP/RA2 LIP coverage is required beyond documented public APIs.
