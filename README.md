# funbox7

A small Rust CLI utility for managing an Orange Funbox 7 locally through the web interface API at `/ws`.

## Build

```sh
cargo build --release
```

## Commands

```sh
cargo run -- status       # model, WAN, and Wi-Fi
cargo run -- dashboard-tiles # dashboard categories, tile order, and favorites
cargo run -- admin-password-state # check whether the default password is in use
cargo run -- language # show the router's interface language
cargo run -- internet-state # Internet connection state, as shown on the dashboard tile
cargo run -- repeater-schedule # Wi-Fi repeater schedule status
cargo run -- power-schedules # LED, Wi-Fi, and power-saving schedules
cargo run -- wifi-iot-status # whether IoT Wi-Fi access points are enabled
cargo run -- iptv-status # IPTV tile status
cargo run -- phone-status # line status and number of missed calls, without phone numbers
cargo run -- dashboard-favorite wifiAdvanced on # add a tile to favorites; confirmation required
cargo run -- dashboard-favorite wifiAdvanced off # remove a tile from favorites; confirmation required
cargo run -- info         # model, firmware, uptime, and reboot count
cargo run -- leds         # indicator light states
cargo run -- users        # web interface users; safe fields only
cargo run -- ddns-services # list supported DynDNS providers
cargo run -- ddns-hosts   # configured DDNS hosts, without credentials
cargo run -- ddns-add NoIP myhost.example.net myuser # asks for confirmation and the provider password (hidden input)
cargo run -- ddns-remove myhost.example.net # remove a DDNS host; confirmation required
cargo run -- wan          # connection status
cargo run -- wan-stats    # inbound and outbound traffic counters
cargo run -- wifi         # Wi-Fi and WPS status, if the field is available
cargo run -- wps          # WPS status
cargo run -- wps off      # disable WPS; confirmation required
cargo run -- wps on       # enable WPS; warning and confirmation required
cargo run -- wifi-open-mode # Open Mode status, without displaying the SSID
cargo run -- wifi-config  # advanced access point settings (secrets are hidden)
cargo run -- wifi-clients # active Wi-Fi clients
cargo run -- wifi-radios  # bands, radios, and channels
cargo run -- wifi-stats   # Wi-Fi bytes received/transmitted
cargo run -- wifi-schedule # Wi-Fi schedule
cargo run -- blocked-devices # devices restricted from Internet access
cargo run -- block-device AA:BB:CC:DD:EE:FF # block Internet access; confirmation required
cargo run -- unblock-device AA:BB:CC:DD:EE:FF # restore Internet access; confirmation required
cargo run -- wifi-steering # Smart Wi-Fi settings
cargo run -- iot-status    # IoT service status
cargo run -- television   # IPTV status and Orange TV channel configuration
cargo run -- voice        # telephony settings with passwords masked
cargo run -- devices      # table of active DHCP devices
cargo run -- all-devices  # Wi-Fi, Ethernet, USB, and DECT
cargo run -- device AA:BB:CC:DD:EE:FF # details for an active device by MAC address
cargo run -- device-history # previously connected devices
cargo run -- networks     # access points and connected Wi-Fi clients
cargo run -- lan          # LAN address and DHCP
cargo run -- lan-set 192.168.1.1 255.255.255.0 192.168.1.50 192.168.1.200 --dhcp on # confirmation required
cargo run -- network-details # LAN interfaces and radio
cargo run -- internet-details # WAN, IPv6, VLAN, MTU, and PPP details (if available)
cargo run -- leases       # reserved DHCP addresses
cargo run -- current-leases # current DHCP leases
cargo run -- dhcp-settings # DHCP pool, lease duration, and DNS settings
cargo run -- dhcp-lease-time 86400 # set lease duration in seconds; confirmation required
cargo run -- firewall     # firewall level
cargo run -- firewall high # change firewall level; confirmation required
cargo run -- guest        # guest Wi-Fi status and settings
cargo run -- guest on     # enable guest Wi-Fi without a timer; confirmation required
cargo run -- guest off    # disable the guest network; confirmation required
cargo run -- energy       # power saving
cargo run -- energy-profiles # available power-saving profiles
cargo run -- forwards     # list port forwards
cargo run -- forward-add web 192.168.1.25 443 443 --protocol tcp # create a port forward; confirmation required
cargo run -- forward-remove web # remove a port forward; confirmation required
cargo run -- upnp-forwards # dynamic UPnP port forwards
cargo run -- upnp off  # disable automatic port opening; confirmation required
cargo run -- upnp on   # enable UPnP; confirmation required
cargo run -- firewall-details # DMZ, ping, and custom rules
cargo run -- firewall-ipv6 # current IPv6 filtering level
cargo run -- firewall-ipv6 high # change the level; confirmation required
cargo run -- remote-access # remote access status (no addresses or secrets)
cargo run -- remote-access-disable # disable remote management; confirmation required
cargo run -- firewall-ping # ping response status
cargo run -- firewall-ping on # enable ping responses; confirmation required
cargo run -- dmz # show the DMZ rule
cargo run -- dmz-set 192.168.1.25 # route incoming traffic to a host; confirmation required
cargo run -- dmz-clear # clear the DMZ; confirmation required
cargo run -- ipv6-pinholes # list open IPv6 pinhole rules
cargo run -- ipv6-pinhole-add web-server fd12:3456:789a::10 AA:BB:CC:DD:EE:FF 443 --protocol tcp # open a port; confirmation required
cargo run -- ipv6-pinhole-remove web-server # remove a rule; confirmation required
cargo run -- ipv6         # IPv6 settings
cargo run -- time         # router time and time zone
cargo run -- time-zone    # current time zone
cargo run -- time-zone Europe/Paris # change the time zone; confirmation required
cargo run -- ntp-servers  # time synchronization servers
cargo run -- backup-status # date of the last configuration backup by the provider
cargo run -- reserve AA:BB:CC:DD:EE:FF 192.168.1.25 # DHCP reservation
cargo run -- unreserve AA:BB:CC:DD:EE:FF # remove a reservation
cargo run -- wifi on      # enable Wi-Fi; confirmation required
cargo run -- wifi off     # disable Wi-Fi; confirmation required
cargo run -- reboot       # reboot the router; confirmation required
```

