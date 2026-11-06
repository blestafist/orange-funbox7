use std::{
    io::{self, Write},
    net::Ipv4Addr,
    str::FromStr,
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

use crate::{
    cli::{Cli, Command, DhcpAction, FavoriteAction, GuestAction, WifiAction},
    core::Router,
};

pub fn password() -> Result<String> {
    if let Ok(value) = std::env::var("FUNBOX_PASSWORD")
        && !value.is_empty()
    {
        return Ok(value);
    }
    rpassword::prompt_password("Router password: ").context("failed to read password")
}

fn confirm_wifi_change(enabled: bool) -> Result<bool> {
    if enabled {
        print!("Enable Wi-Fi on the router? [y/N] ");
    } else {
        print!("Disable Wi-Fi? Devices connected over Wi-Fi will lose connectivity. [y/N] ");
    }
    io::stdout()
        .flush()
        .context("failed to display confirmation prompt")?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("failed to read confirmation")?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn confirm_action(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N] ");
    io::stdout()
        .flush()
        .context("failed to display confirmation prompt")?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("failed to read confirmation")?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn show_info(value: &Value) {
    println!(
        "Model: {}",
        value
            .get("ProductClass")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    );
    println!(
        "Firmware: {}",
        value
            .get("SoftwareVersion")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
    );
    if let Some(version) = value
        .get("AdditionalSoftwareVersion")
        .and_then(Value::as_str)
    {
        println!("Additional version: {version}");
    }
    if let Some(seconds) = unsigned_field(value, &["UpTime", "Uptime"]) {
        println!("Uptime: {}", format_uptime(seconds));
    }
    if let Some(reboots) = unsigned_field(value, &["NumberOfReboots"]) {
        println!("Reboots: {reboots}");
    }
}

fn unsigned_field(value: &Value, fields: &[&str]) -> Option<u64> {
    fields.iter().find_map(|field| {
        value.get(*field).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        })
    })
}

fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else {
        format!("{hours}h {minutes}m")
    }
}

fn show_wifi(value: &Value) {
    print!("Wi-Fi: ");
    match value.get("Enable").and_then(Value::as_bool) {
        Some(true) => println!("enabled"),
        Some(false) => println!("disabled"),
        None => println!("unknown state"),
    }
    if let Some(active) = value.get("Status").and_then(Value::as_bool) {
        println!("Radio active: {}", if active { "yes" } else { "no" });
    }
    if let Some(state) = value.get("CurrentState").and_then(Value::as_str) {
        println!("State: {state}");
    }
    if let Some(scheduled) = value.get("SchedulingEnabled").and_then(Value::as_bool) {
        println!(
            "Schedule: {}",
            if scheduled { "enabled" } else { "disabled" }
        );
    }
    if let Some(wps) = value.get("WPSEnable") {
        println!("WPS: {}", yes_no_field(Some(wps)));
    }
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .unwrap_or("—")
}

fn show_devices(value: &Value, summary: &str) -> Result<()> {
    if !value.is_array() && !value.is_object() {
        bail!("router returned an unexpected device list format");
    }
    let mut devices = Vec::new();
    collect_device_records(value, &mut devices);

    println!(
        "{:<26} {:<16} {:<18} {:<16} NETWORK",
        "NAME", "IP", "MAC", "TYPE"
    );
    println!("{}", "─".repeat(88));
    for device in &devices {
        let fallback_name = device
            .get("BDD")
            .and_then(|bdd| bdd.get("DeviceName"))
            .and_then(Value::as_str)
            .unwrap_or("—");
        let name = device
            .get("Name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .unwrap_or(fallback_name);
        let ip = string_field(device, &["IPAddress"]);
        let mac = string_field(device, &["PhysAddress", "MACAddress"]);
        let kind = string_field(device, &["DeviceType", "DeviceCategory"]);
        let network = string_field(device, &["InterfaceName", "Layer2Interface"]);
        println!(
            "{:<26} {:<16} {:<18} {:<16} {}",
            truncate(name, 25),
            truncate(ip, 15),
            truncate(mac, 17),
            truncate(kind, 15),
            network
        );
    }
    println!("\n{summary}: {}", devices.len());
    Ok(())
}

fn show_device(value: &Value) {
    println!("Name: {}", string_field(value, &["Name", "DeviceName"]));
    println!("IP: {}", string_field(value, &["IPAddress"]));
    println!(
        "MAC: {}",
        string_field(value, &["PhysAddress", "MACAddress"])
    );
    println!(
        "Type: {}",
        string_field(value, &["DeviceType", "DeviceCategory"])
    );
    println!(
        "Connection: {}",
        string_field(value, &["InterfaceName", "Layer2Interface"])
    );
    if let Some(active) = value.get("Active") {
        println!("Active: {}", yes_no_field(Some(active)));
    }
}

fn collect_device_records<'a>(value: &'a Value, records: &mut Vec<&'a Value>) {
    match value {
        Value::Array(entries) => {
            for entry in entries {
                collect_device_records(entry, records);
            }
        }
        Value::Object(fields) if is_device_record(fields) => records.push(value),
        Value::Object(fields) => {
            for entry in fields.values() {
                collect_device_records(entry, records);
            }
        }
        _ => {}
    }
}

