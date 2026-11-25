//! The tray: an icon for each item that isn't passive, in the order they
//! came. A click activates it (or opens its menu, where it has only that),
//! the middle button its second action, the right one its menu, the wheel
//! scrolls it.

use yokoso_ui::el::{El, Size};

use super::Msg;
use crate::sni::Item;

/// The part; None with nothing in the tray.
/// The item whose menu is open now is shown chosen: `open` is the bar part
/// with a panel open, as "tray:<address>".
pub fn view(items: &[Item], icon: f32, open: Option<&str>) -> Option<El<Msg>> {
    let shown: Vec<El<Msg>> = items
        .iter()
        .filter(|item| item.status != "Passive")
        .map(|item| {
            let address = item.address.clone();
            let picture = match &item.icon {
                Some(picture) => El::picture(picture.clone()),
                // An item without an icon still has a place to click.
                None => El::icon("bell"),
            };
            let on_press = if item.only_menu {
                Msg::TrayMenu(address.clone())
            } else {
                Msg::TrayActivate(address.clone())
            };
            let about = [&item.tooltip, &item.title, &item.id]
                .into_iter()
                .find(|text| !text.is_empty())
                .cloned()
                .unwrap_or_default();
            El::press(
                0.0,
                vec![picture.width(Size::Fixed(icon)).height(Size::Fixed(icon))],
                Some(on_press),
            )
            .selected(open.and_then(|part| part.strip_prefix("tray:")) == Some(item.address.as_str()))
            .on_middle(Msg::TraySecondary(address.clone()))
            .on_secondary(Msg::TrayMenu(address.clone()))
            .on_scroll(move |steps| Msg::TrayScroll(address.clone(), steps))
            .key(("tray", item.address.as_str()))
            .padding(yokoso_ui::el::Edges::all(6.0))
            .height(Size::Fill(1.0))
            .tooltip(about)
        })
        .collect();
    (!shown.is_empty()).then(|| El::row(2.0, shown).height(Size::Fill(1.0)).key("tray"))
}
