use super::*;
use pretty_assertions::assert_eq;

#[test]
fn rate_limit_delay_honours_retry_after_within_floor_and_cap() {
    assert_eq!(
        [
            rate_limit_delay(None, 0),
            rate_limit_delay(None, 1),
            rate_limit_delay(None, 3),
            rate_limit_delay(None, 10),
            rate_limit_delay(Some("7"), 0),
            rate_limit_delay(Some("0"), 2),
            rate_limit_delay(Some("600"), 0),
            rate_limit_delay(Some("Wed, 21 Oct 2026 07:28:00 GMT"), 1),
        ],
        [
            Duration::from_secs(2),
            Duration::from_secs(4),
            Duration::from_secs(16),
            Duration::from_secs(30),
            Duration::from_secs(7),
            Duration::from_secs(2),
            Duration::from_secs(30),
            Duration::from_secs(4),
        ]
    );
}
