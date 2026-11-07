use std::net::{Ipv4Addr, Ipv6Addr};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};

use super::RouterClient;

/// Shared router operations for the CLI and future interfaces such as MCP.
pub struct Router {
    client: RouterClient,
}

impl Router {
    pub fn login(host: &str, username: &str, password: &str) -> Result<Self> {
        Ok(Self {
            client: RouterClient::login(host, username, password)?,
        })
    }

    pub fn logout(&mut self) {
        self.client.logout();
    }

    pub fn dashboard_settings(&self) -> Result<Value> {
        let settings = self.client.call(
            "Manifests",
            "retrieve",
            json!({ "user": "admin", "option": "wm_settings" }),
        )?;
        sanitize_dashboard_settings(&settings)
    }

    /// Unsupported: the web UI capture shows no dashboard-settings write call
    /// (`Manifests` only exposes `retrieve`), so favorites cannot be changed.
    pub fn set_dashboard_favorite(&self, tile: &str, _favorite: bool) -> Result<()> {
        validate_dashboard_tile_name(tile)?;
        bail!(
            "dashboard favorite changes are not supported on this firmware: the web UI has no write call for dashboard settings"
        )
    }

    pub fn admin_password_state(&self) -> Result<Value> {
        let user = self
            .client
            .call("UserManagement", "getUser", json!({ "name": "admin" }))?;
        sanitize_admin_password_state(&user)
    }

    pub fn interface_language(&self) -> Result<String> {
        let language = self
            .client
            .call("UserInterface", "getLanguage", json!({}))?;
        sanitize_interface_language(&language)
    }

    pub fn validate_dashboard_tile_name(tile: &str) -> Result<()> {
        validate_dashboard_tile_name(tile)
    }

    pub fn info(&self) -> Result<Value> {
        self.client.call("DeviceInfo", "get", json!({}))
    }

    pub fn led_status(&self) -> Result<Value> {
        self.client.call("LED", "getAllLedStatus", json!({}))
    }

    pub fn users(&self) -> Result<Value> {
        let users = self.client.call("UserManagement", "getUsers", json!({}))?;
        Ok(sanitize_users(&users))
    }

    pub fn dynamic_dns_services(&self) -> Result<Value> {
        self.client.call("DynDNS", "getServices", json!({}))
    }

    pub fn dynamic_dns_hosts(&self) -> Result<Value> {
        let hosts = self.client.call("DynDNS", "getHosts", json!({}))?;
        sanitize_dynamic_dns_hosts(&hosts)
    }

    pub fn add_dynamic_dns_host(
        &self,
        service: &str,
        hostname: &str,
        username: &str,
        password: &str,
    ) -> Result<()> {
        Self::validate_dynamic_dns_config(service, hostname, username, password)?;
        if find_dynamic_dns_host(&self.dynamic_dns_hosts()?, hostname).is_some() {
            bail!("DynDNS host {hostname:?} is already configured");
        }
        let response = self.client.call_sensitive(
            "DynDNS",
            "addHost",
            json!({
                "service": service,
                "hostname": hostname,
                "username": username,
                "password": password
            }),
        )?;
        ensure_mutation_succeeded(&response, "DynDNS.addHost")
    }

    pub fn remove_dynamic_dns_host(&self, hostname: &str) -> Result<()> {
        validate_dynamic_dns_hostname(hostname)?;
        if find_dynamic_dns_host(&self.dynamic_dns_hosts()?, hostname).is_none() {
            bail!("DynDNS host {hostname:?} was not found");
        }
        let response = self
            .client
            .call("DynDNS", "delHost", json!({ "hostname": hostname }))?;
        ensure_mutation_succeeded(&response, "DynDNS.delHost")
    }

    pub fn validate_dynamic_dns_config(
        service: &str,
        hostname: &str,
        username: &str,
        password: &str,
    ) -> Result<()> {
        Self::validate_dynamic_dns_identity(service, hostname, username)?;
        if password.is_empty() || password.len() > 1024 {
            bail!("DynDNS password must not be empty or exceed 1024 bytes");
        }
        Ok(())
    }

