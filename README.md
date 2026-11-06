# funbox7

Небольшая Rust CLI-утилита для локального управления Orange Funbox 7 через API веб-панели `/ws`.

## Сборка

```sh
cargo build --release
```

## Команды

```sh
cargo run -- status       # модель, WAN и Wi-Fi
cargo run -- info         # модель и прошивка
cargo run -- wan          # состояние подключения
cargo run -- wifi         # состояние Wi-Fi
cargo run -- wifi-config  # расширенные параметры точек доступа (секреты скрыты)
cargo run -- wifi-clients # активные Wi-Fi-клиенты
cargo run -- wifi-radios  # диапазоны, радиомодули и каналы
cargo run -- wifi-stats   # байты приёма/передачи Wi-Fi
cargo run -- wifi-schedule # расписание Wi-Fi
cargo run -- wifi-steering # настройки Smart Wi-Fi
cargo run -- iot-status    # состояние IoT-сервиса
cargo run -- devices      # таблица активных DHCP-устройств
cargo run -- device-history # ранее подключавшиеся устройства
cargo run -- networks     # точки доступа и подключённые Wi-Fi-клиенты
cargo run -- lan          # LAN-адрес и DHCP
cargo run -- network-details # LAN-интерфейсы и радио
cargo run -- internet-details # WAN, IPv6 и VLAN провайдера
cargo run -- leases       # закреплённые DHCP-адреса
cargo run -- firewall     # уровень firewall
cargo run -- firewall high # сменить уровень firewall, спросит подтверждение
cargo run -- guest        # гостевая Wi-Fi сеть
cargo run -- energy       # энергосбережение
cargo run -- energy-profiles # доступные профили энергосбережения
cargo run -- forwards     # список пробросов портов
cargo run -- upnp-forwards # динамические пробросы UPnP
cargo run -- firewall-details # DMZ, ping и пользовательские правила
cargo run -- ipv6         # параметры IPv6
cargo run -- time         # время роутера и часовой пояс
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

Клиент хранит сессионную cookie только в памяти на время запуска, отправляет `releaseContext` при завершении команды и не сохраняет пароль или токен в проекте. API этой прошивки доступен по HTTP, поэтому запускай CLI только из доверенной локальной сети.

## Архитектура

`core` содержит API-клиент и общие операции роутера; `cli` — аргументы, подтверждения и отображение результатов. Другой транспортный интерфейс, например MCP, сможет использовать `core` без зависимости от CLI.