fn is_device_record(fields: &serde_json::Map<String, Value>) -> bool {
    [
        "PhysAddress",
        "MACAddress",
        "IPAddress",
        "BDD",
        "InterfaceName",
        "Layer2Interface",
        "DeviceType",
        "DeviceCategory",
    ]
    .iter()
    .any(|key| fields.contains_key(*key))
}

fn show_networks(value: &Value) -> Result<()> {
    let vaps = value
        .get("wlanvap")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("router did not return a list of Wi-Fi access points"))?;

    for (index, (vap_id, vap)) in vaps.iter().enumerate() {
        if index > 0 {
            println!();
        }
        let band = if vap_id.starts_with("vap2g") {
            "2.4 GHz"
        } else if vap_id.starts_with("vap5g") {
            "5 GHz"
        } else if vap_id.starts_with("vap6g") {
            "6 GHz"
        } else {
            vap_id
        };
        println!("{} ({band})", string_field(vap, &["SSID"]));
        println!("  interface: {vap_id}");
        println!("  state: {}", string_field(vap, &["VAPStatus"]));
        let count = vap
            .get("ActiveAssociatedDeviceNumberOfEntries")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        println!("  connected devices: {count}");

        if let Some(associated) = vap.get("AssociatedDevice").and_then(Value::as_object) {
            let active = associated
                .values()
                .filter(|device| device.get("Active").and_then(Value::as_bool) == Some(true))
                .collect::<Vec<_>>();
            if !active.is_empty() {
                println!("  clients:");
                for device in active {
                    let mac = string_field(device, &["MACAddress"]);
                    let signal = device
                        .get("SignalStrength")
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "—".to_owned());
                    let mode = string_field(device, &["OperatingStandard", "Mode"]);
                    println!("    {mac:<18} signal {signal} dBm, {mode}");
                }
            }
        }
    }
    Ok(())
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let prefix = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!(
            "{}…",
            prefix
                .chars()
                .take(max_chars.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        prefix
    }
}

fn show_lan(value: &Value) -> Result<()> {
    println!("{}", pretty_redacted(value)?);
    Ok(())
}

fn show_leases(value: &Value) -> Result<()> {
    let leases = value
        .as_array()
        .ok_or_else(|| anyhow!("router returned an unexpected DHCP reservation format"))?;
    if leases.is_empty() {
        println!("No DHCP addresses are reserved.");
        return Ok(());
    }
    println!("{:<18} {:<18} NAME", "IP", "MAC");
    println!("{}", "─".repeat(60));
    for lease in leases {
        println!(
            "{:<18} {:<18} {}",
            string_field(lease, &["IPAddress", "IP"]),
            string_field(lease, &["MACAddress", "MAC"]),
            string_field(lease, &["HostName", "Hostname", "Name"])
        );
    }
    Ok(())
}

fn show_current_leases(value: &Value) -> Result<()> {
    let leases = value
        .as_array()
        .ok_or_else(|| anyhow!("router returned an unexpected DHCP lease format"))?;
    if leases.is_empty() {
        println!("No current DHCP leases.");
        return Ok(());
    }

    println!(
        "{:<16} {:<18} {:<24} {:<9} {:<9} REMAINING, sec",
        "IP", "MAC", "NAME", "ACTIVE", "RESERVED"
    );
    println!("{}", "─".repeat(100));
    for lease in leases {
        let remaining = lease
            .get("LeaseTimeRemaining")
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| value.to_string())
            })
            .unwrap_or_else(|| "—".to_owned());
        println!(
            "{:<16} {:<18} {:<24} {:<9} {:<9} {}",
            string_field(lease, &["IPAddress"]),
            string_field(lease, &["MACAddress"]),
            truncate(string_field(lease, &["FriendlyName"]), 23),
            yes_no_field(lease.get("Active")),
            yes_no_field(lease.get("Reserved")),
            remaining
        );
    }
    println!("\nCurrent leases: {}", leases.len());
    Ok(())
}

fn show_blocked_devices(value: &Value) -> Result<()> {
    let devices = value
        .as_array()
        .ok_or_else(|| anyhow!("router returned an unexpected blocked devices list format"))?;
    if devices.is_empty() {
        println!("No blocked devices.");
        return Ok(());
    }

    println!("Devices without Internet access:");
    for device in devices {
        println!("  {}", string_field(device, &["ID"]));
    }
    println!("Total: {}", devices.len());
    Ok(())
}

