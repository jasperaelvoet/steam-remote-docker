# Networking

The container runs with host networking: Steam Remote Play's discovery,
pairing, and streaming assume the host's real network identity, and NAT-ing a
container network breaks client discovery.

## Required ports

Allow these through the **host** firewall:

| Port(s) | Protocol | Purpose |
| --- | --- | --- |
| `27031-27036` | UDP | Remote Play discovery and streaming |
| `27036` | TCP | Remote Play session control — the listener `steam-remote health` checks |

With firewalld:

```bash
firewall-cmd --permanent --add-port=27031-27036/udp
firewall-cmd --permanent --add-port=27036/tcp
firewall-cmd --reload
```

With UFW:

```bash
ufw allow 27031:27036/udp
ufw allow 27036/tcp
```

## Discovery

Steam Link discovers hosts via broadcast on the local network. Clients on a
different subnet — or on networks that filter broadcast, like many guest
Wi-Fi setups — can add the host manually by IP in the Steam Link app.

## Good to know

- The TCP `27036` listener only appears once Steam is running **and** logged
  in to an account with Remote Play enabled.
- An established connection on that port is one of the signals the
  [idle lifecycle](./idle-lifecycle.md) uses to keep the session at full
  rate.
- **Internet streaming**: Remote Play can traverse the internet via Steam's
  relays with no port forwarding, at a latency cost. For direct connections
  across networks, a VPN (e.g. WireGuard) that puts the client on the host's
  network is the predictable option.
