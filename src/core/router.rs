use std::net::Ipv4Addr;

use anyhow::{Result, anyhow, bail};
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

    pub fn info(&self) -> Result<Value> {
        self.client.call("DeviceInfo", "get", json!({}))
    }

    pub fn wan_status(&self) -> Result<Value> {
        self.client.call("NMC", "getWANStatus", json!({}))
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

    pub fn devices(&self) -> Result<Value> {
        self.client.call(
            "Devices",
            "get",
            json!({ "expression": "edev && physical && dhcp and .Active==true" }),
        )
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
        validate_lan_config(address, netmask, dhcp_min, dhcp_max)?;
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
            bail!("номер порта должен быть в диапазоне 1–65535");
        }
        self.validate_lan_destination(destination_ip)?;

        let current = self.forwards()?;
        if find_forward(&current, name).is_some() {
            bail!("проброс с именем {name:?} уже существует");
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
            .ok_or_else(|| anyhow!("не найден проброс портов с именем {name:?}"))?;
        let protocol = field_string(entry, &["Protocol", "protocol"])
            .ok_or_else(|| anyhow!("в записи проброса нет протокола"))?;
        let internal_port = field_string(entry, &["InternalPort", "internalPort"])
            .ok_or_else(|| anyhow!("в записи проброса нет внутреннего порта"))?;
        let external_port = field_string(entry, &["ExternalPort", "externalPort"])
            .ok_or_else(|| anyhow!("в записи проброса нет внешнего порта"))?;

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
            bail!("DMZ уже настроена; сначала очисти существующее правило");
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
        bail!("имя правила: 1–32 символа, только латиница, цифры, дефис и подчёркивание");
    }
    Ok(())
}

fn validate_forward_protocol(protocol: &str) -> Result<()> {
    if !matches!(protocol, "6" | "17" | "6,17") {
        bail!("протокол должен быть TCP, UDP или both");
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
        bail!("LAN-адрес и DHCP-диапазон должны быть частными IPv4-адресами RFC 1918");
    }

    let address = u32::from(address);
    let mask = u32::from(netmask);
    let prefix = mask.leading_ones();
    if !(1..=30).contains(&prefix) || mask != (u32::MAX << (32 - prefix)) {
        bail!("маска должна быть непрерывной IPv4-маской от /1 до /30");
    }

    let network = address & mask;
    let broadcast = network | !mask;
    if !is_private_network(network, broadcast) {
        bail!("LAN-подсеть должна целиком находиться в частном диапазоне RFC 1918");
    }
    if address == network || address == broadcast {
        bail!("LAN-адрес роутера не может быть адресом сети или broadcast");
    }
    let min = u32::from(dhcp_min);
    let max = u32::from(dhcp_max);
    if min > max {
        bail!("начало DHCP-диапазона не может быть больше его конца");
    }
    if (min & mask) != network || (max & mask) != network {
        bail!("DHCP-диапазон должен находиться в подсети LAN-адреса");
    }
    if min == network || max == broadcast {
        bail!("DHCP-диапазон не должен включать адрес сети или broadcast");
    }
    if (min..=max).contains(&address) {
        bail!("DHCP-диапазон не должен включать LAN-адрес самого роутера");
    }
    Ok(())
}

fn validate_lan_destination(lan: &Value, destination: Ipv4Addr) -> Result<()> {
    if !destination.is_private() {
        bail!("адрес назначения должен находиться в частном IPv4-диапазоне RFC 1918");
    }

    let address = ipv4_field(lan, &["Address", "IPAddress", "LANIPAddress", "IP"])
        .ok_or_else(|| anyhow!("в ответе LAN API не найден IPv4-адрес роутера"))?;
    let netmask = ipv4_field(lan, &["Netmask", "SubnetMask", "LANSubnetMask", "Mask"])
        .ok_or_else(|| anyhow!("в ответе LAN API не найдена IPv4-маска"))?;
    let mask = u32::from(netmask);
    let prefix = mask.leading_ones();
    if !(1..=30).contains(&prefix) || mask != (u32::MAX << (32 - prefix)) {
        bail!("роутер вернул некорректную маску LAN");
    }

    let address = u32::from(address);
    let network = address & mask;
    let broadcast = network | !mask;
    if !is_private_network(network, broadcast) {
        bail!("текущая LAN-подсеть не является частной IPv4-подсетью RFC 1918");
    }
    if address == network || address == broadcast {
        bail!("роутер вернул некорректный LAN-адрес");
    }

    let destination = u32::from(destination);
    if destination & mask != network {
        bail!("адрес назначения должен находиться в текущей LAN-подсети");
    }
    if destination == network || destination == broadcast || destination == address {
        bail!("адрес назначения должен быть другим LAN-адресом хоста");
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

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::{
        find_forward, validate_forward_name, validate_forward_protocol, validate_lan_config,
        validate_lan_destination,
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
}