fn show_dynamic_dns_hosts(value: &Value) -> Result<()> {
    let hosts = value
        .as_array()
        .ok_or_else(|| anyhow!("router returned an unexpected DynDNS hosts format"))?;
    if hosts.is_empty() {
        println!("No DynDNS hosts configured.");
        return Ok(());
    }

    println!("{:<40} {:<24} STATUS", "HOST", "PROVIDER");
    println!("{}", "─".repeat(76));
    for host in hosts {
        let status = host
            .get("enable")
            .or_else(|| host.get("Enable"))
            .map(|value| yes_no_field(Some(value)).to_owned())
            .unwrap_or_else(|| string_field(host, &["status", "Status"]).to_owned());
        println!(
            "{:<40} {:<24} {}",
            string_field(host, &["hostname", "HostName", "Hostname"]),
            string_field(host, &["service", "Service"]),
            status
        );
    }
    println!("\nTotal DynDNS hosts: {}", hosts.len());
    Ok(())
}

fn show_dashboard_tiles(value: &Value) -> Result<()> {
    let categories = value
        .get("categories")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("router returned an unexpected dashboard settings format"))?;
    let mut categories = categories.iter().collect::<Vec<_>>();
    categories.sort_by(|left, right| left.0.cmp(right.0));
    for (category, tiles) in categories {
        println!("{category}:");
        let tiles = tiles
            .as_array()
            .ok_or_else(|| anyhow!("dashboard category {category:?} has an invalid format"))?;
        let mut tiles = tiles.iter().collect::<Vec<_>>();
        tiles.sort_by_key(|tile| {
            tile.get("index")
                .and_then(Value::as_u64)
                .unwrap_or(u64::MAX)
        });
        if tiles.is_empty() {
            println!("  (empty)");
        }
        for tile in tiles {
            let favorite = tile.get("isFavorite").and_then(Value::as_bool) == Some(true);
            println!(
                "  {}{}",
                if favorite { "★ " } else { "  " },
                string_field(tile, &["name"])
            );
        }
    }
    Ok(())
}

fn show_power_schedules(value: &Value) -> Result<()> {
    let schedules = value
        .as_object()
        .ok_or_else(|| anyhow!("router returned an unexpected schedules format"))?;
    for (profile, scheduled) in schedules {
        let scheduled = scheduled
            .as_bool()
            .ok_or_else(|| anyhow!("schedule for profile {profile:?} has an invalid format"))?;
        let label = match profile.as_str() {
            "LED" => "Indicators",
            "Light" => "Light power-saving mode",
            "Deep" => "Deep power-saving mode",
            "WiFi" => "Wi-Fi",
            "WiFi5G" => "Wi-Fi 5 GHz",
            _ => bail!("router returned an unknown schedule profile"),
        };
        println!(
            "{label}: {}.",
            if scheduled {
                "schedule enabled"
            } else {
                "schedule disabled"
            }
        );
    }
    Ok(())
}

fn show_ipv6_pinholes(value: &Value) -> Result<()> {
    let pinholes = value
        .as_array()
        .ok_or_else(|| anyhow!("router returned an unexpected IPv6 rules format"))?;
    if pinholes.is_empty() {
        println!("No incoming IPv6 access rules.");
        return Ok(());
    }

    println!(
        "{:<24} {:<40} {:<8} {:<8} ENABLED",
        "NAME", "ADDRESS", "PORT", "PROTOCOL"
    );
    println!("{}", "─".repeat(90));
    for pinhole in pinholes {
        println!(
            "{:<24} {:<40} {:<8} {:<8} {}",
            string_field(pinhole, &["id", "Id", "ID", "description", "Description"]),
            string_field(pinhole, &["destinationIPAddress", "DestinationIPAddress"]),
            string_field(pinhole, &["destinationPort", "DestinationPort"]),
            string_field(pinhole, &["protocol", "Protocol"]),
            yes_no_field(pinhole.get("enable").or_else(|| pinhole.get("Enable")))
        );
    }
    println!("\nTotal rules: {}", pinholes.len());
    Ok(())
}

fn yes_no_field(value: Option<&Value>) -> &'static str {
    match value {
        Some(Value::Bool(true)) => "yes",
        Some(Value::Bool(false)) => "no",
        Some(Value::String(value))
            if matches!(
                value.to_ascii_lowercase().as_str(),
                "true" | "1" | "enabled" | "on"
            ) =>
        {
            "yes"
        }
        Some(Value::String(value))
            if matches!(
                value.to_ascii_lowercase().as_str(),
                "false" | "0" | "disabled" | "off"
            ) =>
        {
            "no"
        }
        _ => "—",
    }
}