    pub fn validate_dynamic_dns_identity(
        service: &str,
        hostname: &str,
        username: &str,
    ) -> Result<()> {
        if service.is_empty()
            || service.len() > 64
            || !service
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            bail!("Enter a valid DynDNS provider identifier");
        }
        validate_dynamic_dns_hostname(hostname)?;
        if username.trim().is_empty() || username.len() > 256 {
            bail!("DynDNS username must not be empty or exceed 256 bytes");
        }
        if username.chars().any(char::is_control) {
            bail!("DynDNS username must not contain control characters");
        }
        Ok(())
    }

    pub fn wan_status(&self) -> Result<Value> {
        self.client.call("NMC", "getWANStatus", json!({}))
    }

    pub fn wan_stats(&self) -> Result<Value> {
        let stats = self
            .client
            .call("NeMo.Intf.data", "getNetDevStats", json!({}))?;
        Ok(sanitize_wan_stats(&stats))
    }

    pub fn internet_dashboard_state(&self) -> Result<Value> {
        let state = self
            .client
            .call_nmc_data("NMC", "getWANStatus", json!({}))?;
        sanitize_internet_dashboard_state(&state)
    }

    pub fn wifi_status(&self) -> Result<Value> {
        self.client.call("NMC.Wifi", "get", json!({}))
    }

    pub fn set_wifi(&self, enabled: bool) -> Result<Value> {
        self.client.call(
            "NMC.Wifi",
            "set",
            json!({ "Enable": enabled, "Status": enabled }),
        )
    }

    pub fn set_wps(&self, enabled: bool) -> Result<Value> {
        self.client
            .call("NMC.Wifi", "setWPSEnable", json!({ "enable": enabled }))
    }

    pub fn wifi_open_mode(&self) -> Result<Value> {
        let status = self
            .client
            .call("Wificom.OpenMode", "getStatus", json!({}))?;
        Ok(sanitize_open_mode_status(&status))
    }

    pub fn devices(&self) -> Result<Value> {
        self.client.call(
            "Devices",
            "get",
            json!({ "expression": "edev && physical && dhcp and .Active==true" }),
        )
    }

    pub fn all_devices(&self) -> Result<Value> {
        self.client.call(
            "Devices",
            "get",
            json!({
                "expression": {
                    "usbM2M": " usb && wmbus and .Active==true",
                    "usb": " printer && physical and .Active==true",
                    "usblogical": "volume && logical and .Active==true",
                    "wifi": "(wifi and (edev || wifi_bridge) and .Active == true)",
                    "eth": "(eth and (edev || wifi_bridge) and .Active == true)",
                    "dect": "voice && dect && handset && physical"
                }
            }),
        )
    }

    pub fn device(&self, mac: &str) -> Result<Value> {
        let mac = normalize_mac_address(mac)?;
        let devices = sanitize_device_summaries(&self.all_devices()?);
        find_device_by_mac(&devices, &mac)
            .cloned()
            .ok_or_else(|| anyhow!("Active device with MAC address {mac} was not found"))
    }

    pub fn device_history(&self) -> Result<Value> {
        self.client.call(
            "Devices",
            "get",
            json!({
                "expression": ".Active==false",
                "traverse": "down",
                "flags": ""
            }),
        )
    }

    pub fn networks(&self) -> Result<Value> {
        self.client
            .call("NeMo.Intf.lan", "getMIBs", json!({ "mibs": "wlanvap" }))
    }

    pub fn wifi_config(&self) -> Result<Value> {
        self.client.call(
            "NeMo.Intf.lan",
            "getMIBs",
            json!({ "mibs": "wlanvap base" }),
        )
    }

    pub fn wifi_clients(&self) -> Result<Value> {
        self.client.call(
            "Devices",
            "find",
            json!({ "expression": "wifi and .Active==true and mac" }),
        )
    }

    pub fn wifi_stats(&self) -> Result<Value> {
        self.client.call("NMC.Wifi", "getStats", json!({}))
    }

    pub fn wifi_radios(&self) -> Result<Value> {
        self.client
            .call("NeMo.Intf.lan", "getMIBs", json!({ "mibs": "wlanradio" }))
    }

    pub fn wifi_schedule(&self) -> Result<Value> {
        self.client.call(
            "Scheduler",
            "getCompleteSchedules",
            json!({ "type": "WLAN" }),
        )
    }

    pub fn repeater_schedule_state(&self) -> Result<bool> {
        let schedule = self.client.call_nmc_data(
            "Scheduler",
            "getCompleteSchedules",
            json!({ "type": "RWLAN" }),
        )?;
        sanitize_repeater_schedule_state(&schedule)
    }

    pub fn dashboard_power_schedules(&self) -> Result<Value> {
        let profiles = self.client.call(
            "PowerManagement",
            "getProfiles",
            json!({ "profiles": ["LED", "Light", "Deep", "WiFi", "WiFi5G"] }),
        )?;
        sanitize_dashboard_power_schedules(&profiles)
    }

    pub fn blocked_devices(&self) -> Result<Value> {
        Ok(filter_blocked_schedules(&self.device_schedules()?))
    }

    pub fn block_device(&self, mac: &str) -> Result<()> {
        let mac = normalize_mac_address(mac)?;
        let schedules = self.device_schedules()?;
        if find_device_schedule(&schedules, &mac).is_some() {
            self.client.call(
                "Scheduler",
                "overrideSchedule",
                json!({ "type": "ToD", "ID": mac, "override": "Disable" }),
            )?;
        } else {
            self.client.call(
                "Scheduler",
                "addSchedule",
                json!({
                    "type": "ToD",
                    "info": {
                        "ID": mac,
                        "base": "Weekly",
                        "def": "Enable",
                        "schedule": [],
                        "enable": true,
                        "override": "Disable"
                    }
                }),
            )?;
        }
        Ok(())
    }

    pub fn unblock_device(&self, mac: &str) -> Result<()> {
        let mac = normalize_mac_address(mac)?;
        let schedules = self.device_schedules()?;
        if find_device_schedule(&schedules, &mac).is_none() {
            bail!("No access restriction schedule exists for device {mac}");
        }
        self.client.call(
            "Scheduler",
            "overrideSchedule",
            json!({ "type": "ToD", "ID": mac, "override": "Enable" }),
        )?;
        Ok(())
    }

    fn device_schedules(&self) -> Result<Value> {
        let schedules = self
            .client
            .call("Scheduler", "getSchedules", json!({ "type": "ToD" }))?;
        Ok(sanitize_device_schedules(&schedules))
    }

    pub fn wifi_steering(&self) -> Result<Value> {
        let master = self
            .client
            .call("SSW.Steering.MasterConfig", "get", json!({}))?;
        let add_on = self
            .client
            .call("SSW.FeatureConfig.AddOnMode", "get", json!({}))?;
        Ok(json!({ "master_config": master, "add_on_mode": add_on }))
    }

    pub fn lan(&self) -> Result<Value> {
        self.client.call("NMC", "getLANIP", json!({}))
    }

    pub fn set_lan(
        &self,
        address: Ipv4Addr,
        netmask: Ipv4Addr,
        dhcp_enabled: bool,
        dhcp_min: Ipv4Addr,
        dhcp_max: Ipv4Addr,
    ) -> Result<Value> {
        Self::validate_lan_settings(address, netmask, dhcp_min, dhcp_max)?;
        self.client.call(
            "NMC",
            "setLANIP",
            json!({
                "Address": address.to_string(),
                "Netmask": netmask.to_string(),
                "DHCPEnable": dhcp_enabled,
                "DHCPMinAddress": dhcp_min.to_string(),
                "DHCPMaxAddress": dhcp_max.to_string()
            }),
        )
    }

    pub fn validate_lan_settings(
        address: Ipv4Addr,
        netmask: Ipv4Addr,
        dhcp_min: Ipv4Addr,
        dhcp_max: Ipv4Addr,
    ) -> Result<()> {
        validate_lan_config(address, netmask, dhcp_min, dhcp_max)
    }

    pub fn validate_lan_destination(&self, destination: Ipv4Addr) -> Result<()> {
        validate_lan_destination(&self.lan()?, destination)
    }

    pub fn network_details(&self) -> Result<Value> {
        let lan = self.lan()?;
        let interfaces = self.client.call(
            "NeMo.Intf.lan",
            "getMIBs",
            json!({ "mibs": "base wlanradio" }),
        )?;
        Ok(json!({ "lan": lan, "interfaces": interfaces }))
    }

    pub fn internet_details(&self) -> Result<Value> {
        let wan = self.wan_status()?;
        let ipv6 = self.ipv6()?;
        let vlan_id = self.client.call(
            "NeMo.Intf.data",
            "getFirstParameter",
            json!({ "name": "VLANID" }),
        )?;
        let mtu = self.internet_mtu()?;
        // PPP is unavailable on some connection types and firmware versions.
        // Keep WAN/IPv6 diagnostics useful while reporting PPP errors explicitly.
        let ppp = match self.ppp_info() {
            Ok(data) => json!({ "available": true, "data": data }),
            Err(error) => json!({ "available": false, "error": error.to_string() }),
        };
        Ok(json!({
            "wan_status": wan,
            "ipv6": ipv6,
            "vlan_id": vlan_id,
            "mtu": mtu,
            "ppp": ppp
        }))
    }

    pub fn internet_mtu(&self) -> Result<Value> {
        self.client.call(
            "NeMo.Intf.data",
            "getFirstParameter",
            json!({ "name": "MTU" }),
        )
    }

    pub fn ppp_info(&self) -> Result<Value> {
        self.client
            .call("NeMo.Intf.data", "getMIBs", json!({ "mibs": "ppp" }))
    }

    pub fn leases(&self) -> Result<Value> {
        self.client
            .call("DHCPv4.Server.Pool.default", "getStaticLeases", json!({}))
    }

    pub fn current_leases(&self) -> Result<Value> {
        let leases = self
            .client
            .call("DHCPv4.Server.Pool.default", "getLeases", json!({}))?;
        Ok(sanitize_leases(&leases))
    }

    pub fn dhcp_settings(&self) -> Result<Value> {
        self.client
            .call("DHCPv4.Server.Pool.default", "get", json!({}))
    }

    pub fn set_dhcp_lease_time(&self, seconds: u32) -> Result<Value> {
        Self::validate_dhcp_lease_time(seconds)?;
        self.client.call(
            "DHCPv4.Server.Pool.default",
            "setLeaseTime",
            json!({ "leasetime": seconds }),
        )
    }

    pub fn validate_dhcp_lease_time(seconds: u32) -> Result<()> {
        validate_dhcp_lease_time(seconds)
    }

    pub fn add_lease(&self, mac: &str, ip: &str) -> Result<Value> {
        self.client.call(
            "DHCPv4.Server.Pool.default",
            "addStaticLease",
            json!({ "MACAddress": mac, "IPAddress": ip }),
        )
    }

    pub fn remove_lease(&self, mac: &str) -> Result<Value> {
        self.client.call(
            "DHCPv4.Server.Pool.default",
            "deleteStaticLease",
            json!({ "MACAddress": mac }),
        )
    }

    pub fn firewall_level(&self) -> Result<Value> {
        self.client.call("Firewall", "getFirewallLevel", json!({}))
    }

    pub fn firewall_ipv6_level(&self) -> Result<Value> {
        self.client
            .call("Firewall", "getFirewallIPv6Level", json!({}))
    }

    pub fn set_firewall_ipv6_level(&self, level: &str) -> Result<Value> {
        self.client.call(
            "Firewall",
            "setFirewallIPv6Level",
            json!({ "level": level }),
        )
    }

    pub fn remote_access_status(&self) -> Result<Value> {
        let status = self.client.call("RemoteAccess", "get", json!({}))?;
        Ok(sanitize_remote_access_status(&status))
    }

    pub fn disable_remote_access(&self) -> Result<Value> {
        self.client.call("RemoteAccess", "disable", json!({}))
    }

    pub fn upnp_status(&self) -> Result<Value> {
        let status = self.client.call("UPnP-IGD", "get", json!({}))?;
        Ok(sanitize_enable_status(&status))
    }

    pub fn set_upnp_enabled(&self, enabled: bool) -> Result<Value> {
        self.client.call(
            "UPnP-IGD",
            "set",
            json!({ "parameters": { "Enable": enabled } }),
        )
    }

    pub fn firewall_ping_response(&self) -> Result<Value> {
        self.client.call(
            "Firewall",
            "getRespondToPing",
            json!({ "sourceInterface": "data" }),
        )
    }

    pub fn set_firewall_ping_response(&self, enabled: bool) -> Result<Value> {
        self.client.call(
            "Firewall",
            "setRespondToPing",
            json!({
                "sourceInterface": "data",
                "service_enable": {
                    "enableIPv4": enabled,
                    "enableIPv6": enabled
                }
            }),
        )
    }

    pub fn set_firewall_level(&self, level: &str) -> Result<Value> {
        self.client
            .call("Firewall", "setFirewallLevel", json!({ "level": level }))
    }

    pub fn guest(&self) -> Result<Value> {
        self.client.call("NMC.Guest", "get", json!({}))
    }

    pub fn set_guest(&self, enabled: bool) -> Result<Value> {
        if enabled {
            self.client.call(
                "NMC.WlanTimer",
                "setActivationTimer",
                json!({ "InterfaceName": "guest", "Timeout": 0 }),
            )
        } else {
            self.client.call(
                "NMC.WlanTimer",
                "disableActivationTimer",
                json!({ "InterfaceName": "guest" }),
            )
        }
    }

    pub fn guest_networks(&self) -> Result<Value> {
        self.client.call(
            "NeMo.Intf.guest",
            "getMIBs",
            json!({ "mibs": "wlanvap base" }),
        )
    }

    pub fn energy(&self) -> Result<Value> {
        self.client.call("PowerManagement", "get", json!({}))
    }

    pub fn energy_profiles(&self) -> Result<Value> {
        self.client
            .call("PowerManagement", "getProfiles", json!({}))
    }

    pub fn iot_status(&self) -> Result<Value> {
        self.client.call("IoTService", "getStatus", json!({}))
    }

    pub fn wifi_iot_enabled(&self) -> Result<bool> {
        let mibs = self.client.call(
            "NeMo.Intf.sec",
            "getMIBs",
            json!({ "mibs": "wlanvap base" }),
        )?;
        sanitize_wifi_iot_enabled(&mibs)
    }

    pub fn television_dashboard_state(&self) -> Result<bool> {
        let status = self
            .client
            .call_nmc_data("NMC.OrangeTV", "getIPTVStatus", json!({}))?;
        sanitize_television_dashboard_state(&status)
    }

    pub fn voice_dashboard_status(&self) -> Result<Value> {
        let trunks =
            self.client
                .call_sensitive("VoiceService.VoiceApplication", "listTrunks", json!({}))?;
        let calls = self.client.call_sensitive(
            "VoiceService.VoiceApplication",
            "getCallList",
            json!([{ "line": "1" }]),
        )?;
        sanitize_voice_dashboard_status(&trunks, &calls)
    }

    pub fn television(&self) -> Result<Value> {
        let status = self
            .client
            .call("NMC.OrangeTV", "getIPTVStatus", json!({}))?;
        let channels = self
            .client
            .call("NMC.OrangeTV", "getIPTVConfig", json!({}))?;
        Ok(json!({ "status": status, "channels": channels }))
    }

    pub fn voice(&self) -> Result<Value> {
        self.client.call("NMC", "getVoIPConfig", json!({}))
    }

    pub fn forwards(&self) -> Result<Value> {
        self.client.call(
            "Firewall",
            "getPortForwarding",
            json!({ "origin": "webui" }),
        )
    }

    pub fn add_forward(
        &self,
        name: &str,
        destination_ip: Ipv4Addr,
        external_port: u16,
        internal_port: u16,
        protocol: &str,
    ) -> Result<()> {
        validate_forward_name(name)?;
        validate_forward_protocol(protocol)?;
        if external_port == 0 || internal_port == 0 {
            bail!("Port number must be between 1 and 65535");
        }
        self.validate_lan_destination(destination_ip)?;

        let current = self.forwards()?;
        if find_forward(&current, name).is_some() {
            bail!("A port forward named {name:?} already exists");
        }

        self.client.call(
            "Firewall",
            "setPortForwarding",
            json!({
                "description": name,
                "persistent": true,
                "enable": true,
                "protocol": protocol,
                "destinationIPAddress": destination_ip.to_string(),
                "internalPort": internal_port.to_string(),
                "externalPort": external_port.to_string(),
                "origin": "webui",
                "sourceInterface": "data",
                "sourcePrefix": "",
                "id": name
            }),
        )?;
        self.client.call("Firewall", "commit", json!({}))?;
        Ok(())
    }

    pub fn remove_forward(&self, name: &str) -> Result<()> {
        validate_forward_name(name)?;
        let current = self.forwards()?;
        let entry = find_forward(&current, name)
            .ok_or_else(|| anyhow!("Port forward named {name:?} was not found"))?;
        let protocol = field_string(entry, &["Protocol", "protocol"])
            .ok_or_else(|| anyhow!("Port forward entry has no protocol"))?;
        let internal_port = field_string(entry, &["InternalPort", "internalPort"])
            .ok_or_else(|| anyhow!("Port forward entry has no internal port"))?;
        let external_port = field_string(entry, &["ExternalPort", "externalPort"])
            .ok_or_else(|| anyhow!("Port forward entry has no external port"))?;

        self.client.call(
            "Firewall",
            "setPortForwarding",
            json!({
                "description": name,
                "persistent": true,
                "enable": false,
                "protocol": protocol,
                "destinationIPAddress": "0.0.0.0",
                "internalPort": internal_port,
                "externalPort": external_port,
                "origin": "webui",
                "sourceInterface": "data",
                "sourcePrefix": "",
                "id": name
            }),
        )?;
        self.client.call("Firewall", "commit", json!({}))?;
        Ok(())
    }

    pub fn upnp_forwards(&self) -> Result<Value> {
        self.client
            .call("Firewall", "getPortForwarding", json!({ "origin": "upnp" }))
    }

    pub fn ipv6_pinholes(&self) -> Result<Value> {
        let pinholes = self.client.call("Firewall", "getPinhole", json!({}))?;
        sanitize_pinholes(&pinholes)
    }

    pub fn add_ipv6_pinhole(
        &self,
        name: &str,
        destination: Ipv6Addr,
        mac: &str,
        port: u16,
        protocol: &str,
    ) -> Result<()> {
        Self::validate_ipv6_pinhole(name, destination, mac, port, protocol)?;
        let mac = normalize_mac_address(mac)?;

        let existing = self.ipv6_pinholes()?;
        if find_pinhole(&existing, name)?.is_some() {
            bail!("IPv6 rule named {name:?} already exists");
        }

        let response = self.client.call(
            "Firewall",
            "setPinhole",
            json!({
                "id": name,
                "origin": "webui",
                "sourceInterface": "data",
                "sourcePort": "",
                "destinationPort": port.to_string(),
                "destinationIPAddress": destination.to_string(),
                "destinationMACAddress": mac,
                "sourcePrefix": "",
                "protocol": protocol,
                "ipversion": 6,
                "enable": true,
                "persistent": true
            }),
        )?;
        ensure_mutation_succeeded(&response, "Firewall.setPinhole")?;
        let response = self.client.call("Firewall", "commit", json!({}))?;
        ensure_mutation_succeeded(&response, "Firewall.commit")?;
        Ok(())
    }

    pub fn validate_ipv6_pinhole(
        name: &str,
        destination: Ipv6Addr,
        mac: &str,
        port: u16,
        protocol: &str,
    ) -> Result<()> {
        validate_forward_name(name)?;
        validate_pinhole_destination(destination)?;
        normalize_mac_address(mac)?;
        validate_pinhole_protocol(protocol)?;
        if port == 0 {
            bail!("Port must be between 1 and 65535");
        }
        Ok(())
    }

    pub fn remove_ipv6_pinhole(&self, name: &str) -> Result<()> {
        Self::validate_ipv6_pinhole_identifier(name)?;
        let pinholes = self.ipv6_pinholes()?;
        let entry = find_pinhole(&pinholes, name)?
            .ok_or_else(|| anyhow!("IPv6 rule named {name:?} was not found"))?;
        let id = field_string(entry, &["id", "Id", "ID"])
            .ok_or_else(|| anyhow!("IPv6 rule has no identifier"))?;
        let response = self
            .client
            .call("Firewall", "deletePinhole", json!({ "id": id }))?;
        ensure_mutation_succeeded(&response, "Firewall.deletePinhole")?;
        let response = self.client.call("Firewall", "commit", json!({}))?;
        ensure_mutation_succeeded(&response, "Firewall.commit")?;
        Ok(())
    }

    pub fn validate_ipv6_pinhole_identifier(name: &str) -> Result<()> {
        if name.is_empty()
            || name.len() > 256
            || name.trim() != name
            || name.chars().any(char::is_control)
        {
            bail!(
                "IPv6 rule name or ID must be 1–256 characters and contain no control characters"
            );
        }
        Ok(())
    }

    pub fn firewall_details(&self) -> Result<Value> {
        let level = self.firewall_level()?;
        let ping = self.firewall_ping_response()?;
        let custom_rules =
            self.client
                .call("Firewall", "getCustomRule", json!({ "chain": "Custom" }))?;
        let dmz = self.client.call("Firewall", "getDMZ", json!({}))?;

        Ok(json!({
            "level": level,
            "respond_to_ping": ping,
            "custom_rules": custom_rules,
            "dmz": dmz,
        }))
    }

    pub fn dmz(&self) -> Result<Value> {
        self.client.call("Firewall", "getDMZ", json!({}))
    }

    pub fn set_dmz(&self, destination: Ipv4Addr) -> Result<Value> {
        self.validate_lan_destination(destination)?;
        let current = self.dmz()?;
        let no_existing_dmz =
            current.is_null() || current.as_object().is_some_and(serde_json::Map::is_empty);
        if !no_existing_dmz {
            bail!("DMZ is already configured; clear the existing rule first");
        }
        self.client.call(
            "Firewall",
            "setDMZ",
            json!({
                "id": "webui",
                "sourceInterface": "data",
                "destinationIPAddress": destination.to_string(),
                "enable": true
            }),
        )
    }

    pub fn clear_dmz(&self) -> Result<Value> {
        self.client
            .call("Firewall", "deleteDMZ", json!({ "id": "webui" }))
    }

    pub fn ipv6(&self) -> Result<Value> {
        self.client.call("NMC.IPv6", "get", json!({}))
    }

    pub fn time(&self) -> Result<Value> {
        let current = self.client.call("Time", "getTime", json!({}))?;
        let timezone = self
            .client
            .call("Time", "getLocalTimeZoneName", json!({}))?;
        Ok(json!({ "time": current, "timezone": timezone }))
    }

    pub fn set_timezone(&self, timezone: &str) -> Result<Value> {
        validate_timezone(timezone)?;
        self.client.call(
            "Time",
            "setLocalTimeZoneName",
            json!({ "timezone": timezone }),
        )
    }

    pub fn validate_timezone(timezone: &str) -> Result<()> {
        validate_timezone(timezone)
    }

    pub fn ntp_servers(&self) -> Result<Value> {
        let servers = self.client.call("Time", "getNTPServers", json!({}))?;
        Ok(sanitize_ntp_servers(&servers))
    }

    pub fn backup_status(&self) -> Result<Value> {
        let status = self.client.call("NMC.NetworkConfig", "get", json!({}))?;
        Ok(sanitize_backup_status(&status))
    }

    pub fn reboot(&self) -> Result<Value> {
        self.client.call("NMC", "reboot", json!({}))
    }
}

