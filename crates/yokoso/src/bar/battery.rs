//! The battery: an icon filled as far as it is charged, the percentage
//! beside it; the accent while it charges, a warning below a fifth, the
//! error color below a tenth. What it says in words shows on resting.

use yokoso_ui::{
    el::{Edges, El, Ink, Size, Type},
    theme::Theme,
};

use super::Msg;
use crate::upower::{Battery, Charge, Power};

/// "2 h 5 min", "40 min".
fn span(seconds: i64) -> String {
    let minutes = (seconds + 30) / 60;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// What it says on resting.
pub fn about(battery: &Battery) -> String {
    let percent = format!("{:.0}%", battery.percentage);
    let mut said = match battery.charge {
        Charge::Full => "Full".to_owned(),
        Charge::Holding => format!("{percent}, plugged in, not charging"),
        Charge::Charging if battery.to_full > 0 => format!("{percent}, full in {}", span(battery.to_full)),
        Charge::Charging => format!("{percent}, charging"),
        Charge::Discharging | Charge::Empty if battery.to_empty > 0 => {
            format!("{percent}, {} left", span(battery.to_empty))
        }
        _ => percent,
    };
    if battery.rate >= 0.05 {
        said.push_str(&format!(" \u{b7} {:.1} W", battery.rate));
    }
    said
}

/// Its color: the accent while it is plugged in, else by how low it is.
pub fn ink(battery: &Battery, theme: &Theme) -> Ink {
    match battery.charge {
        Charge::Charging | Charge::Full | Charge::Holding => Ink::Primary,
        _ if battery.percentage <= 10.0 => Ink::Error,
        _ if battery.percentage <= 20.0 => Ink::Own(theme.colors.tertiary),
        _ => Ink::OnSurface,
    }
}

/// The part; None without a battery.
pub fn view(power: &Power, theme: &Theme, pad: Edges, on_click: Option<Msg>) -> Option<El<Msg>> {
    let Power::Battery(battery) = power else {
        return None;
    };
    let size = theme.sizes.icon;
    let ink = ink(battery, theme);
    // The fill, inside the outline's body: from 4.5 to 17 across, 9 to 15
    // down, of the icon's 24.
    let unit = size / 24.0;
    let full = 12.5 * unit;
    let level = (battery.percentage as f32 / 100.0).clamp(0.0, 1.0);
    let fill = El::row(
        0.0,
        vec![
            El::block(ink, unit)
                .width(Size::Fixed((full * level).max(unit)))
                .height(Size::Fixed(6.0 * unit))
                .glide(),
        ],
    )
    .padding(Edges {
        left: 4.5 * unit,
        top: 9.0 * unit,
        ..Edges::default()
    });
    let mut parts = vec![El::stack(vec![El::icon("battery").ink(ink), fill])];
    if matches!(battery.charge, Charge::Charging) {
        parts.push(El::icon("bolt").ink(ink));
    }
    parts.push(
        El::text(format!("{:.0}%", battery.percentage), Type::Body)
            .weight(600.0)
            .tabular()
            .ink(ink),
    );
    Some(
        El::press(4.0, parts, on_click)
            .key("battery")
            .padding(pad)
            .height(Size::Fill(1.0))
            .tooltip(about(battery)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn battery(percentage: f64, charge: Charge) -> Battery {
        Battery {
            percentage,
            charge,
            ..Battery::default()
        }
    }

    #[test]
    fn it_says_how_long_it_lasts_or_charges() {
        let mut b = battery(86.0, Charge::Discharging);
        b.to_empty = 7500;
        b.rate = 8.42;
        assert_eq!(about(&b), "86%, 2 h 5 min left \u{b7} 8.4 W");
        let mut b = battery(40.0, Charge::Charging);
        b.to_full = 2400;
        assert_eq!(about(&b), "40%, full in 40 min");
        assert_eq!(about(&battery(100.0, Charge::Full)), "Full");
        assert_eq!(about(&battery(80.0, Charge::Holding)), "80%, plugged in, not charging");
        assert_eq!(about(&battery(55.0, Charge::Discharging)), "55%");
    }

    #[test]
    fn low_is_a_warning_and_lower_an_alarm_but_not_while_it_charges() {
        let theme = Theme::default();
        assert_eq!(ink(&battery(50.0, Charge::Discharging), &theme), Ink::OnSurface);
        assert_eq!(
            ink(&battery(18.0, Charge::Discharging), &theme),
            Ink::Own(theme.colors.tertiary)
        );
        assert_eq!(ink(&battery(9.0, Charge::Discharging), &theme), Ink::Error);
        assert_eq!(ink(&battery(9.0, Charge::Charging), &theme), Ink::Primary);
    }
}
