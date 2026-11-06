# funbox7

Небольшая Rust CLI-утилита для локального управления Orange Funbox 7 через API веб-панели `/ws`.

## Сборка

```sh
cargo build --release
```

## Команды

```sh
cargo run -- status       # модель, WAN и Wi-Fi
cargo run -- info         # модель, прошивка, аптайм и число перезагрузок
cargo run -- leds         # состояния световых индикаторов
cargo run -- users        # пользователи панели, только безопасные поля
cargo run -- ddns-services # список поддерживаемых DynDNS-провайдеров
cargo run -- ddns-hosts   # настроенные DDNS-хосты без учётных данных
cargo run -- ddns-add NoIP myhost.example.net myuser # добавление спросит подтверждение и скрытый пароль провайдера
cargo run -- ddns-remove myhost.example.net # удалить DDNS-хост, спросит подтверждение
cargo run -- wan          # состояние подключения
cargo run -- wan-stats    # счётчики входящего и исходящего трафика
cargo run -- wifi         # состояние Wi-Fi и WPS, если поле доступно
cargo run -- wps          # статус WPS
cargo run -- wps off      # отключить WPS с подтверждением
cargo run -- wps on       # включить WPS с предупреждением и подтверждением
cargo run -- wifi-open-mode # статус Open Mode без вывода SSID
cargo run -- wifi-config  # расширенные параметры точек доступа (секреты скрыты)
cargo run -- wifi-clients # активные Wi-Fi-клиенты
cargo run -- wifi-radios  # диапазоны, радиомодули и каналы
cargo run -- wifi-stats   # байты приёма/передачи Wi-Fi
cargo run -- wifi-schedule # расписание Wi-Fi
cargo run -- blocked-devices # устройства с ограничением доступа в Интернет
cargo run -- block-device AA:BB:CC:DD:EE:FF # заблокировать Интернет, спросит подтверждение
cargo run -- unblock-device AA:BB:CC:DD:EE:FF # разблокировать, спросит подтверждение
cargo run -- wifi-steering # настройки Smart Wi-Fi
cargo run -- iot-status    # состояние IoT-сервиса
cargo run -- television   # статус IPTV и конфигурация каналов Orange TV
cargo run -- voice        # настройки телефонии с маскировкой паролей
cargo run -- devices      # таблица активных DHCP-устройств
cargo run -- all-devices  # Wi-Fi, Ethernet, USB и DECT
cargo run -- device AA:BB:CC:DD:EE:FF # карточка активного устройства по MAC
cargo run -- device-history # ранее подключавшиеся устройства
cargo run -- networks     # точки доступа и подключённые Wi-Fi-клиенты
cargo run -- lan          # LAN-адрес и DHCP
cargo run -- lan-set 192.168.1.1 255.255.255.0 192.168.1.50 192.168.1.200 --dhcp on # спросит подтверждение
cargo run -- network-details # LAN-интерфейсы и радио
cargo run -- internet-details # WAN, IPv6, VLAN, MTU и сведения PPP (если доступны)
cargo run -- leases       # закреплённые DHCP-адреса
cargo run -- current-leases # текущие DHCP-аренды
cargo run -- dhcp-settings # настройки DHCP-пула, срок аренды и DNS
cargo run -- dhcp-lease-time 86400 # установить срок аренды в секундах, спросит подтверждение
cargo run -- firewall     # уровень firewall
cargo run -- firewall high # сменить уровень firewall, спросит подтверждение
cargo run -- guest        # состояние и параметры гостевой Wi-Fi сети
cargo run -- guest on     # включить гостевую Wi-Fi без таймера, спросит подтверждение
cargo run -- guest off    # выключить гостевую сеть, спросит подтверждение
cargo run -- energy       # энергосбережение
cargo run -- energy-profiles # доступные профили энергосбережения
cargo run -- forwards     # список пробросов портов
cargo run -- forward-add web 192.168.1.25 443 443 --protocol tcp # создаёт проброс с подтверждением
cargo run -- forward-remove web # удаляет проброс с подтверждением
cargo run -- upnp-forwards # динамические пробросы UPnP
cargo run -- upnp off  # отключить автоматическое открытие портов, спросит подтверждение
cargo run -- upnp on   # включить UPnP, спросит подтверждение
cargo run -- firewall-details # DMZ, ping и пользовательские правила
cargo run -- firewall-ipv6 # текущий уровень фильтрации IPv6
cargo run -- firewall-ipv6 high # изменить уровень, спросит подтверждение
cargo run -- remote-access # статус удалённого доступа к панели (без адресов и секретов)
cargo run -- remote-access-disable # отключить удалённое управление, спросит подтверждение
cargo run -- firewall-ping # состояние ответа на ping
cargo run -- firewall-ping on # включить ответ на ping, спросит подтверждение
cargo run -- dmz # показать правило DMZ
cargo run -- dmz-set 192.168.1.25 # направить входящий трафик на хост, спросит подтверждение
cargo run -- dmz-clear # снять DMZ, спросит подтверждение
cargo run -- ipv6-pinholes # список открытых IPv6 pinhole-правил
cargo run -- ipv6-pinhole-add web-server fd12:3456:789a::10 AA:BB:CC:DD:EE:FF 443 --protocol tcp # открыть порт, спросит подтверждение
cargo run -- ipv6-pinhole-remove web-server # удалить правило, спросит подтверждение
cargo run -- ipv6         # параметры IPv6
cargo run -- time         # время роутера и часовой пояс
cargo run -- time-zone    # текущий часовой пояс
cargo run -- time-zone Europe/Paris # изменить часовой пояс, спросит подтверждение
cargo run -- ntp-servers  # серверы синхронизации времени
cargo run -- backup-status # дата последней копии конфигурации у оператора
cargo run -- reserve AA:BB:CC:DD:EE:FF 192.168.1.25 # DHCP-резервация
cargo run -- unreserve AA:BB:CC:DD:EE:FF # удалить резервацию
cargo run -- wifi on      # включить Wi-Fi, спросит подтверждение
cargo run -- wifi off     # выключить Wi-Fi, спросит подтверждение
cargo run -- reboot       # перезагрузить роутер, спросит подтверждение
```