fn validate_forward_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 32
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!(
            "Rule name must be 1–32 characters and contain only ASCII letters, digits, hyphens, and underscores"
        );
    }
    Ok(())
}

fn validate_forward_protocol(protocol: &str) -> Result<()> {
    if !matches!(protocol, "6" | "17" | "6,17") {
        bail!("Protocol must be TCP, UDP, or both");
    }
    Ok(())
}

fn validate_dhcp_lease_time(seconds: u32) -> Result<()> {
    if !(60..=31_536_000).contains(&seconds) {
        bail!("DHCP lease time must be between 60 seconds and 365 days");
    }
    Ok(())
}

fn validate_pinhole_destination(destination: Ipv6Addr) -> Result<()> {
    if destination.is_unspecified()
        || destination.is_loopback()
        || destination.is_multicast()
        || destination.is_unicast_link_local()
        || destination.to_ipv4_mapped().is_some()
    {
        bail!("IPv6 pinhole requires an assignable unicast IPv6 address of a LAN device");
    }
    Ok(())
}

fn validate_pinhole_protocol(protocol: &str) -> Result<()> {
    if !matches!(protocol, "6" | "17") {
        bail!("IPv6 pinhole supports only TCP or UDP");
    }
    Ok(())
}

fn ensure_mutation_succeeded(value: &Value, operation: &str) -> Result<()> {
    let is_failure = |value: &Value| match value {
        Value::Bool(false) => true,
        Value::String(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "false" | "error" | "failed" | "failure"
        ),
        _ => false,
    };
    let rejected = is_failure(value)
        || value.as_object().is_some_and(|fields| {
            fields.iter().any(|(key, value)| {
                (key.eq_ignore_ascii_case("status") || key.eq_ignore_ascii_case("success"))
                    && is_failure(value)
            })
        });
    if rejected {
        bail!("Router did not confirm operation {operation}");
    }
    Ok(())
}

fn validate_dynamic_dns_hostname(hostname: &str) -> Result<()> {
    let valid = !hostname.is_empty()
        && hostname.len() <= 253
        && hostname.trim() == hostname
        && hostname.split('.').count() >= 2
        && hostname.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });
    if !valid {
        bail!("Enter a valid DynDNS hostname with no spaces or trailing dot");
    }
    Ok(())
}

fn validate_timezone(timezone: &str) -> Result<()> {
    let valid = !timezone.is_empty()
        && timezone.len() <= 64
        && !timezone.starts_with('/')
        && !timezone.ends_with('/')
        && timezone
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'+'))
        && timezone
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..");
    if !valid {
        bail!("Enter a timezone with no spaces or control characters, such as Europe/Paris");
    }
    Ok(())
}

fn find_forward<'a>(forwards: &'a Value, name: &str) -> Option<&'a Value> {
    let requested_id = format!("webui_{name}");
    let matches_name = |entry: &&Value| {
        ["Id", "id", "Description", "description"]
            .iter()
            .filter_map(|field| entry.get(*field).and_then(Value::as_str))
            .any(|value| value == name || value == requested_id)
    };

    match forwards {
        Value::Object(entries) => entries
            .get(&requested_id)
            .or_else(|| entries.values().find(matches_name)),
        Value::Array(entries) => entries.iter().find(|entry| {
            ["Id", "id", "Description", "description"]
                .iter()
                .filter_map(|field| entry.get(*field).and_then(Value::as_str))
                .any(|value| value == name || value == requested_id)
        }),
        _ => None,
    }
}

fn field_string(value: &Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        value.get(*field).and_then(|value| match value {
            Value::String(value) => Some(value.clone()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
    })
}

fn validate_lan_config(
    address: Ipv4Addr,
    netmask: Ipv4Addr,
    dhcp_min: Ipv4Addr,
    dhcp_max: Ipv4Addr,
) -> Result<()> {
    if !address.is_private() || !dhcp_min.is_private() || !dhcp_max.is_private() {
        bail!("LAN address and DHCP range must use private RFC 1918 IPv4 addresses");
    }

    let address = u32::from(address);
    let mask = u32::from(netmask);
    let prefix = mask.leading_ones();
    if !(1..=30).contains(&prefix) || mask != (u32::MAX << (32 - prefix)) {
        bail!("Mask must be a contiguous IPv4 mask from /1 to /30");
    }

    let network = address & mask;
    let broadcast = network | !mask;
    if !is_private_network(network, broadcast) {
        bail!("LAN subnet must be entirely within a private RFC 1918 range");
    }
    if address == network || address == broadcast {
        bail!("Router LAN address cannot be the network or broadcast address");
    }
    let min = u32::from(dhcp_min);
    let max = u32::from(dhcp_max);
    if min > max {
        bail!("DHCP range start cannot be greater than its end");
    }
    if (min & mask) != network || (max & mask) != network {
        bail!("DHCP range must be within the LAN address subnet");
    }
    if min == network || max == broadcast {
        bail!("DHCP range must not include the network or broadcast address");
    }
    if (min..=max).contains(&address) {
        bail!("DHCP range must not include the router's LAN address");
    }
    Ok(())
}

fn validate_lan_destination(lan: &Value, destination: Ipv4Addr) -> Result<()> {
    if !destination.is_private() {
        bail!("Destination address must be within a private RFC 1918 IPv4 range");
    }

    let address = ipv4_field(lan, &["Address", "IPAddress", "LANIPAddress", "IP"])
        .ok_or_else(|| anyhow!("Router IPv4 address not found in LAN API response"))?;
    let netmask = ipv4_field(lan, &["Netmask", "SubnetMask", "LANSubnetMask", "Mask"])
        .ok_or_else(|| anyhow!("IPv4 mask not found in LAN API response"))?;
    let mask = u32::from(netmask);
    let prefix = mask.leading_ones();
    if !(1..=30).contains(&prefix) || mask != (u32::MAX << (32 - prefix)) {
        bail!("Router returned an invalid LAN mask");
    }

    let address = u32::from(address);
    let network = address & mask;
    let broadcast = network | !mask;
    if !is_private_network(network, broadcast) {
        bail!("Current LAN subnet is not a private RFC 1918 IPv4 subnet");
    }
    if address == network || address == broadcast {
        bail!("Router returned an invalid LAN address");
    }

    let destination = u32::from(destination);
    if destination & mask != network {
        bail!("Destination address must be within the current LAN subnet");
    }
    if destination == network || destination == broadcast || destination == address {
        bail!("Destination address must be another LAN host address");
    }

    Ok(())
}

fn ipv4_field(value: &Value, fields: &[&str]) -> Option<Ipv4Addr> {
    let object = value.as_object()?;
    fields.iter().find_map(|field| {
        object
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(field))
            .and_then(|(_, value)| value.as_str())
            .and_then(|value| value.parse().ok())
    })
}

