use clap::{Parser, Subcommand, ValueEnum};
use std::net::Ipv6Addr;

mod commands;

#[derive(Debug, Parser)]
#[command(name = "funbox7", version, about = "CLI для Orange Funbox 7")]
pub struct Cli {
    /// Адрес роутера (без http://), по умолчанию 192.168.1.1
    #[arg(
        long,
        env = "FUNBOX_HOST",
        default_value = "192.168.1.1",
        global = true
    )]
    pub host: String,

    /// Имя пользователя веб-панели
    #[arg(long, env = "FUNBOX_USER", default_value = "admin", global = true)]
    pub user: String,

    /// Не спрашивать подтверждение перед изменением настроек
    #[arg(long, global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Краткая сводка: модель, интернет и Wi-Fi
    Status,
    /// Модель, прошивка, время работы и число перезагрузок
    Info,
    /// Состояние световых индикаторов роутера
    Leds,
    /// Список пользователей панели без секретных полей
    Users,
    /// Список поддерживаемых DynDNS-провайдеров
    DdnsServices,
    /// Список DynDNS-хостов (пароли и имена пользователей не выводятся)
    DdnsHosts,
    /// Добавить DynDNS-хост; пароль запрашивается скрытым вводом
    DdnsAdd {
        service: String,
        hostname: String,
        username: String,
    },
    /// Удалить DynDNS-хост
    DdnsRemove { hostname: String },
    /// Статус Wi-Fi и WPS; `on` или `off` меняет состояние радио
    Wifi {
        #[arg(value_enum)]
        action: Option<WifiAction>,
    },
    /// Статус Open Mode Wi-Fi
    WifiOpenMode,
    /// Показать статус WPS или изменить его с подтверждением
    Wps {
        #[arg(value_enum)]
        action: Option<WifiAction>,
    },
    /// Состояние подключения к интернету
    Wan,
    /// Счётчики переданных и принятых байт на WAN
    WanStats,
    /// Статус интернет-соединения из dashboard
    InternetState,
    /// Список активных DHCP-устройств
    Devices,
    /// Сводный список устройств Wi-Fi, Ethernet, USB и DECT
    AllDevices,
    /// Найти активное устройство по MAC-адресу
    Device { mac: String },
    /// Устройства, ранее подключавшиеся к роутеру
    DeviceHistory,
    /// SSID, состояние точек доступа и подключённые Wi-Fi-клиенты
    Networks,
    /// Расширенная конфигурация Wi-Fi (ключи скрыты)
    WifiConfig,
    /// Список активных Wi-Fi-клиентов через Devices API
    WifiClients,
    /// LAN-адрес и настройки DHCP
    Lan,
    /// Изменить LAN-адрес и DHCP-диапазон (потребуется переподключение)
    LanSet {
        address: std::net::Ipv4Addr,
        netmask: std::net::Ipv4Addr,
        dhcp_min: std::net::Ipv4Addr,
        dhcp_max: std::net::Ipv4Addr,
        #[arg(long, value_enum)]
        dhcp: DhcpAction,
    },
    /// Сводные параметры LAN-интерфейсов
    NetworkDetails,
    /// WAN, IPv6, VLAN, MTU и сведения PPP провайдера
    InternetDetails,
    /// Список закреплённых DHCP-адресов
    Leases,
    /// Текущие DHCP-аренды (динамические и зарезервированные)
    CurrentLeases,
    /// Параметры DHCP-пула, включая срок аренды и DNS
    DhcpSettings,
    /// Изменить срок действия DHCP-адреса (с подтверждением)
    DhcpLeaseTime { seconds: u32 },
    /// Текущий уровень firewall или задать новый
    Firewall {
        #[arg(value_enum)]
        level: Option<FirewallLevel>,
    },
    /// Показать или задать уровень firewall для IPv6
    FirewallIpv6 {
        #[arg(value_enum)]
        level: Option<FirewallLevel>,
    },
    /// Статус удалённого доступа к панели
    RemoteAccess,
    /// Отключить управление роутером из Интернета
    RemoteAccessDisable,
    /// Состояние гостевой Wi-Fi сети; on/off управляют сетью
    Guest {
        #[arg(value_enum)]
        action: Option<GuestAction>,
    },
    /// Энергосбережение и режим питания
    Energy,
    /// Настроенные пробросы портов
    Forwards,
    /// Создать проброс порта с подтверждением
    ForwardAdd {
        name: String,
        ip: String,
        external_port: u16,
        internal_port: u16,
        #[arg(long, value_enum, default_value = "tcp")]
        protocol: ForwardProtocol,
    },
    /// Удалить проброс порта по имени
    ForwardRemove { name: String },
    /// Пробросы портов, созданные UPnP
    UpnpForwards,
    /// Показать или изменить доступность UPnP-шлюза
    Upnp {
        #[arg(value_enum)]
        action: Option<WifiAction>,
    },
    /// Детали firewall: ping, DMZ и пользовательские правила
    FirewallDetails,
    /// Читать или менять ответ firewall на ping из Интернета
    FirewallPing {
        #[arg(value_enum)]
        action: Option<WifiAction>,
    },
    /// Текущее правило DMZ
    Dmz,
    /// Направить весь входящий трафик на частный LAN-адрес
    DmzSet { destination: std::net::Ipv4Addr },
    /// Удалить правило DMZ
    DmzClear,
    /// Список правил входящего доступа IPv6
    Ipv6Pinholes,
    /// Открыть порт извне на IPv6-устройстве (потребуется подтверждение)
    Ipv6PinholeAdd {
        name: String,
        destination: Ipv6Addr,
        mac: String,
        port: u16,
        #[arg(long, value_enum, default_value = "tcp")]
        protocol: PinholeProtocol,
    },
    /// Удалить правило входящего доступа IPv6 по имени
    Ipv6PinholeRemove { name: String },
    /// Добавить DHCP-резервацию для MAC и IPv4
    Reserve { mac: String, ip: String },
    /// Удалить DHCP-резервацию по MAC
    Unreserve { mac: String },
    /// Перезагрузить роутер
    Reboot,
    /// Подробная статистика Wi-Fi (байты приёма/передачи)
    WifiStats,
    /// Радиомодули Wi-Fi, диапазоны и доступные каналы
    WifiRadios,
    /// Расписание включения и выключения Wi-Fi
    WifiSchedule,
    /// Статус расписания Wi-Fi-репитера
    RepeaterSchedule,
    /// Статус расписаний индикаторов и режимов энергосбережения
    PowerSchedules,
    /// Устройства, которым назначена блокировка доступа в Интернет
    BlockedDevices,
    /// Запретить устройству доступ в Интернет
    BlockDevice { mac: String },
    /// Снять блокировку доступа в Интернет
    UnblockDevice { mac: String },
    /// Параметры IPv6
    Ipv6,
    /// Системное время и часовой пояс
    Time,
    /// Показать или изменить часовой пояс
    TimeZone { timezone: Option<String> },
    /// Список серверов синхронизации времени
    NtpServers,
    /// Дата последней копии конфигурации у оператора
    BackupStatus,
    /// Профили энергосбережения
    EnergyProfiles,
    /// Параметры Smart Wi-Fi / steering
    WifiSteering,
    /// Статус IoT-сервиса роутера
    IotStatus,
    /// Статус IoT Wi-Fi-точек доступа из dashboard
    WifiIotStatus,
    /// Категории, порядок плиток и избранное главного dashboard
    DashboardTiles,
    /// Проверить, установлен ли пароль администратора, без его вывода
    AdminPasswordState,
    /// Показать язык интерфейса, заданный в роутере
    Language,
    /// Добавить или убрать плитку dashboard из избранного
    DashboardFavorite {
        tile: String,
        #[arg(value_enum)]
        action: FavoriteAction,
    },
    /// Состояние IPTV и параметры каналов Orange TV
    Television,
    /// Статус IPTV из dashboard
    IptvStatus,
    /// Конфигурация телефонии; пароли в ответе маскируются
    Voice,
    /// Сводка телефонной линии и числа пропущенных звонков без деталей журнала
    PhoneStatus,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum WifiAction {
    On,
    Off,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum GuestAction {
    On,
    Off,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DhcpAction {
    On,
    Off,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum FirewallLevel {
    Low,
    Medium,
    High,
    Custom,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ForwardProtocol {
    Tcp,
    Udp,
    Both,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum PinholeProtocol {
    Tcp,
    Udp,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum FavoriteAction {
    On,
    Off,
}

impl PinholeProtocol {
    pub(crate) fn as_api_value(self) -> &'static str {
        match self {
            Self::Tcp => "6",
            Self::Udp => "17",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
        }
    }
}

impl ForwardProtocol {
    pub(crate) fn as_api_value(self) -> &'static str {
        match self {
            Self::Tcp => "6",
            Self::Udp => "17",
            Self::Both => "6,17",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Tcp => "TCP",
            Self::Udp => "UDP",
            Self::Both => "TCP+UDP",
        }
    }
}

impl FirewallLevel {
    pub(crate) fn as_api_value(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Custom => "Custom",
        }
    }
}

pub fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let password = commands::password()?;
    let mut router = crate::core::Router::login(&cli.host, &cli.user, &password)?;
    drop(password);

    let result = commands::execute(&cli, &router);
    router.logout();
    result
}
