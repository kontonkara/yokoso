//! The network: Wi-Fi by its strength, a wire, or nothing; a shield while a
//! VPN is up; a pulse while a connection comes up. Its name, strength and
//! address in words on resting.

use yokoso_ui::{
    el::{Edges, El, Ink, Size},
    theme::Theme,
};

use super::Msg;
use crate::nm::{Link, Net, Network, is_vpn};

/// The Wi-Fi icon for `strength` percent: more arcs, more signal.
pub fn level(strength: u8) -> &'static str {
    match strength {
        67.. => "wifi",
        34.. => "wifi-2",
        1.. => "wifi-1",
        0 => "wifi-0",
    }
}

/// The VPNs that are up, by name: what NetworkManager calls one, WireGuard,
/// and the connections the config names.
pub fn vpns(network: &Network, named: &[String]) -> Vec<String> {
    network
        .up
        .iter()
        .filter(|(name, kind, vpn)| is_vpn(kind, *vpn, name, named))
        .map(|(name, _, _)| name.clone())
        .collect()
}

/// What it says on resting.
pub fn about(net: &Net, named: &[String]) -> String {
    let Net::Up(network) = net else {
        return "No word from NetworkManager".into();
    };
    let address = network
        .address
        .as_ref()
        .map(|a| format!(" \u{b7} {a}"))
        .unwrap_or_default();
    let mut said = match &network.link {
        _ if network.airplane => "Airplane mode".to_owned(),
        Link::Wifi { ssid, strength } => format!("{ssid} \u{b7} {strength}%{address}"),
        Link::Wired => format!("Wired: {}{address}", network.name),
        Link::Other(_) => format!("{}{address}", network.name),
        Link::None => match &network.connecting {
            Some(name) => format!("Connecting to {name}"),
            None if !network.wireless => "Wi-Fi is off".into(),
            None => "Not connected".into(),
        },
    };
    let up = vpns(network, named);
    if !up.is_empty() {
        said.push_str(&format!(" \u{b7} VPN: {}", up.join(", ")));
    }
    said
}

/// The part: always there, as no network is worth seeing too.
pub fn view(net: &Net, named: &[String], theme: &Theme, pad: Edges, on_click: Option<Msg>) -> El<Msg> {
    let faint = Ink::Own(theme.colors.on_surface.alpha(0.3));
    let mut pending = false;
    let mut parts = match net {
        Net::Unknown => vec![El::icon("wifi-off").ink(faint)],
        Net::Up(network) if network.airplane => vec![El::icon("airplane")],
        Net::Up(network) => match &network.link {
            Link::Wifi { strength, .. } => {
                vec![El::stack(vec![El::icon("wifi").ink(faint), El::icon(level(*strength))])]
            }
            Link::Wired | Link::Other(_) => vec![El::icon("ethernet")],
            Link::None if network.connecting.is_some() => {
                pending = true;
                vec![El::icon("wifi").ink(faint)]
            }
            Link::None => vec![El::icon("wifi-off")],
        },
    };
    if let Net::Up(network) = net
        && !vpns(network, named).is_empty()
    {
        parts.push(El::icon("shield").ink(Ink::Primary));
    }
    El::press(2.0, parts, on_click)
        .pending(pending)
        .key("network")
        .padding(pad)
        .height(Size::Fill(1.0))
        .tooltip(about(net, named))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wifi(strength: u8) -> Network {
        Network {
            link: Link::Wifi {
                ssid: "HomeWiFi5G".into(),
                strength,
            },
            name: "HomeWiFi5G".into(),
            address: Some("192.168.1.5".into()),
            wireless: true,
            up: vec![
                ("HomeWiFi5G".into(), "802-11-wireless".into(), false),
                ("relay".into(), "tun".into(), false),
                ("myvpn".into(), "tun".into(), false),
            ],
            ..Network::default()
        }
    }

    #[test]
    fn wifi_shows_its_strength_in_arcs() {
        assert_eq!(level(90), "wifi");
        assert_eq!(level(50), "wifi-2");
        assert_eq!(level(10), "wifi-1");
        assert_eq!(level(0), "wifi-0");
    }

    #[test]
    fn it_says_where_it_is_connected_and_which_vpn_is_up() {
        let named = vec!["myvpn".to_owned()];
        assert_eq!(
            about(&Net::Up(wifi(72)), &named),
            "HomeWiFi5G \u{b7} 72% \u{b7} 192.168.1.5 \u{b7} VPN: myvpn"
        );
        assert_eq!(
            about(&Net::Up(wifi(72)), &[]),
            "HomeWiFi5G \u{b7} 72% \u{b7} 192.168.1.5"
        );
        let connecting = Network {
            connecting: Some("Cafe".into()),
            wireless: true,
            ..Network::default()
        };
        assert_eq!(about(&Net::Up(connecting), &[]), "Connecting to Cafe");
        let off = Network::default();
        assert_eq!(about(&Net::Up(off), &[]), "Wi-Fi is off");
        let airplane = Network {
            airplane: true,
            ..Network::default()
        };
        assert_eq!(about(&Net::Up(airplane), &[]), "Airplane mode");
        assert_eq!(about(&Net::Unknown, &[]), "No word from NetworkManager");
    }
}