fn is_private_network(network: u32, broadcast: u32) -> bool {
    const PRIVATE_RANGES: [(u32, u32); 3] = [
        (0x0a00_0000, 0x0aff_ffff), // 10.0.0.0/8
        (0xac10_0000, 0xac1f_ffff), // 172.16.0.0/12
        (0xc0a8_0000, 0xc0a8_ffff), // 192.168.0.0/16
    ];

    PRIVATE_RANGES
        .iter()
        .any(|(start, end)| network >= *start && broadcast <= *end)
}

fn sanitize_users(value: &Value) -> Value {
    let mut users = Vec::new();
    collect_user_records(value, &mut users);
    Value::Array(users)
}

fn collect_user_records(value: &Value, users: &mut Vec<Value>) {
    match value {
        Value::Array(entries) => {
            for entry in entries {
                collect_user_records(entry, users);
            }
        }
        Value::Object(fields) if fields.keys().any(|key| is_public_user_field(key)) => {
            users.push(Value::Object(
                fields
                    .iter()
                    .filter(|(key, _)| is_public_user_field(key))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            ));
        }
        Value::Object(fields) => {
            for entry in fields.values() {
                collect_user_records(entry, users);
            }
        }
        _ => {}
    }
}

fn is_public_user_field(key: &str) -> bool {
    ["name", "enable", "type", "groups"]
        .iter()
        .any(|allowed| key.eq_ignore_ascii_case(allowed))
}

fn sanitize_leases(value: &Value) -> Value {
    let mut leases = Vec::new();
    collect_lease_records(value, &mut leases);
    Value::Array(leases)
}

fn sanitize_device_summaries(value: &Value) -> Value {
    let mut devices = Vec::new();
    collect_device_summaries(value, &mut devices);
    Value::Array(devices)
}

fn sanitize_dashboard_settings(value: &Value) -> Result<Value> {
    let fields = value
        .as_object()
        .ok_or_else(|| anyhow!("Router returned an unexpected dashboard settings format"))?;
    let version = fields
        .get("version")
        .and_then(Value::as_str)
        .unwrap_or("V2");
    if !matches!(version, "V1" | "V2") {
        bail!("Unsupported dashboard settings version: {version}");
    }
    let categories = fields
        .get("categories")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("Dashboard settings contain no category list"))?;

    let mut clean_categories = serde_json::Map::new();
    for (category, entries) in categories {
        validate_dashboard_category_name(category)?;
        let entries = entries
            .as_array()
            .ok_or_else(|| anyhow!("Dashboard category {category:?} has an invalid format"))?;
        let mut clean_entries = Vec::with_capacity(entries.len());
        for (position, entry) in entries.iter().enumerate() {
            let entry = entry.as_object().ok_or_else(|| {
                anyhow!("Dashboard category {category:?} contains an invalid entry")
            })?;
            let raw_name = entry
                .get("name")
                .or_else(|| entry.get("manifest"))
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    anyhow!("Dashboard category {category:?} contains a tile without a name")
                })?;
            let name = raw_name.strip_prefix("internal_").unwrap_or(raw_name);
            validate_dashboard_tile_name(name)?;
            let index = match entry.get("index") {
                None => position as u64,
                Some(Value::Number(index)) => index
                    .as_u64()
                    .ok_or_else(|| anyhow!("Invalid index for dashboard tile {name:?}"))?,
                Some(Value::String(index)) => index
                    .parse::<u64>()
                    .with_context(|| format!("Invalid index for dashboard tile {name:?}"))?,
                Some(_) => bail!("Invalid index for dashboard tile {name:?}"),
            };
            let mut clean_entry = json!({
                "name": name,
                "index": index
            });
            if let Some(is_favorite) = entry.get("isFavorite") {
                let is_favorite = is_favorite
                    .as_bool()
                    .ok_or_else(|| anyhow!("Invalid favorite status for tile {name:?}"))?;
                clean_entry["isFavorite"] = Value::Bool(is_favorite);
            }
            clean_entries.push(clean_entry);
        }
        clean_categories.insert(category.clone(), Value::Array(clean_entries));
    }

    let mut clean = serde_json::Map::new();
    clean.insert("version".to_owned(), Value::String("V2".to_owned()));
    clean.insert("categories".to_owned(), Value::Object(clean_categories));
    if let Some(project) = fields.get("project").and_then(Value::as_str)
        && project.len() <= 64
        && project
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        clean.insert("project".to_owned(), Value::String(project.to_owned()));
    }
    Ok(Value::Object(clean))
}

fn sanitize_admin_password_state(value: &Value) -> Result<Value> {
    let raw = value
        .get("passwordstate")
        .ok_or_else(|| anyhow!("Router returned an unexpected administrator password status"))?;
    let state = match raw.as_str() {
        Some(state @ ("notSet" | "factoryDefault")) => state,
        _ => "unknown",
    };
    Ok(json!({ "state": state }))
}

fn sanitize_interface_language(value: &Value) -> Result<String> {
    let language = value
        .as_str()
        .ok_or_else(|| anyhow!("Router returned an unexpected interface language"))?;
    if !matches!(language, "en" | "pl") {
        bail!("Router reported an unsupported interface language code");
    }
    Ok(language.to_owned())
}

fn sanitize_internet_dashboard_state(value: &Value) -> Result<Value> {
    // NMC.getWANStatus reports ConnectionState ("Connected" when the link is up)
    // and LinkType (for example "gpon"). Addresses are intentionally dropped.
    let connection = value
        .get("ConnectionState")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Response contains no internet connection state"))?;
    let available = connection == "Connected";
    let mut clean = json!({ "available": available });
    if available {
        let link_mode = value
            .get("LinkType")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Response contains no internet connection type"))?;
        if link_mode.is_empty() || link_mode.len() > 64 || link_mode.chars().any(char::is_control) {
            bail!("Router returned an invalid internet connection type");
        }
        clean["linkMode"] = Value::String(link_mode.to_owned());
    }
    Ok(clean)
}

fn sanitize_repeater_schedule_state(value: &Value) -> Result<bool> {
    let schedules = value
        .get("scheduleInfo")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Router returned an unexpected repeater schedule format"))?;
    let Some(first) = schedules.first() else {
        return Ok(false);
    };
    first
        .get("enable")
        .and_then(Value::as_bool)
        .ok_or_else(|| anyhow!("Repeater schedule has no valid enable status"))
}

fn sanitize_dashboard_power_schedules(value: &Value) -> Result<Value> {
    let profiles = value
        .as_object()
        .ok_or_else(|| anyhow!("Router returned an unexpected list of power-saving profiles"))?;
    let mut result = serde_json::Map::new();
    for name in ["LED", "Light", "Deep", "WiFi", "WiFi5G"] {
        let Some(profile) = profiles.get(name) else {
            continue;
        };
        let profile = profile
            .as_object()
            .ok_or_else(|| anyhow!("Power-saving profile {name} has an invalid format"))?;
        let active = match profile.get("Activate") {
            None => false,
            Some(Value::Bool(active)) => *active,
            Some(_) => bail!("Power-saving profile {name} has an invalid Activate status"),
        };
        let scheduled = if active {
            profile
                .get("Scheduler")
                .and_then(|scheduler| scheduler.get("Type"))
                .and_then(Value::as_str)
                .is_some_and(|kind| kind == "Weekly")
        } else {
            false
        };
        result.insert(name.to_owned(), Value::Bool(scheduled));
    }
    if result.is_empty() {
        bail!("Router returned no recognized power-saving profiles");
    }
    Ok(Value::Object(result))
}

fn sanitize_wifi_iot_enabled(value: &Value) -> Result<bool> {
    let base = value
        .get("base")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("Router returned an unexpected list of Wi-Fi interfaces"))?;
    let wlanvap = value
        .get("wlanvap")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("Router returned an unexpected list of Wi-Fi access points"))?;
    let mut any_enabled = false;
    for name in wlanvap.keys() {
        let enabled = base
            .get(name)
            .and_then(|interface| interface.get("Enable"))
            .and_then(Value::as_bool)
            .ok_or_else(|| anyhow!("Wi-Fi access point has no valid Enable status"))?;
        any_enabled |= enabled;
    }
    Ok(any_enabled)
}

fn sanitize_television_dashboard_state(value: &Value) -> Result<bool> {
    let state = value
        .get("IPTVStatus")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("Router did not report the IPTV state"))?;
    match state {
        "Unavailable" | "Disabled" => Ok(false),
        "Enabled" => Ok(true),
        _ => bail!("Router returned an unsupported IPTV state format"),
    }
}

fn sanitize_voice_dashboard_status(trunks: &Value, calls: &Value) -> Result<Value> {
    let trunk = trunks
        .as_array()
        .and_then(|trunks| trunks.first())
        .ok_or_else(|| anyhow!("Router did not report the phone line status"))?;
    let enabled = match trunk.get("enable").and_then(Value::as_str) {
        Some("Enabled") => true,
        Some("Disabled") => false,
        _ => bail!("Router returned an unsupported phone line status"),
    };
    let calls = calls
        .as_array()
        .ok_or_else(|| anyhow!("Router returned an unexpected call list"))?;
    let mut missed_calls = 0u64;
    for call in calls {
        let call_type = call
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Call log entry has no type"))?;
        if call_type == "missed" {
            missed_calls = missed_calls
                .checked_add(1)
                .ok_or_else(|| anyhow!("Missed call counter overflowed"))?;
        }
    }
    Ok(json!({ "lineEnabled": enabled, "missedCalls": missed_calls }))
}

