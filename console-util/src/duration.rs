use std::time::Duration;

pub fn parse_duration(s: &str) -> Result<Duration, Box<dyn std::error::Error>> {
    let s = s.trim();
    if let Some(s) = s
        .strip_suffix('h')
        .or_else(|| s.strip_suffix("hour"))
        .or_else(|| s.strip_suffix("hours"))
    {
        let s = s.trim();
        return Ok(s
            .parse::<u64>()
            .map(|hours| Duration::from_secs(hours * 60 * 60))
            .or_else(|_| {
                s.parse::<f64>()
                    .map(|hours| Duration::from_secs_f64(hours * 60.0 * 60.0))
            })?);
    }

    if let Some(s) = s
        .strip_suffix('m')
        .or_else(|| s.strip_suffix("min"))
        .or_else(|| s.strip_suffix("mins"))
        .or_else(|| s.strip_suffix("minute"))
        .or_else(|| s.strip_suffix("minutes"))
    {
        let s = s.trim();
        return Ok(s
            .parse::<u64>()
            .map(|mins| Duration::from_secs(mins * 60))
            .or_else(|_| {
                s.parse::<f64>()
                    .map(|mins| Duration::from_secs_f64(mins * 60.0))
            })?);
    }

    if let Some(s) = s.strip_suffix("ms") {
        return Ok(Duration::from_millis(s.trim().parse()?));
    }

    if let Some(s) = s.strip_suffix("us") {
        return Ok(Duration::from_micros(s.trim().parse()?));
    }

    // Order matters here -- we have to try `ns` for nanoseconds after we try
    // minutes, because `mins` ends in `ns`.
    if let Some(s) = s.strip_suffix("ns") {
        return Ok(Duration::from_nanos(s.trim().parse()?));
    }

    if let Some(s) = s
        .strip_suffix("sec")
        .or_else(|| s.strip_suffix("secs"))
        .or_else(|| s.strip_suffix("seconds"))
        // Order matters here -- we have to try `s` for seconds _last_, because
        // every other plural and subsecond unit also ends in `s`...
        .or_else(|| s.strip_suffix('s'))
    {
        let s = s.trim();
        return Ok(s
            .parse::<u64>()
            .map(Duration::from_secs)
            .or_else(|_| s.parse::<f64>().map(Duration::from_secs_f64))?);
    }

    Err("expected an integer followed by one of {`ns`, `us`, `ms`, `s`, `sec`, `m`, `min`, `h`, `hours`}".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_parse_durations(expected: Duration, inputs: &[&str]) {
        for input in inputs {
            println!("trying: parse_duration({:?}) -> {:?}", input, expected);
            match parse_duration(input) {
                Err(e) => panic!(
                    "parse_duration({:?}) -> {} (expected {:?})",
                    input, e, expected
                ),
                Ok(dur) => assert_eq!(
                    dur, expected,
                    "parse_duration({:?}) -> {:?} (expected {:?})",
                    input, dur, expected
                ),
            }
        }
    }

    #[test]
    fn parse_hours() {
        test_parse_durations(
            Duration::from_secs(3 * 60 * 60),
            &["3h", "3 h", " 3 h", "3 hours", "3hours"],
        )
    }

    #[test]
    fn parse_mins() {
        test_parse_durations(
            Duration::from_secs(10 * 60),
            &[
                "10m",
                "10 m",
                "10 m",
                "10 minutes",
                "10minutes",
                "  10 minutes",
                "10 min",
                " 10 min",
                "10min",
            ],
        )
    }

    #[test]
    fn parse_secs() {
        test_parse_durations(
            Duration::from_secs(10),
            &[
                "10s",
                "10 s",
                "10 s",
                "10 seconds",
                "10seconds",
                "  10 seconds",
                "10 sec",
                " 10 sec",
                "10sec",
            ],
        )
    }

    #[test]
    fn parse_fractional_hours() {
        test_parse_durations(
            Duration::from_millis(1500 * 60 * 60),
            &["1.5h", "1.5 h", " 1.5 h", "1.5 hours", "1.5hours"],
        )
    }

    #[test]
    fn parse_fractional_mins() {
        test_parse_durations(
            Duration::from_millis(1500 * 60),
            &[
                "1.5m",
                "1.5 m",
                "1.5 m",
                "1.5 minutes",
                "1.5 minutes",
                "  1.5 minutes",
                "1.5 min",
                " 1.5 min",
                "1.5min",
            ],
        )
    }

    #[test]
    fn parse_fractional_secs() {
        test_parse_durations(
            Duration::from_millis(1500),
            &[
                "1.5s",
                "1.5 s",
                "1.5 s",
                "1.5 seconds",
                "1.5 seconds",
                "  1.5 seconds",
                "1.5 sec",
                " 1.5 sec",
                "1.5sec",
            ],
        )
    }
}
