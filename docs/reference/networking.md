# Networking & ports

The container runs with `--network host`: Steam Remote Play's discovery,
pairing, and streaming assume the host's real network identity, and NAT-ing a
container network breaks client discovery.

## Required ports

Allow these through the **host** firewall:

| Port(s) | Protocol | Purpose |
| --- | --- | --- |
| `27031-27036` | UDP | Remote Play discovery and streaming |
| `27036` | TCP | Remote Play session control — this is the listener that `steam-remote health` checks |

For example, with firewalld:

```sh
firewall-cmd --permanent --add-port=27031-27036/udp
firewall-cmd --permanent --add-port=27036/tcp
firewall-cmd --reload
```

or with UFW:

```sh
ufw allow 27031:27036/udp
ufw allow 27036/tcp
```

## Discovery

Steam Link discovers hosts via broadcast on the local network. Clients on a
different subnet (or with broadcast filtered, as on many Wi-Fi guest networks)
can add the host manually by IP address in the Steam Link app.

## How the image uses these ports

- The TCP `27036` listener only appears once Steam is running **and** logged
  in to an account with Remote Play enabled. `steam-remote status` reports it
  as `remote play`.
- An **established** connection on TCP `27036` is one of the signals the
  [idle lifecycle](/guide/idle-lifecycle) uses to keep the session at full
  rate.

## Streaming over the internet

Steam Remote Play can traverse the internet via Steam's relays without any
port forwarding, at the cost of latency. For direct connections across
networks, a VPN (for example WireGuard) that puts the client on the host's
network is the predictable option; the image itself has no opinion on this —
it only needs the ports above reachable from the client.