#[cfg(test)]
fn set_dashboard_favorite_in_settings(
    settings: &mut Value,
    tile: &str,
    favorite: bool,
) -> Result<()> {
    validate_dashboard_tile_name(tile)?;
    let categories = settings
        .get_mut("categories")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("Dashboard settings contain no category list"))?;
    let mut matches = 0;
    for (category, entries) in categories.iter_mut() {
        if category.eq_ignore_ascii_case("Favorites") {
            continue;
        }
        let entries = entries
            .as_array_mut()
            .ok_or_else(|| anyhow!("Dashboard category {category:?} has an invalid format"))?;
        for entry in entries {
            if entry.get("name").and_then(Value::as_str) == Some(tile) {
                entry["isFavorite"] = Value::Bool(favorite);
                matches += 1;
            }
        }
    }
    if matches == 0 {
        bail!("Dashboard tile {tile:?} was not found");
    }
    if matches > 1 {
        bail!("Dashboard tile {tile:?} appears in multiple categories");
    }

    let favorites_key = categories
        .keys()
        .find(|key| key.eq_ignore_ascii_case("Favorites"))
        .cloned()
        .unwrap_or_else(|| "Favorites".to_owned());
    let entries = categories
        .entry(favorites_key)
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| anyhow!("Favorites category has an invalid format"))?;
    if favorite {
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.get("name").and_then(Value::as_str) == Some(tile))
        {
            entry["isFavorite"] = Value::Bool(true);
        } else {
            let index = entries
                .iter()
                .filter_map(|entry| entry.get("index").and_then(Value::as_u64))
                .max()
                .map_or(Some(0), |index| index.checked_add(1))
                .ok_or_else(|| anyhow!("Favorites tile index overflowed"))?;
            entries.push(json!({ "name": tile, "index": index, "isFavorite": true }));
        }
    } else {
        entries.retain(|entry| entry.get("name").and_then(Value::as_str) != Some(tile));
        entries.sort_by_key(|entry| {
            entry
                .get("index")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX)
        });
        for (index, entry) in entries.iter_mut().enumerate() {
            entry["index"] = json!(index);
        }
    }
    Ok(())
}

fn validate_dashboard_tile_name(tile: &str) -> Result<()> {
    if tile.is_empty()
        || tile.len() > 64
        || !tile
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        bail!(
            "Dashboard tile name must contain only ASCII letters, digits, hyphens, and underscores"
        );
    }
    Ok(())
}

fn validate_dashboard_category_name(category: &str) -> Result<()> {
    if category.is_empty()
        || category.len() > 64
        || !category
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        bail!("Unknown dashboard category name format");
    }
    Ok(())
}

fn sanitize_dynamic_dns_hosts(value: &Value) -> Result<Value> {
    let mut hosts = Vec::new();
    collect_dynamic_dns_hosts(value, &mut hosts);
    if has_unrecognized_dynamic_dns_entry(value, false) {
        bail!(
            "DynDNS response contains unrecognized entries; list was not displayed and change was canceled"
        );
    }
    if hosts.is_empty()
        && !is_empty_supported_collection(
            value,
            &["hosts", "host", "entries"],
            &["status", "count", "total", "data", "result"],
        )
    {
        bail!("Router returned an unknown DynDNS host format; contents were not displayed");
    }
    Ok(Value::Array(hosts))
}

fn collect_dynamic_dns_hosts(value: &Value, hosts: &mut Vec<Value>) {
    match value {
        Value::Array(entries) => {
            for entry in entries {
                collect_dynamic_dns_hosts(entry, hosts);
            }
        }
        Value::Object(fields)
            if fields.iter().any(|(key, value)| {
                key.eq_ignore_ascii_case("hostname") && value.as_str().is_some()
            }) =>
        {
            hosts.push(Value::Object(
                fields
                    .iter()
                    .filter(|(key, _)| {
                        ["hostname", "service", "status", "enable"]
                            .iter()
                            .any(|allowed| key.eq_ignore_ascii_case(allowed))
                    })
                    .filter(|(_, value)| !matches!(value, Value::Array(_) | Value::Object(_)))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            ));
        }
        Value::Object(fields) => {
            for entry in fields.values() {
                collect_dynamic_dns_hosts(entry, hosts);
            }
        }
        _ => {}
    }
}

fn find_dynamic_dns_host<'a>(hosts: &'a Value, hostname: &str) -> Option<&'a Value> {
    hosts.as_array()?.iter().find(|host| {
        field_string_case_insensitive(host, &["hostname"])
            .is_some_and(|current| current.eq_ignore_ascii_case(hostname))
    })
}

fn sanitize_pinholes(value: &Value) -> Result<Value> {
    let mut pinholes = Vec::new();
    collect_pinholes(value, &mut pinholes);
    if has_unrecognized_pinhole_entry(value, false) {
        bail!(
            "Response contains unrecognized IPv6 rules; list was not displayed and change was canceled"
        );
    }
    if pinholes.is_empty()
        && !is_empty_supported_collection(
            value,
            &["pinholes", "pinhole", "entries", "rules"],
            &["status", "count", "total", "data", "result"],
        )
    {
        bail!("Router returned an unknown IPv6 pinhole rule format; contents were not displayed");
    }
    Ok(Value::Array(pinholes))
}

fn is_empty_supported_collection(
    value: &Value,
    collection_keys: &[&str],
    wrapper_keys: &[&str],
) -> bool {
    match value {
        Value::Null => true,
        Value::Array(entries) => entries.is_empty(),
        Value::Object(fields) if fields.is_empty() => true,
        Value::Object(fields) => {
            let has_collection = fields.keys().any(|key| {
                collection_keys
                    .iter()
                    .any(|name| key.eq_ignore_ascii_case(name))
            });
            if has_collection {
                fields.iter().all(|(key, value)| {
                    if collection_keys
                        .iter()
                        .any(|name| key.eq_ignore_ascii_case(name))
                    {
                        matches!(value, Value::Null)
                            || matches!(value, Value::Array(values) if values.is_empty())
                            || matches!(value, Value::Object(values) if values.is_empty())
                    } else {
                        wrapper_keys
                            .iter()
                            .any(|name| key.eq_ignore_ascii_case(name))
                            && !value.is_object()
                            && !value.is_array()
                    }
                })
            } else if fields.len() == 1 {
                fields.iter().all(|(key, value)| {
                    wrapper_keys
                        .iter()
                        .any(|name| key.eq_ignore_ascii_case(name))
                        && is_empty_supported_collection(value, collection_keys, wrapper_keys)
                })
            } else {
                false
            }
        }
        _ => false,
    }
}

fn has_unrecognized_dynamic_dns_entry(value: &Value, inside_collection: bool) -> bool {
    match value {
        Value::Array(entries) => entries
            .iter()
            .any(|entry| has_unrecognized_dynamic_dns_entry(entry, true)),
        Value::Object(fields) => {
            let is_record = fields.iter().any(|(key, value)| {
                key.eq_ignore_ascii_case("hostname") && value.as_str().is_some()
            });
            if is_record {
                return false;
            }
            let has_host_fields = fields.keys().any(|key| {
                ["hostname", "service", "username", "password", "enable"]
                    .iter()
                    .any(|known| key.eq_ignore_ascii_case(known))
            });
            if has_host_fields {
                return true;
            }
            if fields.is_empty() {
                return inside_collection;
            }
            fields.iter().any(|(key, child)| {
                let is_collection = ["hosts", "host", "entries"]
                    .iter()
                    .any(|name| key.eq_ignore_ascii_case(name));
                let is_wrapper = ["status", "data", "result"]
                    .iter()
                    .any(|name| key.eq_ignore_ascii_case(name));
                has_unrecognized_dynamic_dns_entry(
                    child,
                    is_collection || (inside_collection && !is_wrapper),
                )
            })
        }
        Value::Null => inside_collection,
        Value::Bool(_) | Value::Number(_) | Value::String(_) => inside_collection,
    }
}

fn has_unrecognized_pinhole_entry(value: &Value, inside_collection: bool) -> bool {
    match value {
        Value::Array(entries) => entries
            .iter()
            .any(|entry| has_unrecognized_pinhole_entry(entry, true)),
        Value::Object(fields) => {
            if is_pinhole_record(fields) {
                return false;
            }
            let has_rule_fields = fields.keys().any(|key| {
                [
                    "id",
                    "description",
                    "protocol",
                    "enable",
                    "destinationipaddress",
                    "destinationport",
                ]
                .iter()
                .any(|known| key.eq_ignore_ascii_case(known))
            });
            if has_rule_fields {
                return true;
            }
            if fields.is_empty() {
                return inside_collection;
            }
            fields.iter().any(|(key, child)| {
                let is_collection = ["pinholes", "pinhole", "entries", "rules"]
                    .iter()
                    .any(|name| key.eq_ignore_ascii_case(name));
                let is_wrapper = ["status", "data", "result"]
                    .iter()
                    .any(|name| key.eq_ignore_ascii_case(name));
                has_unrecognized_pinhole_entry(
                    child,
                    is_collection || (inside_collection && !is_wrapper),
                )
            })
        }
        Value::Null => inside_collection,
        Value::Bool(_) | Value::Number(_) | Value::String(_) => inside_collection,
    }
}

fn collect_pinholes(value: &Value, pinholes: &mut Vec<Value>) {
    match value {
        Value::Array(entries) => {
            for entry in entries {
                collect_pinholes(entry, pinholes);
            }
        }
        Value::Object(fields) if is_pinhole_record(fields) => {
            pinholes.push(Value::Object(
                fields
                    .iter()
                    .filter(|(key, _)| is_public_pinhole_field(key))
                    .filter(|(_, value)| !matches!(value, Value::Array(_) | Value::Object(_)))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            ));
        }
        Value::Object(fields) => {
            for entry in fields.values() {
                collect_pinholes(entry, pinholes);
            }
        }
        _ => {}
    }
}

fn is_pinhole_record(fields: &serde_json::Map<String, Value>) -> bool {
    fields.iter().any(|(key, value)| {
        key.eq_ignore_ascii_case("destinationIPAddress") && value.as_str().is_some()
    })
}

fn is_public_pinhole_field(key: &str) -> bool {
    [
        "id",
        "Id",
        "ID",
        "description",
        "Description",
        "protocol",
        "Protocol",
        "enable",
        "Enable",
        "destinationIPAddress",
        "DestinationIPAddress",
        "destinationPort",
        "DestinationPort",
        "sourcePort",
        "SourcePort",
    ]
    .iter()
    .any(|allowed| key.eq_ignore_ascii_case(allowed))
}

fn find_pinhole<'a>(pinholes: &'a Value, name: &str) -> Result<Option<&'a Value>> {
    let Some(entries) = pinholes.as_array() else {
        bail!("Internal error: IPv6 rule list is not an array");
    };
    let mut matches = entries.iter().filter(|entry| {
        ["id", "Id", "ID", "description", "Description"]
            .iter()
            .any(|key| field_string(entry, &[*key]).is_some_and(|id| id.eq_ignore_ascii_case(name)))
    });
    let first = matches.next();
    if matches.next().is_some() {
        bail!("Name {name:?} matches multiple IPv6 rules; deletion was canceled");
    }
    Ok(first)
}

fn collect_device_summaries(value: &Value, devices: &mut Vec<Value>) {
    match value {
        Value::Array(entries) => {
            for entry in entries {
                collect_device_summaries(entry, devices);
            }
        }
        Value::Object(fields) if is_device_summary(fields) => {
            let mut summary = serde_json::Map::new();
            for (key, value) in fields {
                if is_public_device_field(key)
                    && !matches!(value, Value::Array(_) | Value::Object(_))
                {
                    summary.insert(key.clone(), value.clone());
                } else if key.eq_ignore_ascii_case("BDD")
                    && let Some(name) = value
                        .as_object()
                        .and_then(|fields| field_value_case_insensitive(fields, &["DeviceName"]))
                {
                    summary.insert("DeviceName".to_owned(), name.clone());
                }
            }
            devices.push(Value::Object(summary));
        }
        Value::Object(fields) => {
            for entry in fields.values() {
                collect_device_summaries(entry, devices);
            }
        }
        _ => {}
    }
}

