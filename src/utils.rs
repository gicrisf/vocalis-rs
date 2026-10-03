use gtk::glib;

pub fn format_time(nano_seconds: i64) -> String {
    let total_ms = nano_seconds / 1_000_000;
    let hundredths = (total_ms / 100) % 10;
    let seconds = (total_ms / 1_000) % 60;
    let minutes = (total_ms / 60_000) % 60;
    let hours = total_ms / 3_600_000;

    format!(
        "{hours}\u{200a}\u{2236}\u{200a}{minutes:02}\u{200a}\u{2236}\u{200a}\
         {seconds:02}\u{200a}.\u{200a}<small>{hundredths}</small>"
    )
}

pub fn display_date_time(time: &glib::DateTime) -> String {
    const DAY: i64 = 86_400_000_000;

    let now = glib::DateTime::now_local().unwrap();
    let difference = now.difference(time).as_microseconds();

    // Keep the `ngettext` calls (with literal msgids) inline so that `xgettext`
    // can extract them.
    if difference < DAY {
        match time.format("%X") {
            Ok(formatted) => formatted.to_string(),
            Err(_) => gettextrs::gettext("Less than a day ago"),
        }
    } else if difference < 2 * DAY {
        gettextrs::gettext("Yesterday")
    } else if difference < 7 * DAY {
        let n = (difference / DAY).max(0) as u32;
        gettextrs::ngettext("%d day ago", "%d days ago", n).replacen("%d", &n.to_string(), 1)
    } else if difference < 14 * DAY {
        gettextrs::gettext("Last week")
    } else if difference < 28 * DAY {
        let n = (difference / (7 * DAY)).max(0) as u32;
        gettextrs::ngettext("%d week ago", "%d weeks ago", n).replacen("%d", &n.to_string(), 1)
    } else if difference < 60 * DAY {
        gettextrs::gettext("Last month")
    } else if difference < 360 * DAY {
        let n = (difference / (30 * DAY)).max(0) as u32;
        gettextrs::ngettext("%d month ago", "%d months ago", n).replacen("%d", &n.to_string(), 1)
    } else if difference < 730 * DAY {
        gettextrs::gettext("Last year")
    } else {
        let n = (difference / (365 * DAY)).max(0) as u32;
        gettextrs::ngettext("%d year ago", "%d years ago", n).replacen("%d", &n.to_string(), 1)
    }
}
