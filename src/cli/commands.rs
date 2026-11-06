use std::{
    io::{self, Write},
    net::Ipv4Addr,
    str::FromStr,
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

use crate::{
    cli::{Cli, Command, DhcpAction, GuestAction, WifiAction},
    core::Router,
};

pub fn password() -> Result<String> {
    if let Ok(value) = std::env::var("FUNBOX_PASSWORD")
        && !value.is_empty()
    {
        return Ok(value);
    }
    rpassword::prompt_password("Пароль роутера: ").context("не удалось прочитать пароль")
}

fn confirm_wifi_change(enabled: bool) -> Result<bool> {
    if enabled {
        print!("Включить Wi-Fi на роутере? [д/Н] ");
    } else {
        print!("Выключить Wi-Fi? Подключённые по Wi-Fi устройства потеряют связь. [д/Н] ");
    }
    io::stdout()
        .flush()
        .context("не удалось вывести запрос подтверждения")?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("не удалось прочитать подтверждение")?;
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "д" | "да" | "y" | "yes"
    ))
}

fn confirm_action(prompt: &str) -> Result<bool> {
    print!("{prompt} [д/Н] ");
    io::stdout()
        .flush()
        .context("не удалось вывести запрос подтверждения")?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .context("не удалось прочитать подтверждение")?;
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "д" | "да" | "y" | "yes"
    ))
}

fn show_info(value: &Value) {
    println!(
        "Модель: {}",
        value
            .get("ProductClass")
            .and_then(Value::as_str)
            .unwrap_or("неизвестно")
    );
    println!(
        "Прошивка: {}",
        value
            .get("SoftwareVersion")
            .and_then(Value::as_str)
            .unwrap_or("неизвестна")
    );
    if let Some(version) = value
        .get("AdditionalSoftwareVersion")
        .and_then(Value::as_str)
    {
        println!("Доп. версия: {version}");
    }
}

fn show_wifi(value: &Value) {
    print!("Wi-Fi: ");
    match value.get("Enable").and_then(Value::as_bool) {
        Some(true) => println!("включён"),
        Some(false) => println!("выключен"),
        None => println!("состояние неизвестно"),
    }
    if let Some(active) = value.get("Status").and_then(Value::as_bool) {
        println!("Радио активно: {}", if active { "да" } else { "нет" });
    }
    if let Some(state) = value.get("CurrentState").and_then(Value::as_str) {
        println!("Состояние: {state}");
    }
    if let Some(scheduled) = value.get("SchedulingEnabled").and_then(Value::as_bool) {
        println!(
            "Расписание: {}",
            if scheduled {
                "включено"
            } else {
                "выключено"
            }
        );
    }
}

fn string_field<'a>(value: &'a Value, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .unwrap_or("—")
}