fn is_device_summary(fields: &serde_json::Map<String, Value>) -> bool {
    ["PhysAddress", "MACAddress", "IPAddress"]
        .iter()
        .any(|key| {
            fields
                .keys()
                .any(|candidate| candidate.eq_ignore_ascii_case(key))
        })
}

fn is_public_device_field(key: &str) -> bool {
    [
        "Name",
        "PhysAddress",
        "MACAddress",
        "IPAddress",
        "InterfaceName",
        "Layer2Interface",
        "DeviceType",
        "DeviceCategory",
        "Active",
    ]
    .iter()
    .any(|allowed| key.eq_ignore_ascii_case(allowed))
}

fn field_value_case_insensitive<'a>(
    fields: &'a serde_json::Map<String, Value>,
    names: &[&str],
) -> Option<&'a Value> {
    fields.iter().find_map(|(key, value)| {
        names
            .iter()
            .any(|name| key.eq_ignore_ascii_case(name))
            .then_some(value)
    })
}

fn field_string_case_insensitive<'a>(value: &'a Value, names: &[&str]) -> Option<&'a str> {
    value
        .as_object()
        .and_then(|fields| field_value_case_insensitive(fields, names))
        .and_then(Value::as_str)
}

fn find_device_by_mac<'a>(devices: &'a Value, mac: &str) -> Option<&'a Value> {
    devices.as_array()?.iter().find(|device| {
        field_string_case_insensitive(device, &["PhysAddress", "MACAddress"])
            .is_some_and(|address| address.eq_ignore_ascii_case(mac))
    })
}

fn collect_lease_records(value: &Value, leases: &mut Vec<Value>) {
    match value {
        Value::Array(entries) => {
            for entry in entries {
                collect_lease_records(entry, leases);
            }
        }
        Value::Object(fields) if is_lease_record(fields) => {
            leases.push(Value::Object(
                fields
                    .iter()
                    .filter(|(key, _)| is_public_lease_field(key))
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            ));
        }
        Value::Object(fields) => {
            for entry in fields.values() {
                collect_lease_records(entry, leases);
            }
        }
        _ => {}
    }
}

fn is_lease_record(fields: &serde_json::Map<String, Value>) -> bool {
    fields.keys().any(|key| {
        ["IPAddress", "MACAddress", "LeasePath"]
            .iter()
            .any(|field| key.eq_ignore_ascii_case(field))
    })
}

fn is_public_lease_field(key: &str) -> bool {
    [
        "IPAddress",
        "MACAddress",
        "Reserved",
        "Active",
        "FriendlyName",
        "LeaseTimeRemaining",
    ]
    .iter()
    .any(|allowed| key.eq_ignore_ascii_case(allowed))
}

