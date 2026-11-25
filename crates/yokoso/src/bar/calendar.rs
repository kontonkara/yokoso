//! The calendar the clock opens: a month, weeks from Monday with their ISO
//! numbers, today marked. The arrows and the wheel go a month on or back;
//! the month's name goes back to today's.

use chrono::{Datelike, Duration, NaiveDate};
use yokoso_ui::{
    el::{Align, Edges, El, Ink, Size, Type},
    theme::Theme,
};

use super::Msg;

/// The panel's size.
pub const WIDTH: f32 = 320.0;
pub const HEIGHT: f32 = 336.0;
const CELL: f32 = 36.0;
const WEEK: f32 = 30.0;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// The month `steps` on from `year`, `month` (1 to 12); back for less.
pub fn step(year: i32, month: u32, steps: i32) -> (i32, u32) {
    let index = year * 12 + month as i32 - 1 + steps;
    (index.div_euclid(12), index.rem_euclid(12) as u32 + 1)
}

/// The six weeks shown for a month: each its ISO number and its days, from
/// the Monday on or before the first.
pub fn weeks(year: i32, month: u32) -> Vec<(u32, [NaiveDate; 7])> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("a month of a year");
    let monday = first - Duration::days(i64::from(first.weekday().num_days_from_monday()));
    (0..6)
        .map(|week| {
            let start = monday + Duration::days(week * 7);
            let days = std::array::from_fn(|day| start + Duration::days(day as i64));
            (start.iso_week().week(), days)
        })
        .collect()
}

/// The panel's content for `shown` (year, month), `today` marked.
pub fn view(shown: (i32, u32), today: NaiveDate, theme: &Theme) -> El<Msg> {
    let (year, month) = shown;
    let faint = Ink::Own(theme.colors.on_surface.alpha(0.38));
    let arrow = |icon: &'static str, steps: f32| {
        El::press(0.0, vec![El::icon(icon)], Some(Msg::CalendarStep(steps)))
            .width(Size::Fixed(CELL))
            .height(Size::Fixed(CELL))
            .key(icon)
    };
    let title = El::press(
        0.0,
        vec![El::text(format!("{} {year}", MONTHS[month as usize - 1]), Type::Title).weight(600.0)],
        Some(Msg::CalendarToday),
    )
    .key("month")
    .padding(Edges {
        left: 10.0,
        right: 10.0,
        ..Edges::default()
    })
    .height(Size::Fixed(CELL));
    let header = El::row(
        0.0,
        vec![
            title,
            El::spacer().width(Size::Fill(1.0)),
            arrow("chevron-left", -1.0),
            arrow("chevron-right", 1.0),
        ],
    )
    .align(Align::Center)
    .width(Size::Fill(1.0));
    let cell = |text: String, ink: Ink, width: f32| {
        El::row(
            0.0,
            vec![
                El::spacer(),
                El::text(text, Type::Label).ink(ink).tabular(),
                El::spacer(),
            ],
        )
        .align(Align::Center)
        .width(Size::Fixed(width))
        .height(Size::Fixed(24.0))
    };
    let mut names = vec![cell(String::new(), faint, WEEK)];
    names.extend(
        ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
            .iter()
            .map(|name| cell((*name).into(), Ink::OnSurfaceVariant, CELL)),
    );
    let mut rows = vec![header, El::row(0.0, names)];
    for (number, days) in weeks(year, month) {
        let mut row = vec![cell(number.to_string(), faint, WEEK)];
        for day in days {
            let ink = if day.month() == month { Ink::OnSurface } else { faint };
            let ink = if day == today {
                Ink::Own(theme.colors.on_secondary_container)
            } else {
                ink
            };
            let mut number = El::text(day.day().to_string(), Type::Body).ink(ink).tabular();
            if day == today {
                number = number.weight(700.0);
            }
            row.push(
                El::press(0.0, vec![El::spacer(), number, El::spacer()], None)
                    .selected(day == today)
                    .width(Size::Fixed(CELL))
                    .height(Size::Fixed(CELL))
                    .key(("day", day.num_days_from_ce())),
            );
        }
        rows.push(El::row(0.0, row));
    }
    El::press(2.0, vec![El::column(2.0, rows)], None)
        .on_scroll(Msg::CalendarStep)
        .padding(Edges::all(12.0))
        .fill()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn months_step_over_years() {
        assert_eq!(step(2026, 11, 1), (2026, 12));
        assert_eq!(step(2026, 12, 1), (2027, 1));
        assert_eq!(step(2026, 1, -1), (2025, 12));
        assert_eq!(step(2026, 11, -23), (2024, 12));
    }

    #[test]
    fn a_month_is_six_weeks_from_the_monday_before_its_first() {
        let november = weeks(2026, 11);
        assert_eq!(november.len(), 6);
        // The first is a Sunday: the week starts on the 26th of October, the
        // 44th of the year.
        assert_eq!(november[0].1[0], NaiveDate::from_ymd_opt(2026, 10, 26).unwrap());
        assert_eq!(november[0].0, 44);
        assert_eq!(november[0].1[6], NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        // A new year's week 1 can start in December.
        assert_eq!(weeks(2025, 12)[5].0, 2);
    }
}