fn show_devices(value: &Value, summary: &str) -> Result<()> {
    let devices = match value {
        Value::Array(devices) => devices.iter().collect::<Vec<_>>(),
        Value::Object(devices) => devices.values().collect::<Vec<_>>(),
        _ => bail!("роутер вернул неожиданный формат списка устройств"),
    };

    println!(
        "{:<26} {:<16} {:<18} {:<16} СЕТЬ",
        "ИМЯ", "IP", "MAC", "ТИП"
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
        let mac = string_field(device, &["PhysAddress"]);
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

fn show_networks(value: &Value) -> Result<()> {
    let vaps = value
        .get("wlanvap")
        .and_then(Value::as_object)
        .ok_or_else(|| anyhow!("роутер не вернул список Wi-Fi точек доступа"))?;

    for (index, (vap_id, vap)) in vaps.iter().enumerate() {
        if index > 0 {
            println!();
        }
        let band = if vap_id.starts_with("vap2g") {
            "2.4 ГГц"
        } else if vap_id.starts_with("vap5g") {
            "5 ГГц"
        } else if vap_id.starts_with("vap6g") {
            "6 ГГц"
        } else {
            vap_id
        };
        println!("{} ({band})", string_field(vap, &["SSID"]));
        println!("  интерфейс: {vap_id}");
        println!("  состояние: {}", string_field(vap, &["VAPStatus"]));
        let count = vap
            .get("ActiveAssociatedDeviceNumberOfEntries")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        println!("  подключено устройств: {count}");

        if let Some(associated) = vap.get("AssociatedDevice").and_then(Value::as_object) {
            let active = associated
                .values()
                .filter(|device| device.get("Active").and_then(Value::as_bool) == Some(true))
                .collect::<Vec<_>>();
            if !active.is_empty() {
                println!("  клиенты:");
                for device in active {
                    let mac = string_field(device, &["MACAddress"]);
                    let signal = device
                        .get("SignalStrength")
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "—".to_owned());
                    let mode = string_field(device, &["OperatingStandard", "Mode"]);
                    println!("    {mac:<18} сигнал {signal} dBm, {mode}");
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
        .ok_or_else(|| anyhow!("роутер вернул неожиданный формат DHCP-резерваций"))?;
    if leases.is_empty() {
        println!("Закреплённых DHCP-адресов нет.");
        return Ok(());
    }
    println!("{:<18} {:<18} ИМЯ", "IP", "MAC");
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

fn show_guest(value: &Value) {
    for key in ["Enable", "Status"] {
        if let Some(enabled) = value.get(key).and_then(Value::as_bool) {
            println!(
                "{}: {}",
                if key == "Enable" {
                    "Гостевая Wi-Fi"
                } else {
                    "Активна"
                },
                if enabled { "да" } else { "нет" }
            );
        }
    }
    for (key, label) in [
        ("ActivationTimeout", "Таймаут активации"),
        ("ActivationDuration", "Длительность"),
        ("StartTime", "Время запуска"),
        ("ValidTime", "Время окончания"),
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
    println!("\nПараметры гостевой сети (ключи скрыты):");
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
        bail!("неверный MAC-адрес; ожидается формат AA:BB:CC:DD:EE:FF");
    }
    Ok(value.to_ascii_uppercase())
}

fn validate_ipv4(value: &str) -> Result<&str> {
    Ipv4Addr::from_str(value).context("ожидается корректный IPv4-адрес")?;
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
    serde_json::to_string_pretty(&redact(value)).context("не удалось подготовить JSON для вывода")
}

fn redact(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut result = serde_json::Map::new();
            for (key, value) in object {
                if is_sensitive_key(key) {
                    result.insert(key.clone(), Value::String("[СКРЫТО]".to_owned()));
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
        Command::Wan => {
            let status = router.wan_status()?;
            match status.as_bool() {
                Some(true) => println!("WAN: доступен"),
                Some(false) => println!("WAN: недоступен"),
                None => println!("WAN: {}", status),
            }
        }
        Command::Wifi { action: None } => show_wifi(&router.wifi_status()?),
        Command::Wifi {
            action: Some(action),
        } => {
            let enabled = matches!(action, WifiAction::On);
            if !cli.yes && !confirm_wifi_change(enabled)? {
                println!("Отменено.");
                return Ok(());
            }
            // The Funbox API requires both fields: Enable and Status.
            router.set_wifi(enabled)?;
            println!("Команда отправлена.");
            show_wifi(&router.wifi_status()?);
        }
        Command::Devices => {
            let devices = router.devices()?;
            show_devices(&devices, "Активных DHCP-устройств")?;
        }
        Command::DeviceHistory => {
            let devices = router.device_history()?;
            show_devices(&devices, "Записей об устройствах")?;
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
        Command::WifiSteering => {
            let steering = router.wifi_steering()?;
            println!("{}", pretty_redacted(&steering)?);
        }
        Command::IotStatus => {
            let status = router.iot_status()?;
            println!("{}", pretty_redacted(&status)?);
        }
        Command::Television => {
            let television = router.television()?;
            println!("{}", pretty_redacted(&television)?);
        }
        Command::Voice => {
            let voice = router.voice()?;
            println!("{}", pretty_redacted(&voice)?);
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
            let dhcp_enabled = matches!(dhcp, DhcpAction::On);
            let prompt = format!(
                "Установить LAN {address}/{netmask}, DHCP {} ({dhcp_min}–{dhcp_max})? При смене адреса соединение с роутером может оборваться.",
                if dhcp_enabled {
                    "включён"
                } else {
                    "выключен"
                }
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Отменено.");
                return Ok(());
            }
            router.set_lan(*address, *netmask, dhcp_enabled, *dhcp_min, *dhcp_max)?;
            println!(
                "Настройки отправлены. Если LAN-адрес изменён, подключайся к роутеру по адресу http://{address}."
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
        Command::Firewall { level: None } => {
            let level = router.firewall_level()?;
            println!("Уровень firewall: {level}");
        }
        Command::Firewall { level: Some(level) } => {
            let level = level.as_api_value();
            let prompt = format!("Установить уровень firewall {level}?");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Отменено.");
                return Ok(());
            }
            router.set_firewall_level(level)?;
            let current = router.firewall_level()?;
            println!("Текущий уровень firewall: {current}");
        }
        Command::Guest { action: None } => show_guest_page(router)?,
        Command::Guest {
            action: Some(action),
        } => {
            let enabled = matches!(action, GuestAction::On);
            let prompt = if enabled {
                "Включить гостевую Wi-Fi без таймера отключения?"
            } else {
                "Выключить гостевую Wi-Fi сеть? Подключённые к ней устройства потеряют связь."
            };
            if !cli.yes && !confirm_action(prompt)? {
                println!("Отменено.");
                return Ok(());
            }
            router.set_guest(enabled)?;
            println!("Команда отправлена.");
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
                bail!("номер порта должен быть в диапазоне 1–65535");
            }
            router.validate_lan_destination(destination_ip)?;
            let protocol_value = protocol.as_api_value();
            let prompt = format!(
                "Открыть порт {external_port}/{} на {destination_ip}:{internal_port} с доступом из Интернета?",
                protocol.label()
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Отменено.");
                return Ok(());
            }
            router.add_forward(
                name,
                destination_ip,
                *external_port,
                *internal_port,
                protocol_value,
            )?;
            println!("Проброс порта создан и сохранён.");
        }
        Command::ForwardRemove { name } => {
            let prompt =
                format!("Удалить проброс «{name}»? Входящий доступ через это правило прекратится.");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Отменено.");
                return Ok(());
            }
            router.remove_forward(name)?;
            println!("Правило проброса удалено.");
        }
        Command::UpnpForwards => {
            let forwards = router.upnp_forwards()?;
            println!("{}", pretty_redacted(&forwards)?);
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
                "Разрешить ответы firewall на ping из Интернета (IPv4 и IPv6)?"
            } else {
                "Запретить ответы firewall на ping из Интернета (IPv4 и IPv6)?"
            };
            if !cli.yes && !confirm_action(prompt)? {
                println!("Отменено.");
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
                "ВНИМАНИЕ: направить весь входящий интернет-трафик на {destination}? Это уменьшит защиту этого устройства."
            );
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Отменено.");
                return Ok(());
            }
            let result = router.set_dmz(*destination)?;
            println!("{}", pretty_redacted(&result)?);
        }
        Command::DmzClear => {
            if !cli.yes
                && !confirm_action(
                    "Удалить правило DMZ и прекратить перенаправление всего входящего трафика?",
                )?
            {
                println!("Отменено.");
                return Ok(());
            }
            let result = router.clear_dmz()?;
            println!("{}", pretty_redacted(&result)?);
        }
        Command::Ipv6 => {
            let settings = router.ipv6()?;
            println!("{}", pretty_redacted(&settings)?);
        }
        Command::Time => {
            let time = router.time()?;
            println!("{}", pretty_redacted(&time)?);
        }
        Command::Reserve { mac, ip } => {
            let mac = normalize_mac(mac)?;
            let ip = validate_ipv4(ip)?;
            let prompt = format!("Закрепить адрес {ip} за устройством {mac}?");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Отменено.");
                return Ok(());
            }
            router.add_lease(&mac, ip)?;
            println!("DHCP-резервация добавлена.");
        }
        Command::Unreserve { mac } => {
            let mac = normalize_mac(mac)?;
            let prompt = format!("Удалить DHCP-резервацию для {mac}?");
            if !cli.yes && !confirm_action(&prompt)? {
                println!("Отменено.");
                return Ok(());
            }
            router.remove_lease(&mac.to_ascii_lowercase())?;
            println!("DHCP-резервация удалена.");
        }
        Command::Reboot => {
            if !cli.yes
                && !confirm_action("Перезагрузить роутер? Связь прервётся на несколько минут.")?
            {
                println!("Отменено.");
                return Ok(());
            }
            router.reboot()?;
            println!("Команда перезагрузки отправлена.");
        }
        Command::Status => {
            show_info(&router.info()?);
            let wan = router.wan_status()?;
            match wan.as_bool() {
                Some(true) => println!("WAN: доступен"),
                Some(false) => println!("WAN: недоступен"),
                None => println!("WAN: {}", wan),
            }
            show_wifi(&router.wifi_status()?);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{normalize_mac, pretty_redacted, validate_ipv4};

    #[test]
    fn normalizes_mac_addresses() {
        assert_eq!(
            normalize_mac("aa:bb:cc:dd:ee:ff").unwrap(),
            "AA:BB:CC:DD:EE:FF"
        );
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
        assert_eq!(output.matches("[СКРЫТО]").count(), 3);
    }
}
