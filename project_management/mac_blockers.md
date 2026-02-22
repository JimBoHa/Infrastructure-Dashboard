# Work Blocked on this Mac (hardware/credential limits)

This MacBook can continue code/docs/test work, but the items below need physical gear or live provider access to complete.

- **Utility provider ingestion (AN-10)**
  - Implemented provider mappers (PGE/ERCOT/NYISO) + fixture-driven contract tests and an HTTP/file/fixed dispatcher.
  - Blocked on live utility endpoints/credentials to run end-to-end QA in a real deployment.

- **External feeds hardware QA (AN-1/AN-11)**
  - Renogy Modbus validation requires at least one Rover controller + USB/RS-485 adapter to confirm register scaling and run a soak test.
  - Emporia/Tesla/Enphase HTTP pollers are implemented with contract tests, but live-path validation needs access to those devices/accounts or LAN gateways to poll real payloads.

- **ESP32 firmware validation (FW-2/3/4)**
  - Multi-channel ADS1115 and pulse-path verification needs an ESP32 board, ADS1115 breakout, and a pulse source/flow meter to confirm scaling/IDs/auth on-device.
  - Captive portal/QR provisioning and auth/perm parity require testing on physical hardware and a real Wi‑Fi environment.
