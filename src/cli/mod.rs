use clap::{Parser, Subcommand, ValueEnum};
use std::net::Ipv6Addr;

mod commands;

#[derive(Debug, Parser)]
#[command(name = "funbox7", version, about = "CLI for Orange Funbox 7")]
pub struct Cli {
    /// Router address (without http://); defaults to 192.168.1.1
    #[arg(
        long,
        env = "FUNBOX_HOST",
        default_value = "192.168.1.1",
        global = true
    )]
    pub host: String,

    /// Web interface username
    #[arg(long, env = "FUNBOX_USER", default_value = "admin", global = true)]
    pub user: String,

    /// Do not ask for confirmation before changing settings
    #[arg(long, global = true)]
    pub yes: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Brief summary: model, Internet, and Wi-Fi
    Status,
    /// Model, firmware, uptime, and number of reboots
    Info,
    /// Router LED status
    Leds,
    /// List of interface users, excluding secret fields
    Users,
    /// List supported DynDNS providers
    DdnsServices,
    /// List DynDNS hosts (passwords and usernames are not shown)
    DdnsHosts,
    /// Add a DynDNS host; the password is requested with hidden input
    DdnsAdd {
        service: String,
        hostname: String,
        username: String,
    },
    /// Remove a DynDNS host
    DdnsRemove { hostname: String },
    /// Wi-Fi and WPS status; `on` or `off` changes the radio state
    Wifi {
        #[arg(value_enum)]
        action: Option<WifiAction>,
    },
    /// Open Mode Wi-Fi status
    WifiOpenMode,
    /// Show WPS status or change it with confirmation
    Wps {
        #[arg(value_enum)]
        action: Option<WifiAction>,
    },
    /// Internet connection status
    Wan,
    /// WAN bytes sent and received counters
    WanStats,
    /// Internet connection status from the dashboard
    InternetState,
    /// List active DHCP devices
    Devices,
    /// Summary of Wi-Fi, Ethernet, USB, and DECT devices
    AllDevices,
    /// Find an active device by MAC address
    Device { mac: String },
    /// Devices previously connected to the router
    DeviceHistory,
    /// SSIDs, access point status, and connected Wi-Fi clients
    Networks,
    /// Extended Wi-Fi configuration (keys are hidden)
    WifiConfig,
    /// List active Wi-Fi clients via the Devices API
    WifiClients,
    /// LAN address and DHCP settings
    Lan,
    /// Change the LAN address and DHCP range (reconnection required)
    LanSet {
        address: std::net::Ipv4Addr,
        netmask: std::net::Ipv4Addr,
        dhcp_min: std::net::Ipv4Addr,
        dhcp_max: std::net::Ipv4Addr,
        #[arg(long, value_enum)]
        dhcp: DhcpAction,
    },
    /// Summary of LAN interface settings
    NetworkDetails,
    /// WAN, IPv6, VLAN, MTU, and ISP PPP details
    InternetDetails,
    /// List reserved DHCP addresses
    Leases,
    /// Current DHCP leases (dynamic and reserved)
    CurrentLeases,
    /// DHCP pool settings, including lease duration and DNS
    DhcpSettings,
    /// Change the DHCP lease duration (with confirmation)
    DhcpLeaseTime { seconds: u32 },
    /// Show or set the current firewall level
    Firewall {
        #[arg(value_enum)]
        level: Option<FirewallLevel>,
    },
    /// Show or set the IPv6 firewall level
    FirewallIpv6 {
        #[arg(value_enum)]
        level: Option<FirewallLevel>,
    },
    /// Remote access status for the interface
    RemoteAccess,
    /// Disable router management over the Internet
    RemoteAccessDisable,
    /// Guest Wi-Fi status; on/off controls the network
    Guest {
        #[arg(value_enum)]
        action: Option<GuestAction>,
    },
    /// Energy-saving and power settings
    Energy,
    /// Configured port forwards
    Forwards,
    /// Create a port forward with confirmation
    ForwardAdd {
        name: String,
        ip: String,
        external_port: u16,
        internal_port: u16,
        #[arg(long, value_enum, default_value = "tcp")]
        protocol: ForwardProtocol,
    },
    /// Remove a port forward by name
    ForwardRemove { name: String },
    /// Port forwards created by UPnP
    UpnpForwards,
    /// Show or change UPnP gateway availability
    Upnp {
        #[arg(value_enum)]
        action: Option<WifiAction>,
    },
    /// Firewall details: ping, DMZ, and custom rules
    FirewallDetails,
    /// Read or change the firewall response to pings from the Internet
    FirewallPing {
        #[arg(value_enum)]
        action: Option<WifiAction>,
    },
    /// Current DMZ rule
    Dmz,
    /// Route all incoming traffic to a private LAN address
    DmzSet { destination: std::net::Ipv4Addr },
    /// Remove the DMZ rule
    DmzClear,
    /// List IPv6 inbound access rules
    Ipv6Pinholes,
    /// Open an external port on an IPv6 device (confirmation required)
    Ipv6PinholeAdd {
        name: String,
        destination: Ipv6Addr,
        mac: String,
        port: u16,
        #[arg(long, value_enum, default_value = "tcp")]
        protocol: PinholeProtocol,
    },
    /// Remove an IPv6 inbound access rule by name
    Ipv6PinholeRemove { name: String },
    /// Add a DHCP reservation for a MAC and IPv4 address
    Reserve { mac: String, ip: String },
    /// Remove a DHCP reservation by MAC address
    Unreserve { mac: String },
    /// Reboot the router
    Reboot,
    /// Detailed Wi-Fi statistics (bytes received/sent)
    WifiStats,
    /// Wi-Fi radios, bands, and available channels
    WifiRadios,
    /// Wi-Fi on/off schedule
    WifiSchedule,
    /// Wi-Fi repeater schedule status
    RepeaterSchedule,
    /// LED and energy-saving schedule status
    PowerSchedules,
    /// Devices blocked from Internet access
    BlockedDevices,
    /// Block a device from accessing the Internet
    BlockDevice { mac: String },
    /// Unblock a device's Internet access
    UnblockDevice { mac: String },
    /// IPv6 settings
    Ipv6,
    /// System time and time zone
    Time,
    /// Show or change the time zone
    TimeZone { timezone: Option<String> },
    /// List time synchronization servers
    NtpServers,
    /// Date of the last configuration backup at the ISP
    BackupStatus,
    /// Energy-saving profiles
    EnergyProfiles,
    /// Smart Wi-Fi / steering settings
    WifiSteering,
    /// Router IoT service status
    IotStatus,
    /// IoT Wi-Fi access point status from the dashboard
    WifiIotStatus,
    /// Categories, tile order, and favorites on the main dashboard
    DashboardTiles,
    /// Check whether an administrator password is set without revealing it
    AdminPasswordState,
    /// Show the interface language configured on the router
    Language,
    /// Add or remove a dashboard tile from favorites
    DashboardFavorite {
        tile: String,
        #[arg(value_enum)]
        action: FavoriteAction,
    },
    /// IPTV status and Orange TV channel settings
    Television,
    /// IPTV status from the dashboard
    IptvStatus,
    /// Telephony configuration; passwords in the response are masked
    Voice,
    /// Phone line and missed call summary, without call log details
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
