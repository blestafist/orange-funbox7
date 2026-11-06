use clap::{Parser, Subcommand, ValueEnum};

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
    /// Модель и версия прошивки
    Info,
    /// Статус Wi-Fi; `on` или `off` меняет его состояние
    Wifi {
        #[arg(value_enum)]
        action: Option<WifiAction>,
    },
    /// Состояние подключения к интернету
    Wan,
    /// Список активных DHCP-устройств
    Devices,
    /// SSID, состояние точек доступа и подключённые Wi-Fi-клиенты
    Networks,
    /// LAN-адрес и настройки DHCP
    Lan,
    /// Список закреплённых DHCP-адресов
    Leases,
    /// Текущий уровень firewall или задать новый
    Firewall {
        #[arg(value_enum)]
        level: Option<FirewallLevel>,
    },
    /// Состояние гостевой Wi-Fi сети
    Guest,
    /// Энергосбережение и режим питания
    Energy,
    /// Настроенные пробросы портов
    Forwards,
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
    /// Параметры IPv6
    Ipv6,
    /// Системное время и часовой пояс
    Time,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum WifiAction {
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