The address and username can be overridden with flags or the `FUNBOX_HOST` and `FUNBOX_USER` environment variables. By default, the password is requested with hidden input; you can pass it through `FUNBOX_PASSWORD` for automation, but it is safer not to store it in shell history or files.

```sh
cargo run -- --host 192.168.1.1 --user admin status
```

`--yes` skips confirmation when changing settings. Do not use `wifi off` if you are managing the router over Wi-Fi: the CLI connection will be lost.

Commands that change settings intentionally require confirmation. `--yes` is intended for deliberate automation; use it with particular care with `wifi off` and `reboot`.

For `forward-add` and `dmz-set`, the destination address is checked against the current private LAN subnet. `lan-set` accepts only RFC 1918 subnets; this helps prevent losing access to the web interface and avoids incorrect routing.

The client keeps the session cookie in memory only for the duration of the run, sends `releaseContext` when the command exits, and does not store the password or token in the project. This firmware's API is available over HTTP, so run the CLI only on a trusted local network.
The DDNS provider password for `ddns-add` is entered without being displayed; the CLI does not print it and suppresses API error details for this request. It is still sent to the router over HTTP, so use this command only on a trusted local network.

Some less common methods (UPnP management, IPv6 pinholes, DDNS hosts, time zone changes, `firewall-ipv6`, `remote-access`, `backup-status`, WPS management, `wan-stats`, DHCP lease duration, and others) have been cross-checked against APIs from related Livebox/Funbox generations, but have not yet been tested on Funbox 7 `ARF7-pl-01.01.05.27`. If the firmware does not support them, the command will return an API error.
On firmware `ARF7-pl-01.01.05.27`, the read-only `sah.*` calls used by `dashboard-tiles`, `admin-password-state`, `language`, `internet-state`, `power-schedules`, `repeater-schedule`, `wifi-iot-status`, `iptv-status`, and `phone-status` were tested and return error `196618: Object or parameter not found`. These commands are currently non-functional on this firmware; do not rely on them.
`dashboard-favorite` writes through `sah.Mhs.storeCategories`; it was not invoked, so its compatibility is unverified. No router write operations were performed.
`blocked-devices` reports only restrictions on Internet access; it does not isolate a device from other devices on the local network.
`block-device` and `unblock-device` also control Internet access only; both commands require confirmation.

## Architecture

`core` contains the API client and shared router operations; `cli` handles arguments, confirmations, and displaying results. Another transport interface, such as MCP, can use `core` without depending on the CLI.