fn show_guest(value: &Value) {
    for key in ["Enable", "Status"] {
        if let Some(enabled) = value.get(key).and_then(Value::as_bool) {
            println!(
                "{}: {}",
                if key == "Enable" {
                    "Guest Wi-Fi"
                } else {
                    "Active"
                },
                if enabled { "yes" } else { "no" }
            );
        }
    }
    for (key, label) in [
        ("ActivationTimeout", "Activation timeout"),
        ("ActivationDuration", "Duration"),
        ("StartTime", "Start time"),
        ("ValidTime", "End time"),
    ] {
        if let Some(value) = value.get(key).filter(|value| !value.is_null()) {
            println!("{label}: {value}");
        }
    }
}

fn show_guest_page(router: &Router) -> Result<()> {
    let status = router.guest()?;
    show_guest(&status);
    let details = router.guest_networks()?;
    println!("\nGuest network settings (keys redacted):");
    println!("{}", pretty_redacted(&details)?);
    Ok(())
}

fn normalize_mac(value: &str) -> Result<String> {
    let octets = value.split(':').collect::<Vec<_>>();
    if octets.len() != 6
        || octets
            .iter()
            .any(|octet| octet.len() != 2 || !octet.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        bail!("invalid MAC address; expected format AA:BB:CC:DD:EE:FF");
    }
    let first_octet = u8::from_str_radix(octets[0], 16)?;
    if first_octet & 1 != 0 {
        bail!("a unicast MAC address for a specific device is required");
    }
    Ok(value.to_ascii_uppercase())
}

fn validate_ipv4(value: &str) -> Result<&str> {
    Ipv4Addr::from_str(value).context("expected a valid IPv4 address")?;
    Ok(value)
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("password")
        || key.contains("passphrase")
        || key.contains("secret")
        || key.contains("psk")
        || key.contains("credential")
        || key.contains("preshared")
        || key.contains("pre_shared")
        || key.ends_with("key")
        || key == "pin"
        || key == "pincode"
        || key == "key"
        || key.ends_with("_key")
}

fn pretty_redacted(value: &Value) -> Result<String> {
    serde_json::to_string_pretty(&redact(value)).context("failed to prepare JSON for output")
}

fn redact(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut result = serde_json::Map::new();
            for (key, value) in object {
                if is_sensitive_key(key) {
                    result.insert(key.clone(), Value::String("[REDACTED]".to_owned()));
                } else {
                    result.insert(key.clone(), redact(value));
                }
            }
            Value::Object(result)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact).collect()),
        other => other.clone(),
    }
}

