use std::net::Ipv4Addr;

use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};

use super::RouterClient;

/// Общие операции Funbox, используемые CLI и будущими интерфейсами (например, MCP).
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
        Ok(json!({ "wan_status": wan, "ipv6": ipv6, "vlan_id": vlan_id }))
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
        let ping = self.client.call(
            "Firewall",
            "getRespondToPing",
            json!({ "sourceInterface": "data" }),
        )?;
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

#[cfg(test)]
mod tests {
    use super::{find_forward, validate_forward_name, validate_forward_protocol};

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
}
