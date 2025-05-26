#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        {
            let datetime = time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap();
            eprintln!("[{}] {}", datetime, format_args!($($arg)*));
        }
    };
}