pub fn execute(cli: &Cli, router: &Router) -> Result<()> {
    match &cli.command {
        Command::Info => show_info(&router.info()?),
        Command::Leds => {
            let leds = router.led_status()?;
            println!("{}", pretty_redacted(&leds)?);
        }
        Command::Users => {
            let users = router.users()?;
            println!("{}", pretty_redacted(&users)?);
        }
        Command::DdnsServices => {
            let services = router.dynamic_dns_services()?;
            println!("{}", pretty_redacted(&services)?);
        }
        Command::DdnsHosts => {
            show_dynamic_dns_hosts(&router.dynamic_dns_hosts()?)?;
        }
        Command::DdnsAdd {
            service,
            hostname,
            username,
        } => {
            Router::validate_dynamic_dns_identity(service, hostname, username)?;
            let prompt = format!(
                "Add DynDNS host {hostname} via {service}? Provider credentials will be saved on the router."
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            let password = rpassword::prompt_password("DynDNS password (input hidden): ")
                .context("failed to read DynDNS password")?;
            router.add_dynamic_dns_host(service, hostname, username, &password)?;
            drop(password);
            println!("DynDNS host added.");
        }
        Command::DdnsRemove { hostname } => {
            let prompt = format!("Remove DynDNS host {hostname}?");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.remove_dynamic_dns_host(hostname)?;
            println!("DynDNS host removed.");
        }
        Command::Wan => {
            let status = router.wan_status()?;
            match status.as_bool() {
                Some(true) => println!("WAN: available"),
                Some(false) => println!("WAN: unavailable"),
                None => println!("WAN: {}", status),
            }
        }
        Command::WanStats => {
            let stats = router.wan_stats()?;
            println!("{}", pretty_redacted(&stats)?);
        }
        Command::InternetState => {
            let state = router.internet_dashboard_state()?;
            let available = state.get("available").and_then(Value::as_bool) == Some(true);
            println!(
                "Internet connection: {}.",
                if available {
                    "available"
                } else {
                    "unavailable"
                }
            );
            if let Some(link_mode) = state.get("linkMode").and_then(Value::as_str) {
                println!("Connection type: {link_mode}.");
            }
        }
        Command::Wifi { action: None } => show_wifi(&router.wifi_status()?),
        Command::Wifi {
            action: Some(action),
        } => {
            let enabled = matches!(action, WifiAction::On);
            if !cli.yes && !confirm_wifi_change(enabled)? {
                println!("Cancelled.");
                return Ok(());
            }
            // The Funbox API requires both fields: Enable and Status.
            router.set_wifi(enabled)?;
            println!("Command sent.");
            show_wifi(&router.wifi_status()?);
        }
        Command::WifiOpenMode => {
            let status = router.wifi_open_mode()?;
            println!("{}", pretty_redacted(&status)?);
        }
        Command::Wps { action: None } => {
            let status = router.wifi_status()?;
            match status.get("WPSEnable") {
                Some(value) => println!("WPS: {}", yes_no_field(Some(value))),
                None => println!("Router did not report WPS status."),
            }
        }
        Command::Wps {
            action: Some(action),
        } => {
            let enabled = matches!(action, WifiAction::On);
            let prompt = if enabled {
                "Enable WPS? PIN-based connections may weaken Wi-Fi security."
            } else {
                "Disable WPS and prevent connections via WPS?"
            };
            if !cli.yes && !confirm_action(prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.set_wps(enabled)?;
            let status = router.wifi_status()?;
            match status.get("WPSEnable") {
                Some(value) => println!("WPS: {}", yes_no_field(Some(value))),
                None => println!("Command sent; router did not report WPS status."),
            }
        }
        Command::Devices => {
            let devices = router.devices()?;
            show_devices(&devices, "Active DHCP devices")?;
        }
        Command::AllDevices => {
            let devices = router.all_devices()?;
            show_devices(&devices, "Devices from all available categories")?;
        }
        Command::Device { mac } => {
            let mac = normalize_mac(mac)?;
            show_device(&router.device(&mac)?);
        }
        Command::DeviceHistory => {
            let devices = router.device_history()?;
            show_devices(&devices, "Device records")?;
        }
        Command::Networks => {
            let networks = router.networks()?;
            show_networks(&networks)?;
        }
        Command::WifiConfig => {
            let config = router.wifi_config()?;
            println!("{}", pretty_redacted(&config)?);
        }
        Command::WifiClients => {
            let clients = router.wifi_clients()?;
            println!("{}", pretty_redacted(&clients)?);
        }
        Command::WifiStats => {
            let stats = router.wifi_stats()?;
            println!("{}", pretty_redacted(&stats)?);
        }
        Command::WifiRadios => {
            let radios = router.wifi_radios()?;
            println!("{}", pretty_redacted(&radios)?);
        }
        Command::WifiSchedule => {
            let schedule = router.wifi_schedule()?;
            println!("{}", pretty_redacted(&schedule)?);
        }
        Command::RepeaterSchedule => {
            let enabled = router.repeater_schedule_state()?;
            println!(
                "Wi-Fi repeater schedule: {}.",
                if enabled { "enabled" } else { "disabled" }
            );
        }
        Command::PowerSchedules => {
            show_power_schedules(&router.dashboard_power_schedules()?)?;
        }
        Command::BlockedDevices => {
            let blocked = router.blocked_devices()?;
            show_blocked_devices(&blocked)?;
        }
        Command::BlockDevice { mac } => {
            let mac = normalize_mac(mac)?;
            let prompt = format!(
                "Block Internet access for device {mac}? Access to other local network devices will remain."
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.block_device(&mac)?;
            println!("Internet access restriction enabled for {mac}.");
        }
        Command::UnblockDevice { mac } => {
            let mac = normalize_mac(mac)?;
            let prompt = format!("Restore Internet access for device {mac}?");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.unblock_device(&mac)?;
            println!("Internet access restored for {mac}.");
        }
        Command::WifiSteering => {
            let steering = router.wifi_steering()?;
            println!("{}", pretty_redacted(&steering)?);
        }
        Command::IotStatus => {
            let status = router.iot_status()?;
            println!("{}", pretty_redacted(&status)?);
        }
        Command::WifiIotStatus => {
            let enabled = router.wifi_iot_enabled()?;
            println!(
                "IoT Wi-Fi: {}.",
                if enabled { "enabled" } else { "disabled" }
            );
        }
        Command::DashboardTiles => {
            show_dashboard_tiles(&router.dashboard_settings()?)?;
        }
        Command::AdminPasswordState => {
            let state = router.admin_password_state()?;
            match state.get("state").and_then(Value::as_str) {
                Some("notSet") => println!("Administrator password is not set."),
                Some("factoryDefault") => {
                    println!("The default administrator password is in use.")
                }
                _ => println!("Router reported an unknown administrator password state."),
            }
        }
        Command::Language => {
            let language = router.interface_language()?;
            let name = match language.as_str() {
                "en" => "English",
                "pl" => "Polish",
                _ => unreachable!("language code is validated by Router"),
            };
            println!("Router interface language: {name} ({language}).");
        }
        Command::DashboardFavorite { tile, action } => {
            Router::validate_dashboard_tile_name(tile)?;
            let favorite = matches!(action, FavoriteAction::On);
            let prompt = if favorite {
                format!("Add dashboard tile {tile} to favorites?")
            } else {
                format!("Remove dashboard tile {tile} from dashboard favorites?")
            };
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.set_dashboard_favorite(tile, favorite)?;
            if favorite {
                println!("Dashboard tile {tile} added to favorites.");
            } else {
                println!("Dashboard tile {tile} removed from favorites.");
            }
        }
        Command::Television => {
            let television = router.television()?;
            println!("{}", pretty_redacted(&television)?);
        }
        Command::IptvStatus => {
            let enabled = router.television_dashboard_state()?;
            println!("IPTV: {}.", if enabled { "available" } else { "inactive" });
        }
        Command::Voice => {
            let voice = router.voice()?;
            println!("{}", pretty_redacted(&voice)?);
        }
        Command::PhoneStatus => {
            let status = router.voice_dashboard_status()?;
            let enabled = status.get("lineEnabled").and_then(Value::as_bool) == Some(true);
            let missed_calls = status
                .get("missedCalls")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            println!(
                "Phone line: {}.",
                if enabled { "enabled" } else { "disabled" }
            );
            println!("Missed calls: {missed_calls}.");
        }
        Command::Lan => {
            let lan = router.lan()?;
            show_lan(&lan)?;
        }
        Command::LanSet {
            address,
            netmask,
            dhcp_min,
            dhcp_max,
            dhcp,
        } => {
            Router::validate_lan_settings(*address, *netmask, *dhcp_min, *dhcp_max)?;
            let dhcp_enabled = matches!(dhcp, DhcpAction::On);
            let prompt = format!(
                "Set LAN {address}/{netmask}, DHCP {} ({dhcp_min}–{dhcp_max})? Changing the address may disconnect you from the router.",
                if dhcp_enabled { "enabled" } else { "disabled" }
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.set_lan(*address, *netmask, dhcp_enabled, *dhcp_min, *dhcp_max)?;
            println!(
                "Settings sent. If the LAN address changed, reconnect to the router at http://{address}."
            );
        }
        Command::NetworkDetails => {
            let details = router.network_details()?;
            println!("{}", pretty_redacted(&details)?);
        }
        Command::InternetDetails => {
            let details = router.internet_details()?;
            println!("{}", pretty_redacted(&details)?);
        }
        Command::Leases => {
            let leases = router.leases()?;
            show_leases(&leases)?;
        }
        Command::CurrentLeases => {
            let leases = router.current_leases()?;
            show_current_leases(&leases)?;
        }
        Command::DhcpSettings => {
            let settings = router.dhcp_settings()?;
            println!("{}", pretty_redacted(&settings)?);
        }
        Command::DhcpLeaseTime { seconds } => {
            Router::validate_dhcp_lease_time(*seconds)?;
            let prompt =
                format!("Set DHCP lease time to {seconds} seconds (from 60 seconds to 365 days)?");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.set_dhcp_lease_time(*seconds)?;
            println!("DHCP lease time updated.");
        }
        Command::Firewall { level: None } => {
            let level = router.firewall_level()?;
            println!("Firewall level: {level}");
        }
        Command::Firewall { level: Some(level) } => {
            let level = level.as_api_value();
            let prompt = format!("Set firewall level to {level}?");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.set_firewall_level(level)?;
            let current = router.firewall_level()?;
            println!("Current firewall level: {current}");
        }
        Command::FirewallIpv6 { level: None } => {
            let level = router.firewall_ipv6_level()?;
            println!("IPv6 firewall level: {level}");
        }
        Command::FirewallIpv6 { level: Some(level) } => {
            let level = level.as_api_value();
            let prompt = format!("Set IPv6 firewall level to {level}?");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.set_firewall_ipv6_level(level)?;
            println!("IPv6 firewall level: {}", router.firewall_ipv6_level()?);
        }
        Command::RemoteAccess => {
            let status = router.remote_access_status()?;
            println!("{}", pretty_redacted(&status)?);
        }
        Command::RemoteAccessDisable => {
            if !cli.yes
                && !confirm_action(
                    "Disable remote router management from the Internet? Local access will remain available.",
                )?
            {
                println!("Cancelled.");
                return Ok(());
            }
            router.disable_remote_access()?;
            println!("{}", pretty_redacted(&router.remote_access_status()?)?);
        }
        Command::Guest { action: None } => show_guest_page(router)?,
        Command::Guest {
            action: Some(action),
        } => {
            let enabled = matches!(action, GuestAction::On);
            let prompt = if enabled {
                "Enable guest Wi-Fi without an auto-disable timer?"
            } else {
                "Disable guest Wi-Fi? Connected devices will lose connectivity."
            };
            if !cli.yes && !confirm_action(prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.set_guest(enabled)?;
            println!("Command sent.");
            show_guest_page(router)?;
        }
        Command::Energy => {
            let energy = router.energy()?;
            println!("{}", pretty_redacted(&energy)?);
        }
        Command::EnergyProfiles => {
            let profiles = router.energy_profiles()?;
            println!("{}", pretty_redacted(&profiles)?);
        }
        Command::Forwards => {
            let forwards = router.forwards()?;
            println!("{}", pretty_redacted(&forwards)?);
        }
        Command::ForwardAdd {
            name,
            ip,
            external_port,
            internal_port,
            protocol,
        } => {
            let destination_ip = Ipv4Addr::from_str(validate_ipv4(ip)?)?;
            if *external_port == 0 || *internal_port == 0 {
                bail!("port number must be in the range 1–65535");
            }
            router.validate_lan_destination(destination_ip)?;
            let protocol_value = protocol.as_api_value();
            let prompt = format!(
                "Open port {external_port}/{} on {destination_ip}:{internal_port} for Internet access?",
                protocol.label()
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.add_forward(
                name,
                destination_ip,
                *external_port,
                *internal_port,
                protocol_value,
            )?;
            println!("Port forwarding rule created and saved.");
        }
        Command::ForwardRemove { name } => {
            let prompt = format!(
                "Remove port forwarding rule “{name}”? Incoming access through this rule will stop."
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.remove_forward(name)?;
            println!("Port forwarding rule removed.");
        }
        Command::UpnpForwards => {
            let forwards = router.upnp_forwards()?;
            println!("{}", pretty_redacted(&forwards)?);
        }
        Command::Upnp { action: None } => {
            let status = router.upnp_status()?;
            println!("{}", pretty_redacted(&status)?);
        }
        Command::Upnp {
            action: Some(action),
        } => {
            let enabled = matches!(action, WifiAction::On);
            let prompt = if enabled {
                "Allow devices to automatically open incoming ports via UPnP? This may increase the risk of Internet access."
            } else {
                "Prevent devices from automatically opening ports? Some applications may lose incoming connections."
            };
            if !cli.yes && !confirm_action(prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.set_upnp_enabled(enabled)?;
            let status = router.upnp_status()?;
            println!("{}", pretty_redacted(&status)?);
        }
        Command::FirewallDetails => {
            let details = router.firewall_details()?;
            println!("{}", pretty_redacted(&details)?);
        }
        Command::FirewallPing { action: None } => {
            let policy = router.firewall_ping_response()?;
            println!("{}", pretty_redacted(&policy)?);
        }
        Command::FirewallPing {
            action: Some(action),
        } => {
            let enabled = matches!(action, WifiAction::On);
            let prompt = if enabled {
                "Allow firewall ping responses from the Internet (IPv4 and IPv6)?"
            } else {
                "Block firewall ping responses from the Internet (IPv4 and IPv6)?"
            };
            if !cli.yes && !confirm_action(prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            let result = router.set_firewall_ping_response(enabled)?;
            println!("{}", pretty_redacted(&result)?);
        }
        Command::Dmz => {
            let dmz = router.dmz()?;
            println!("{}", pretty_redacted(&dmz)?);
        }
        Command::DmzSet { destination } => {
            router.validate_lan_destination(*destination)?;
            let prompt = format!(
                "WARNING: route all incoming Internet traffic to {destination}? This will reduce this device’s security."
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            let result = router.set_dmz(*destination)?;
            println!("{}", pretty_redacted(&result)?);
        }
        Command::DmzClear => {
            if !cli.yes
                && !confirm_action("Remove the DMZ rule and stop forwarding all incoming traffic?")?
            {
                println!("Cancelled.");
                return Ok(());
            }
            let result = router.clear_dmz()?;
            println!("{}", pretty_redacted(&result)?);
        }
        Command::Ipv6Pinholes => {
            show_ipv6_pinholes(&router.ipv6_pinholes()?)?;
        }
        Command::Ipv6PinholeAdd {
            name,
            destination,
            mac,
            port,
            protocol,
        } => {
            let mac = normalize_mac(mac)?;
            let protocol_value = protocol.as_api_value();
            Router::validate_ipv6_pinhole(name, *destination, &mac, *port, protocol_value)?;
            let prompt = format!(
                "Open incoming IPv6 port {port}/{} for {destination} (MAC {mac}) to the Internet?",
                protocol.label()
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.add_ipv6_pinhole(name, *destination, &mac, *port, protocol_value)?;
            println!("IPv6 rule created and saved.");
        }
        Command::Ipv6PinholeRemove { name } => {
            Router::validate_ipv6_pinhole_identifier(name)?;
            let prompt =
                format!("Remove IPv6 rule “{name}”? Incoming connections through it will stop.");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.remove_ipv6_pinhole(name)?;
            println!("IPv6 rule removed.");
        }
        Command::Ipv6 => {
            let settings = router.ipv6()?;
            println!("{}", pretty_redacted(&settings)?);
        }
        Command::Time => {
            let time = router.time()?;
            println!("{}", pretty_redacted(&time)?);
        }
        Command::TimeZone { timezone: None } => {
            let time = router.time()?;
            println!(
                "Time zone: {}",
                time.get("timezone").unwrap_or(&Value::Null)
            );
        }
        Command::TimeZone {
            timezone: Some(timezone),
        } => {
            Router::validate_timezone(timezone)?;
            let prompt = format!(
                "Set time zone to {timezone}? This will change displayed times and may affect schedules."
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.set_timezone(timezone)?;
            let time = router.time()?;
            println!(
                "Time zone: {}",
                time.get("timezone").unwrap_or(&Value::Null)
            );
        }
        Command::NtpServers => {
            let servers = router.ntp_servers()?;
            println!("{}", pretty_redacted(&servers)?);
        }
        Command::BackupStatus => {
            let status = router.backup_status()?;
            println!("{}", pretty_redacted(&status)?);
        }
        Command::Reserve { mac, ip } => {
            let mac = normalize_mac(mac)?;
            let ip = validate_ipv4(ip)?;
            let prompt = format!("Reserve address {ip} for device {mac}?");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.add_lease(&mac, ip)?;
            println!("DHCP reservation added.");
        }
        Command::Unreserve { mac } => {
            let mac = normalize_mac(mac)?;
            let prompt = format!("Remove DHCP reservation for {mac}?");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Cancelled.");
                return Ok(());
            }
            router.remove_lease(&mac.to_ascii_lowercase())?;
            println!("DHCP reservation removed.");
        }
        Command::Reboot => {
            if !cli.yes
                && !confirm_action(
                    "Reboot the router? Connectivity will be interrupted for several minutes.",
                )?
            {
                println!("Cancelled.");
                return Ok(());
            }
            router.reboot()?;
            println!("Reboot command sent.");
        }
        Command::Status => {
            show_info(&router.info()?);
            let wan = router.wan_status()?;
            match wan.as_bool() {
                Some(true) => println!("WAN: available"),
                Some(false) => println!("WAN: unavailable"),
                None => println!("WAN: {}", wan),
            }
            show_wifi(&router.wifi_status()?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        collect_device_records, format_uptime, normalize_mac, pretty_redacted, unsigned_field,
        validate_ipv4,
    };

    #[test]
    fn normalizes_mac_addresses() {
        assert_eq!(
            normalize_mac("aa:bb:cc:dd:ee:ff").unwrap(),
            "AA:BB:CC:DD:EE:FF"
        );
        assert!(normalize_mac("01:00:5E:00:00:01").is_err());
        assert!(normalize_mac("aa:bb:cc:dd:ee").is_err());
        assert!(normalize_mac("gg:bb:cc:dd:ee:ff").is_err());
    }

    #[test]
    fn validates_ipv4_addresses() {
        assert_eq!(validate_ipv4("192.168.1.25").unwrap(), "192.168.1.25");
        assert!(validate_ipv4("192.168.1.999").is_err());
        assert!(validate_ipv4("not-an-ip").is_err());
    }

    #[test]
    fn redacts_wifi_secrets_recursively() {
        let input = serde_json::json!({
            "SSID": "guest-net",
            "Security": {
                "PreSharedKey": "do-not-print",
                "KeyPassPhrase": "also-secret",
                "password": "secret"
            }
        });

        let output = pretty_redacted(&input).unwrap();
        assert!(output.contains("guest-net"));
        assert!(!output.contains("do-not-print"));
        assert!(!output.contains("also-secret"));
        assert!(!output.contains("\"secret\""));
        assert_eq!(output.matches("[REDACTED]").count(), 3);
    }

    #[test]
    fn flattens_grouped_device_results() {
        let grouped = serde_json::json!({
            "usb": {
                "printer": {
                    "Name": "Printer",
                    "IPAddress": "192.168.1.20",
                    "PhysAddress": "AA:BB:CC:DD:EE:FF"
                }
            },
            "wifi": [
                {
                    "Name": "Phone",
                    "IPAddress": "192.168.1.21",
                    "MACAddress": "11:22:33:44:55:66"
                }
            ],
            "metadata": { "count": 2 }
        });
        let mut records = Vec::new();
        collect_device_records(&grouped, &mut records);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0]["IPAddress"], "192.168.1.20");
        assert_eq!(records[1]["IPAddress"], "192.168.1.21");
    }

    #[test]
    fn formats_uptime_and_reads_numeric_strings() {
        assert_eq!(format_uptime(90_061), "1d 1h 1m");
        assert_eq!(format_uptime(3_599), "0h 59m");
        let info = serde_json::json!({ "UpTime": "7200" });
        assert_eq!(unsigned_field(&info, &["UpTime", "Uptime"]), Some(7_200));
    }
}