fn sanitize_open_mode_status(value: &Value) -> Value {
    let Some(fields) = value.as_object() else {
        return Value::Null;
    };

    Value::Object(
        fields
            .iter()
            .filter(|(key, _)| {
                key.eq_ignore_ascii_case("enable") || key.eq_ignore_ascii_case("status")
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    )
}

fn sanitize_enable_status(value: &Value) -> Value {
    let Some(fields) = value.as_object() else {
        return Value::Null;
    };

    Value::Object(
        fields
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case("enable"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    )
}

fn sanitize_wan_stats(value: &Value) -> Value {
    let Some(fields) = value.as_object() else {
        return Value::Null;
    };

    Value::Object(
        fields
            .iter()
            .filter(|(key, _)| {
                key.eq_ignore_ascii_case("RxBytes") || key.eq_ignore_ascii_case("TxBytes")
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    )
}

fn sanitize_remote_access_status(value: &Value) -> Value {
    let Some(fields) = value.as_object() else {
        return Value::Null;
    };

    const ALLOWED_FIELDS: [&str; 5] = ["enable", "status", "port", "secure", "timeout"];
    Value::Object(
        fields
            .iter()
            .filter(|(key, _)| {
                ALLOWED_FIELDS
                    .iter()
                    .any(|field| key.eq_ignore_ascii_case(field))
            })
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    )
}

fn sanitize_ntp_servers(value: &Value) -> Value {
    let Some(servers) = value.as_object().and_then(|fields| {
        fields
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("servers"))
            .map(|(_, value)| value)
    }) else {
        return Value::Array(Vec::new());
    };

    let values = match servers {
        Value::Array(values) => values.iter().collect::<Vec<_>>(),
        Value::Object(values) => values.values().collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    Value::Array(
        values
            .into_iter()
            .filter_map(Value::as_str)
            .map(|server| Value::String(server.to_owned()))
            .collect(),
    )
}

fn sanitize_backup_status(value: &Value) -> Value {
    let Some(fields) = value.as_object() else {
        return Value::Null;
    };

    Value::Object(
        fields
            .iter()
            .filter(|(key, _)| key.eq_ignore_ascii_case("ConfigDate"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    )
}

fn sanitize_device_schedules(value: &Value) -> Value {
    let mut records = Vec::new();
    collect_device_schedules(value, &mut records);
    Value::Array(records)
}

fn collect_device_schedules(value: &Value, records: &mut Vec<Value>) {
    match value {
        Value::Array(entries) => {
            for entry in entries {
                collect_device_schedules(entry, records);
            }
        }
        Value::Object(fields) if fields.keys().any(|key| key.eq_ignore_ascii_case("ID")) => {
            records.push(Value::Object(
                fields
                    .iter()
                    .filter(|(key, _)| {
                        ["ID", "enable", "override", "value"]
                            .iter()
                            .any(|allowed| key.eq_ignore_ascii_case(allowed))
                    })
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
            ));
        }
        Value::Object(fields) => {
            for entry in fields.values() {
                collect_device_schedules(entry, records);
            }
        }
        _ => {}
    }
}

fn filter_blocked_schedules(schedules: &Value) -> Value {
    Value::Array(
        schedules
            .as_array()
            .into_iter()
            .flatten()
            .filter(|schedule| is_blocked_schedule(schedule))
            .cloned()
            .collect(),
    )
}

fn is_blocked_schedule(schedule: &Value) -> bool {
    schedule.as_object().and_then(|fields| {
        fields
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("value"))
            .and_then(|(_, value)| value.as_str())
    }) == Some("Disable")
}

fn find_device_schedule<'a>(schedules: &'a Value, mac: &str) -> Option<&'a Value> {
    schedules.as_array()?.iter().find(|schedule| {
        schedule
            .as_object()
            .and_then(|fields| {
                fields
                    .iter()
                    .find(|(key, _)| key.eq_ignore_ascii_case("ID"))
                    .and_then(|(_, value)| value.as_str())
            })
            .is_some_and(|id| id.eq_ignore_ascii_case(mac))
    })
}

fn normalize_mac_address(mac: &str) -> Result<String> {
    let octets = mac.split(':').collect::<Vec<_>>();
    if octets.len() != 6
        || octets
            .iter()
            .any(|octet| octet.len() != 2 || !octet.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        bail!("Invalid MAC address; expected format AA:BB:CC:DD:EE:FF");
    }
    let first_octet = u8::from_str_radix(octets[0], 16)?;
    if first_octet & 1 != 0 {
        bail!("A unicast MAC address is required to manage the device");
    }
    Ok(mac.to_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::{
        Router, ensure_mutation_succeeded, filter_blocked_schedules, find_device_by_mac,
        find_device_schedule, find_dynamic_dns_host, find_forward, find_pinhole,
        normalize_mac_address, sanitize_admin_password_state, sanitize_backup_status,
        sanitize_dashboard_power_schedules, sanitize_dashboard_settings, sanitize_device_schedules,
        sanitize_device_summaries, sanitize_dynamic_dns_hosts, sanitize_enable_status,
        sanitize_interface_language, sanitize_internet_dashboard_state, sanitize_leases,
        sanitize_ntp_servers, sanitize_open_mode_status, sanitize_pinholes,
        sanitize_remote_access_status, sanitize_repeater_schedule_state,
        sanitize_television_dashboard_state, sanitize_users, sanitize_voice_dashboard_status,
        sanitize_wan_stats, sanitize_wifi_iot_enabled, set_dashboard_favorite_in_settings,
        validate_dhcp_lease_time, validate_dynamic_dns_hostname, validate_forward_name,
        validate_forward_protocol, validate_lan_config, validate_lan_destination,
        validate_pinhole_destination, validate_pinhole_protocol, validate_timezone,
    };

    #[test]
    fn validates_forward_names_and_protocols() {
        assert!(validate_forward_name("home-server_1").is_ok());
        assert!(validate_forward_name("bad name").is_err());
        assert!(validate_forward_name("..").is_err());
        assert!(validate_forward_protocol("6").is_ok());
        assert!(validate_forward_protocol("17").is_ok());
        assert!(validate_forward_protocol("6,17").is_ok());
        assert!(validate_forward_protocol("tcp").is_err());
    }

    #[test]
    fn validates_dhcp_lease_time_bounds() {
        assert!(validate_dhcp_lease_time(60).is_ok());
        assert!(validate_dhcp_lease_time(86_400).is_ok());
        assert!(validate_dhcp_lease_time(31_536_000).is_ok());
        assert!(validate_dhcp_lease_time(59).is_err());
        assert!(validate_dhcp_lease_time(31_536_001).is_err());
    }

    #[test]
    fn finds_port_forward_by_id_or_description() {
        let forwards = serde_json::json!({
            "webui_game": { "Id": "game", "ExternalPort": "27015" },
            "webui_media": { "id": "media", "description": "media" }
        });
        assert!(find_forward(&forwards, "game").is_some());
        assert!(find_forward(&forwards, "media").is_some());
        assert!(find_forward(&forwards, "missing").is_none());
    }

    #[test]
    fn validates_lan_and_dhcp_subnet() {
        let ip = |value: &str| value.parse::<Ipv4Addr>().unwrap();
        assert!(
            validate_lan_config(
                ip("192.168.1.1"),
                ip("255.255.255.0"),
                ip("192.168.1.50"),
                ip("192.168.1.200")
            )
            .is_ok()
        );
        assert!(
            validate_lan_config(
                ip("192.168.1.1"),
                ip("255.255.255.0"),
                ip("192.168.1.1"),
                ip("192.168.1.200")
            )
            .is_err()
        );
        assert!(
            validate_lan_config(
                ip("192.168.1.1"),
                ip("255.0.255.0"),
                ip("192.168.1.50"),
                ip("192.168.1.200")
            )
            .is_err()
        );
        assert!(
            validate_lan_config(
                ip("8.8.8.1"),
                ip("255.255.255.0"),
                ip("8.8.8.50"),
                ip("8.8.8.200")
            )
            .is_err()
        );
        assert!(
            validate_lan_config(
                ip("127.0.0.2"),
                ip("255.255.255.0"),
                ip("127.0.0.50"),
                ip("127.0.0.200")
            )
            .is_err()
        );
        assert!(
            validate_lan_config(
                ip("10.0.0.1"),
                ip("254.0.0.0"),
                ip("10.0.0.50"),
                ip("10.0.0.200")
            )
            .is_err()
        );
    }

    #[test]
    fn forwarding_and_dmz_destinations_must_be_lan_hosts() {
        let lan = serde_json::json!({
            "Address": "192.168.1.1",
            "Netmask": "255.255.255.0"
        });
        let ip = |value: &str| value.parse::<Ipv4Addr>().unwrap();

        assert!(validate_lan_destination(&lan, ip("192.168.1.25")).is_ok());
        assert!(validate_lan_destination(&lan, ip("192.168.2.25")).is_err());
        assert!(validate_lan_destination(&lan, ip("8.8.8.8")).is_err());
        assert!(validate_lan_destination(&lan, ip("127.0.0.1")).is_err());
        assert!(validate_lan_destination(&lan, ip("192.168.1.1")).is_err());
        assert!(validate_lan_destination(&lan, ip("192.168.1.0")).is_err());
        assert!(validate_lan_destination(&lan, ip("192.168.1.255")).is_err());
        assert!(
            validate_lan_destination(
                &serde_json::json!({ "Address": "192.168.1.1" }),
                ip("192.168.1.25")
            )
            .is_err()
        );
    }

    #[test]
    fn user_output_contains_only_public_fields() {
        let users = serde_json::json!({
            "admin": {
                "Name": "admin",
                "Enable": true,
                "Type": "local",
                "Groups": ["administrators"],
                "Password": "must-not-leak",
                "PasswordHash": "also-secret"
            },
            "metadata": { "count": 1 }
        });

        let sanitized = sanitize_users(&users);
        let serialized = sanitized.to_string();
        assert!(serialized.contains("admin"));
        assert!(serialized.contains("administrators"));
        assert!(!serialized.contains("must-not-leak"));
        assert!(!serialized.contains("PasswordHash"));
    }

    #[test]
    fn open_mode_output_contains_only_status_fields() {
        let status = serde_json::json!({
            "Enable": true,
            "Status": false,
            "SSID": "private-network",
            "Password": "must-not-leak"
        });

        assert_eq!(
            sanitize_open_mode_status(&status),
            serde_json::json!({ "Enable": true, "Status": false })
        );
    }

    #[test]
    fn upnp_status_output_contains_only_enable_flag() {
        let status = serde_json::json!({
            "Enable": true,
            "Username": "not-needed",
            "Description": "UPnP gateway"
        });

        assert_eq!(
            sanitize_enable_status(&status),
            serde_json::json!({ "Enable": true })
        );
    }

    #[test]
    fn current_lease_output_is_flattened_and_allowlisted() {
        let data = serde_json::json!({
            "pool": {
                "lease_rule": {
                    "device_1": {
                        "IPAddress": "192.168.1.40",
                        "MACAddress": "AA:BB:CC:DD:EE:FF",
                        "Reserved": false,
                        "Active": true,
                        "FriendlyName": "Laptop",
                        "LeaseTimeRemaining": 300,
                        "Password": "must-not-leak"
                    }
                }
            }
        });

        let leases = sanitize_leases(&data);
        assert_eq!(leases.as_array().unwrap().len(), 1);
        let serialized = leases.to_string();
        assert!(serialized.contains("192.168.1.40"));
        assert!(!serialized.contains("must-not-leak"));
    }

    #[test]
    fn wan_stats_output_contains_only_byte_counters() {
        let stats = serde_json::json!({
            "RxBytes": 1234,
            "TxBytes": 5678,
            "InterfaceName": "data",
            "Secret": "must-not-leak"
        });

        assert_eq!(
            sanitize_wan_stats(&stats),
            serde_json::json!({ "RxBytes": 1234, "TxBytes": 5678 })
        );
    }

    #[test]
    fn remote_access_output_excludes_addresses_and_credentials() {
        let status = serde_json::json!({
            "Enable": true,
            "Port": 443,
            "Secure": true,
            "Timeout": 0,
            "SourcePrefix": "203.0.113.0/24",
            "Username": "admin",
            "Password": "must-not-leak"
        });

        assert_eq!(
            sanitize_remote_access_status(&status),
            serde_json::json!({ "Enable": true, "Port": 443, "Secure": true, "Timeout": 0 })
        );
    }

    #[test]
    fn blocked_device_output_contains_only_blocked_schedule_fields() {
        let schedules = serde_json::json!({
            "scheduleInfo": [
                {
                    "ID": "AA:BB:CC:DD:EE:FF",
                    "enable": true,
                    "override": "",
                    "value": "Disable",
                    "Password": "must-not-leak"
                },
                {
                    "ID": "11:22:33:44:55:66",
                    "enable": true,
                    "override": "",
                    "value": "Enable"
                }
            ]
        });

        let schedules = sanitize_device_schedules(&schedules);
        let blocked = filter_blocked_schedules(&schedules);
        assert_eq!(blocked.as_array().unwrap().len(), 1);
        assert_eq!(blocked[0]["ID"], "AA:BB:CC:DD:EE:FF");
        assert!(!blocked.to_string().contains("must-not-leak"));
        assert!(find_device_schedule(&schedules, "aa:bb:cc:dd:ee:ff").is_some());
        assert!(find_device_schedule(&schedules, "FF:FF:FF:FF:FF:FF").is_none());
    }

    #[test]
    fn validates_unicast_mac_addresses_for_device_controls() {
        assert_eq!(
            normalize_mac_address("aa:bb:cc:dd:ee:ff").unwrap(),
            "AA:BB:CC:DD:EE:FF"
        );
        assert!(normalize_mac_address("01:00:5E:00:00:01").is_err());
        assert!(normalize_mac_address("invalid").is_err());
    }

    #[test]
    fn device_lookup_flattens_grouped_results_and_filters_secrets() {
        let grouped = serde_json::json!({
            "wifi": {
                "entry-1": {
                    "PhysAddress": "AA:BB:CC:DD:EE:FF",
                    "IPAddress": "192.168.1.42",
                    "Name": "tablet",
                    "InterfaceName": "vap5g0priv",
                    "Active": true,
                    "BDD": { "DeviceName": "Tablet" },
                    "Password": "must-not-leak"
                }
            },
            "usb": [{
                "MACAddress": "11:22:33:44:55:66",
                "IPAddress": "192.168.1.43",
                "Secret": "also-must-not-leak"
            }]
        });

        let devices = sanitize_device_summaries(&grouped);
        assert_eq!(devices.as_array().unwrap().len(), 2);
        let found = find_device_by_mac(&devices, "aa:bb:cc:dd:ee:ff").unwrap();
        assert_eq!(found["IPAddress"], "192.168.1.42");
        assert_eq!(found["DeviceName"], "Tablet");
        assert!(!devices.to_string().contains("must-not-leak"));
        assert!(find_device_by_mac(&devices, "FF:FF:FF:FF:FF:FF").is_none());
    }

    #[test]
    fn pinhole_output_is_flattened_and_allowlisted() {
        let response = serde_json::json!({
            "entries": [{
                "id": "web-server",
                "destinationIPAddress": "fd12:3456:789a::10",
                "destinationPort": "443",
                "protocol": "6",
                "enable": true,
                "username": "must-not-leak"
            }],
            "metadata": { "password": "also-must-not-leak" }
        });

        let pinholes = sanitize_pinholes(&response).unwrap();
        assert_eq!(pinholes.as_array().unwrap().len(), 1);
        assert_eq!(pinholes[0]["id"], "web-server");
        assert!(find_pinhole(&pinholes, "WEB-SERVER").unwrap().is_some());
        assert!(!pinholes.to_string().contains("must-not-leak"));
    }

    #[test]
    fn validates_ipv6_pinhole_destination_and_protocol() {
        assert!(validate_pinhole_destination("fd12:3456:789a::10".parse().unwrap()).is_ok());
        assert!(validate_pinhole_destination("2001:db8::10".parse().unwrap()).is_ok());
        assert!(validate_pinhole_destination("::".parse().unwrap()).is_err());
        assert!(validate_pinhole_destination("::1".parse().unwrap()).is_err());
        assert!(validate_pinhole_destination("fe80::10".parse().unwrap()).is_err());
        assert!(validate_pinhole_destination("ff02::1".parse().unwrap()).is_err());
        assert!(validate_pinhole_protocol("6").is_ok());
        assert!(validate_pinhole_protocol("17").is_ok());
        assert!(validate_pinhole_protocol("6,17").is_err());
    }

    #[test]
    fn ambiguous_pinhole_identifier_is_rejected() {
        let pinholes = serde_json::json!([
            { "id": "first", "description": "second", "destinationIPAddress": "fd12::1" },
            { "id": "second", "description": "other", "destinationIPAddress": "fd12::2" }
        ]);
        assert!(find_pinhole(&pinholes, "second").is_err());
        assert!(Router::validate_ipv6_pinhole_identifier("Friendly name").is_ok());
        assert!(Router::validate_ipv6_pinhole_identifier("bad\nname").is_err());
    }

    #[test]
    fn dynamic_dns_host_output_excludes_credentials() {
        let response = serde_json::json!({
            "hosts": [{
                "hostname": "home.example.net",
                "service": "NoIP",
                "status": "updating",
                "enable": true,
                "username": "private-user",
                "password": "must-not-leak"
            }],
            "metadata": { "token": "also-must-not-leak" }
        });

        let hosts = sanitize_dynamic_dns_hosts(&response).unwrap();
        assert_eq!(hosts.as_array().unwrap().len(), 1);
        assert_eq!(hosts[0]["hostname"], "home.example.net");
        assert!(find_dynamic_dns_host(&hosts, "HOME.EXAMPLE.NET").is_some());
        assert!(!hosts.to_string().contains("private-user"));
        assert!(!hosts.to_string().contains("must-not-leak"));
    }

    #[test]
    fn validates_dynamic_dns_identity_and_hostname() {
        assert!(validate_dynamic_dns_hostname("home.example.net").is_ok());
        assert!(validate_dynamic_dns_hostname("-bad.example.net").is_err());
        assert!(validate_dynamic_dns_hostname("bad..example.net").is_err());
        assert!(validate_dynamic_dns_hostname("bad.example.net.").is_err());
        assert!(Router::validate_dynamic_dns_identity("NoIP", "home.example.net", "user").is_ok());
        assert!(Router::validate_dynamic_dns_identity("", "home.example.net", "user").is_err());
        assert!(Router::validate_dynamic_dns_identity("NoIP", "home.example.net", " ").is_err());
    }

    #[test]
    fn validates_timezone_identifiers() {
        assert!(validate_timezone("Europe/Paris").is_ok());
        assert!(validate_timezone("UTC").is_ok());
        assert!(validate_timezone("Etc/GMT+1").is_ok());
        assert!(validate_timezone("").is_err());
        assert!(validate_timezone("/Europe/Paris").is_err());
        assert!(validate_timezone("Europe//Paris").is_err());
        assert!(validate_timezone("Europe/../Paris").is_err());
        assert!(validate_timezone("Europe/Paris\n").is_err());
    }

    #[test]
    fn dashboard_settings_are_allowlisted_and_favorites_toggle_safely() {
        let raw = serde_json::json!({
            "version": "V2",
            "project": "funbox7",
            "categories": {
                "Network": [{
                    "name": "wifiAdvanced",
                    "index": 2,
                    "isFavorite": false,
                    "password": "must-not-leak"
                }],
                "Favorites": []
            },
            "accessToken": "also-must-not-leak"
        });

        let mut settings = sanitize_dashboard_settings(&raw).unwrap();
        assert!(!settings.to_string().contains("must-not-leak"));
        assert_eq!(settings["categories"]["Network"][0]["name"], "wifiAdvanced");
        set_dashboard_favorite_in_settings(&mut settings, "wifiAdvanced", true).unwrap();
        assert_eq!(settings["categories"]["Network"][0]["isFavorite"], true);
        assert_eq!(
            settings["categories"]["Favorites"][0]["name"],
            "wifiAdvanced"
        );
        set_dashboard_favorite_in_settings(&mut settings, "wifiAdvanced", false).unwrap();
        assert_eq!(settings["categories"]["Network"][0]["isFavorite"], false);
        assert_eq!(
            settings["categories"]["Favorites"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
        assert!(set_dashboard_favorite_in_settings(&mut settings, "missing", true).is_err());
    }

    #[test]
    fn dashboard_v1_manifest_names_are_normalized_to_v2() {
        let raw = serde_json::json!({
            "version": "V1",
            "categories": {
                "Network": [{ "manifest": "internal_wifiAdvanced" }],
                "Favorites": []
            }
        });

        let settings = sanitize_dashboard_settings(&raw).unwrap();
        assert_eq!(settings["version"], "V2");
        assert_eq!(settings["categories"]["Network"][0]["name"], "wifiAdvanced");
        assert!(
            settings["categories"]["Network"][0]
                .get("isFavorite")
                .is_none()
        );
        assert!(sanitize_dashboard_settings(&serde_json::json!({ "invalid": true })).is_err());
    }

    #[test]
    fn dashboard_favorite_change_preserves_implicit_default_favorites() {
        let raw = serde_json::json!({
            "version": "V1",
            "categories": {
                "Network": [
                    { "name": "wifiAdvanced", "index": 0, "isFavorite": false },
                    { "name": "internetState", "index": 1 }
                ],
                "Favorites": []
            }
        });
        let mut settings = sanitize_dashboard_settings(&raw).unwrap();

        set_dashboard_favorite_in_settings(&mut settings, "wifiAdvanced", true).unwrap();

        assert_eq!(settings["categories"]["Network"][0]["isFavorite"], true);
        assert!(
            settings["categories"]["Network"][1]
                .get("isFavorite")
                .is_none()
        );
    }

    #[test]
    fn admin_password_state_exposes_only_nonsecret_state() {
        let status = serde_json::json!({
            "name": "admin",
            "passwordstate": "factoryDefault",
            "password": "must-not-leak",
            "passwordHash": "also-must-not-leak"
        });
        assert_eq!(
            sanitize_admin_password_state(&status).unwrap(),
            serde_json::json!({ "state": "factoryDefault" })
        );
        assert_eq!(
            sanitize_admin_password_state(&serde_json::json!({ "passwordstate": "unexpected" }))
                .unwrap(),
            serde_json::json!({ "state": "unknown" })
        );
        assert!(sanitize_admin_password_state(&serde_json::json!({})).is_err());
    }

    #[test]
    fn interface_language_is_validated_against_firmware_languages() {
        assert_eq!(
            sanitize_interface_language(&serde_json::json!("pl")).unwrap(),
            "pl"
        );
        assert!(sanitize_interface_language(&serde_json::json!("fr")).is_err());
        assert!(sanitize_interface_language(&serde_json::json!({})).is_err());
    }

    #[test]
    fn dashboard_connection_state_is_allowlisted() {
        let state = serde_json::json!({
            "ConnectionState": "Connected",
            "LinkType": "gpon",
            "IPAddress": "private",
            "password": "private"
        });
        let clean = sanitize_internet_dashboard_state(&state).unwrap();
        assert_eq!(clean["available"], true);
        assert_eq!(clean["linkMode"], "gpon");
        assert!(!clean.to_string().contains("private"));
        let disconnected = sanitize_internet_dashboard_state(&serde_json::json!({
            "ConnectionState": "Disconnected"
        }))
        .unwrap();
        assert_eq!(disconnected["available"], false);
        assert!(disconnected.get("linkMode").is_none());
        assert!(sanitize_internet_dashboard_state(&serde_json::json!({})).is_err());
    }

    #[test]
    fn repeater_schedule_state_matches_dashboard_tile() {
        assert!(
            sanitize_repeater_schedule_state(&serde_json::json!({
                "scheduleInfo": [{ "enable": true }]
            }))
            .unwrap()
        );
        assert!(
            !sanitize_repeater_schedule_state(&serde_json::json!({
                "scheduleInfo": []
            }))
            .unwrap()
        );
        assert!(
            sanitize_repeater_schedule_state(&serde_json::json!({
                "scheduleInfo": [{ "enable": "yes" }]
            }))
            .is_err()
        );
    }

    #[test]
    fn power_schedules_only_return_known_profile_states() {
        let raw = serde_json::json!({
            "LED": { "Activate": true, "Scheduler": { "Type": "Weekly" }, "password": "private" },
            "WiFi": { "Activate": false, "Scheduler": { "Type": "Weekly" } },
            "unknown": { "Activate": true, "Scheduler": { "Type": "Weekly" } }
        });
        let clean = sanitize_dashboard_power_schedules(&raw).unwrap();
        assert_eq!(clean["LED"], true);
        assert_eq!(clean["WiFi"], false);
        assert!(clean.get("unknown").is_none());
        assert!(!clean.to_string().contains("private"));
        assert!(sanitize_dashboard_power_schedules(&serde_json::json!({ "unknown": {} })).is_err());
    }

    #[test]
    fn wifi_iot_status_requires_boolean_access_point_states() {
        assert!(
            sanitize_wifi_iot_enabled(&serde_json::json!({
                "base": { "vap2g0sec": { "Enable": false }, "vap5g0sec": { "Enable": true } },
                "wlanvap": { "vap2g0sec": { "SSID": "private" }, "vap5g0sec": { "SSID": "private" } }
            }))
            .unwrap()
        );
        assert!(
            !sanitize_wifi_iot_enabled(&serde_json::json!({
                "base": { "vap2g0sec": { "Enable": false } },
                "wlanvap": { "vap2g0sec": { "SSID": "private" } }
            }))
            .unwrap()
        );
        assert!(
            sanitize_wifi_iot_enabled(&serde_json::json!({
                "base": { "vap2g0sec": { "Enable": "true" } },
                "wlanvap": { "vap2g0sec": { "SSID": "private" } }
            }))
            .is_err()
        );
    }

    #[test]
    fn iptv_status_maps_web_ui_state_string() {
        assert!(
            !sanitize_television_dashboard_state(
                &serde_json::json!({ "IPTVStatus": "Unavailable" })
            )
            .unwrap()
        );
        assert!(
            sanitize_television_dashboard_state(&serde_json::json!({ "IPTVStatus": "Enabled" }))
                .unwrap()
        );
        assert!(
            sanitize_television_dashboard_state(&serde_json::json!({ "IPTVStatus": "bogus" }))
                .is_err()
        );
        assert!(sanitize_television_dashboard_state(&serde_json::json!({})).is_err());
    }

    #[test]
    fn voice_dashboard_summary_hides_call_details() {
        let trunks = serde_json::json!([{ "enable": "Enabled", "username": "private" }]);
        let calls = serde_json::json!([
            { "type": "missed", "phoneNumber": "+48123456789" },
            { "type": "outgoing", "phoneNumber": "+48987654321" },
            { "type": "missed", "phoneNumber": "+48000000000" }
        ]);
        let clean = sanitize_voice_dashboard_status(&trunks, &calls).unwrap();
        assert_eq!(clean["lineEnabled"], true);
        assert_eq!(clean["missedCalls"], 2);
        assert!(!clean.to_string().contains("private"));
        assert!(!clean.to_string().contains("+48"));
        assert!(sanitize_voice_dashboard_status(&trunks, &serde_json::json!({})).is_err());
    }

    #[test]
    fn unknown_empty_list_response_is_not_mistaken_for_no_rules() {
        let unknown = serde_json::json!({ "metadata": { "items": [] } });
        assert!(sanitize_pinholes(&unknown).is_err());
        assert!(sanitize_dynamic_dns_hosts(&unknown).is_err());
        assert!(
            sanitize_dynamic_dns_hosts(&serde_json::json!({
                "hosts": [{ "hostname": { "password": "private" } }]
            }))
            .is_err()
        );
        assert!(
            sanitize_dynamic_dns_hosts(&serde_json::json!({
                "hosts": [
                    { "hostname": "known.example.net", "service": "NoIP" },
                    { "hostname": 12, "service": "unknown" }
                ]
            }))
            .is_err()
        );
        assert!(
            sanitize_pinholes(&serde_json::json!({
                "rules": [{ "destinationIPAddress": { "token": "private" } }]
            }))
            .is_err()
        );
        assert!(
            sanitize_pinholes(&serde_json::json!({
                "rules": [
                    { "id": "known", "destinationIPAddress": "fd12::1" },
                    { "id": "unknown", "destinationPort": "443" }
                ]
            }))
            .is_err()
        );
        assert_eq!(
            sanitize_pinholes(&serde_json::json!({ "Pinhole": [] })).unwrap(),
            serde_json::json!([])
        );
        assert_eq!(
            sanitize_dynamic_dns_hosts(&serde_json::json!({ "hosts": [] })).unwrap(),
            serde_json::json!([])
        );
    }

    #[test]
    fn rejected_mutation_response_is_not_reported_as_success() {
        assert!(ensure_mutation_succeeded(&serde_json::json!(false), "write").is_err());
        assert!(
            ensure_mutation_succeeded(&serde_json::json!({ "success": false }), "write").is_err()
        );
        assert!(ensure_mutation_succeeded(&serde_json::json!(true), "write").is_ok());
        assert!(
            ensure_mutation_succeeded(&serde_json::json!({ "status": "done" }), "write").is_ok()
        );
    }

    #[test]
    fn ntp_server_output_contains_only_server_addresses() {
        let servers = serde_json::json!({
            "servers": {
                "first": "ntp.example.net",
                "second": "192.0.2.10"
            },
            "password": "must-not-leak"
        });

        assert_eq!(
            sanitize_ntp_servers(&servers),
            serde_json::json!(["ntp.example.net", "192.0.2.10"])
        );
    }

    #[test]
    fn backup_status_exposes_only_last_configuration_date() {
        let status = serde_json::json!({
            "ConfigDate": "2026-10-31T12:00:00Z",
            "Password": "must-not-leak",
            "Account": "private"
        });

        assert_eq!(
            sanitize_backup_status(&status),
            serde_json::json!({ "ConfigDate": "2026-10-31T12:00:00Z" })
        );
    }
}
