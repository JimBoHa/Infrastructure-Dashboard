import type { ExternalDeviceCatalog } from "@/types/integrations";

export const fallbackExternalDeviceCatalog: ExternalDeviceCatalog = {
  version: 1,
  vendors: [
    {
      id: "schneider_electric",
      name: "Schneider Electric",
      models: [
        { id: "powerlogic_pm8210", name: "PowerLogic PM8210", since_year: null, protocols: ["modbus_tcp"], points: [] },
        { id: "powerlogic_pm8240", name: "PowerLogic PM8240", since_year: null, protocols: ["modbus_tcp"], points: [] },
        { id: "powerlogic_pm8280", name: "PowerLogic PM8280", since_year: null, protocols: ["modbus_tcp"], points: [] },
        { id: "powerlogic_pm82403", name: "PowerLogic PM82403", since_year: null, protocols: ["modbus_tcp"], points: [] },
        { id: "powerlogic_pm82404", name: "PowerLogic PM82404", since_year: null, protocols: ["modbus_tcp"], points: [] },
        { id: "powerlogic_pm8243", name: "PowerLogic PM8243", since_year: null, protocols: ["modbus_tcp"], points: [] },
      ],
    },
    {
      id: "apc",
      name: "APC by Schneider Electric",
      models: [
        { id: "apc_ups", name: "APC UPS (PowerNet MIB)", since_year: null, protocols: ["snmp"], points: [] },
        { id: "apc_pdu", name: "APC PDU (PowerNet MIB)", since_year: null, protocols: ["snmp"], points: [] },
      ],
    },
    {
      id: "setra",
      name: "Setra",
      models: [
        {
          id: "setra_power_meter_generic",
          name: "Setra Power Meter (Modbus)",
          since_year: null,
          protocols: ["modbus_tcp"],
          points: [],
        },
      ],
    },
    {
      id: "metasys",
      name: "Johnson Controls Metasys",
      models: [
        { id: "metasys_server", name: "Metasys Server", since_year: null, protocols: ["http_json"], points: [] },
      ],
    },
    {
      id: "lutron",
      name: "Lutron",
      models: [
        { id: "lutron_lip", name: "Lutron Integration Protocol (Telnet)", since_year: null, protocols: ["lutron_lip"], points: [] },
        { id: "lutron_leap", name: "Lutron LEAP (TLS)", since_year: null, protocols: ["lutron_leap"], points: [] },
      ],
    },
    {
      id: "megatron",
      name: "Megatron",
      models: [
        {
          id: "megatron_controller",
          name: "Megatron Water Treatment Controller",
          since_year: null,
          protocols: ["modbus_tcp"],
          points: [],
        },
      ],
    },
    {
      id: "cps",
      name: "Chint Power Systems (CPS)",
      models: [
        { id: "cps_sunspec", name: "CPS Inverters (SunSpec Modbus)", since_year: null, protocols: ["modbus_tcp"], points: [] },
      ],
    },
    {
      id: "tridium",
      name: "Tridium Niagara",
      models: [
        { id: "tridium_haystack", name: "Niagara 4 (Haystack API)", since_year: null, protocols: ["http_json"], points: [] },
      ],
    },
    {
      id: "multistack",
      name: "Multistack",
      models: [
        { id: "multistack_hrc", name: "Multistack HRC (BACnet/IP)", since_year: null, protocols: ["bacnet_ip"], points: [] },
      ],
    },
    {
      id: "generator_ats",
      name: "Generator / ATS",
      models: [
        { id: "generator_ats_generic", name: "Generator + ATS (Modbus/BACnet)", since_year: null, protocols: ["modbus_tcp", "bacnet_ip"], points: [] },
      ],
    },
    {
      id: "victron_energy",
      name: "Victron Energy",
      models: [
        { id: "cerbo_gx", name: "Cerbo GX", since_year: null, protocols: ["modbus_tcp"], points: [] },
        { id: "cerbo_s_gx", name: "Cerbo-S GX", since_year: null, protocols: ["modbus_tcp"], points: [] },
        { id: "color_control_gx", name: "Color Control GX", since_year: null, protocols: ["modbus_tcp"], points: [] },
        { id: "venus_gx", name: "Venus GX", since_year: null, protocols: ["modbus_tcp"], points: [] },
        { id: "octo_gx", name: "Octo GX", since_year: null, protocols: ["modbus_tcp"], points: [] },
        { id: "ekrano_gx", name: "Ekrano GX", since_year: null, protocols: ["modbus_tcp"], points: [] },
      ],
    },
  ],
};