Адрес и имя пользователя можно переопределить флагами или переменными `FUNBOX_HOST` и `FUNBOX_USER`. Пароль по умолчанию запрашивается скрытым вводом; его можно передать через `FUNBOX_PASSWORD` для автоматизации, но безопаснее не хранить его в shell history или файлах.

```sh
cargo run -- --host 192.168.1.1 --user admin status
```

`--yes` пропускает подтверждение изменения настроек. Не используй `wifi off`, если управляешь роутером по Wi-Fi: соединение с CLI оборвётся.

Команды изменения параметров специально требуют подтверждения. `--yes` предназначен для осознанной автоматизации; особенно осторожно используй его с `wifi off` и `reboot`.

Для `forward-add` и `dmz-set` адрес назначения проверяется по текущей частной LAN-подсети. `lan-set` принимает только подсети RFC 1918; это помогает избежать потери доступа к панели и ошибочной маршрутизации.

Клиент хранит сессионную cookie только в памяти на время запуска, отправляет `releaseContext` при завершении команды и не сохраняет пароль или токен в проекте. API этой прошивки доступен по HTTP, поэтому запускай CLI только из доверенной локальной сети.
Пароль DDNS-провайдера для `ddns-add` вводится скрыто; CLI не выводит его и подавляет детали API-ошибок такого запроса. Передача всё равно идёт по HTTP к роутеру — используй команду только в доверенной локальной сети.

Некоторые редкие методы (управление UPnP, IPv6 pinholes, DDNS-хосты, установка часового пояса, `firewall-ipv6`, `remote-access`, `backup-status`, управление WPS, `wan-stats`, срок DHCP-аренды и другие) сверены по API соседних поколений Livebox/Funbox, но ещё не проверены на Funbox 7 `ARF7-pl-01.01.05.27`; если прошивка их не поддерживает, команда вернёт ошибку API.
`blocked-devices` показывает только блокировку выхода в Интернет; она не изолирует устройство от других узлов локальной сети.
`block-device` и `unblock-device` также управляют только доступом в Интернет; обе команды требуют подтверждения.

## Архитектура

`core` содержит API-клиент и общие операции роутера; `cli` — аргументы, подтверждения и отображение результатов. Другой транспортный интерфейс, например MCP, сможет использовать `core` без зависимости от CLI.
