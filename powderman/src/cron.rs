//! A five-field cron parser, because this is eighty lines and a dependency
//! is forever.
//!
//! Supports what crontab supports and nothing else: `*`, `n`, `a-b`, `a,b,c`,
//! and `*/n` or `a-b/n`. No `@daily`, no month names, no seconds field. A bad
//! expression is an error at registration rather than at 06:00 — a schedule
//! that silently never fires is the worst failure this file can have.

use anyhow::{Result, bail};
use chrono::{DateTime, Datelike, Local, Timelike};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct Cron {
    minute: BTreeSet<u32>,
    hour: BTreeSet<u32>,
    dom: BTreeSet<u32>,
    month: BTreeSet<u32>,
    dow: BTreeSet<u32>,
    /// True when day-of-month and day-of-week are *both* restricted.
    dom_and_dow_both_set: bool,
    pub source: String,
}

const FIELDS: [(&str, u32, u32); 5] = [
    ("minute", 0, 59),
    ("hour", 0, 23),
    ("day-of-month", 1, 31),
    ("month", 1, 12),
    ("day-of-week", 0, 6),
];

fn field(spec: &str, name: &str, min: u32, max: u32) -> Result<BTreeSet<u32>> {
    let mut out = BTreeSet::new();
    for part in spec.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((r, s)) => (
                r,
                match s.parse::<u32>() {
                    Ok(n) if n >= 1 => n,
                    _ => bail!("cron: bad step {s:?} in {name} field {spec:?}"),
                },
            ),
            None => (part, 1),
        };
        let (lo, hi) = if range == "*" {
            (min, max)
        } else if let Some((a, b)) = range.split_once('-') {
            match (a.parse::<u32>(), b.parse::<u32>()) {
                (Ok(a), Ok(b)) => (a, b),
                _ => bail!("cron: bad range {range:?} in {name} field"),
            }
        } else {
            match range.parse::<u32>() {
                Ok(v) => (v, v),
                Err(_) => bail!("cron: bad value {range:?} in {name} field"),
            }
        };
        if lo < min || hi > max || lo > hi {
            bail!("cron: {name} field out of range in {spec:?} (allowed {min}-{max})");
        }
        let mut v = lo;
        while v <= hi {
            out.insert(v);
            v += step;
        }
    }
    Ok(out)
}

pub fn parse(source: &str) -> Result<Cron> {
    let parts: Vec<&str> = source.split_whitespace().collect();
    if parts.len() != 5 {
        bail!("cron: expected 5 fields, got {} in {source:?}", parts.len());
    }
    let mut sets = Vec::with_capacity(5);
    for (i, part) in parts.iter().enumerate() {
        let (name, min, max) = FIELDS[i];
        sets.push(field(part, name, min, max)?);
    }
    Ok(Cron {
        minute: sets[0].clone(),
        hour: sets[1].clone(),
        dom: sets[2].clone(),
        month: sets[3].clone(),
        dow: sets[4].clone(),
        dom_and_dow_both_set: parts[2] != "*" && parts[4] != "*",
        source: source.split_whitespace().collect::<Vec<_>>().join(" "),
    })
}

impl Cron {
    /// Does `at` fall in a firing minute?
    ///
    /// The day-of-month / day-of-week rule is cron's genuinely surprising
    /// one: when **both** are restricted the fields are OR-ed, not AND-ed, so
    /// `0 6 1 * 1` fires on the first of the month *and* on every Monday.
    /// Vixie cron does this and everyone copied it; being subtly different
    /// would astonish more than matching it does.
    pub fn matches(&self, at: DateTime<Local>) -> bool {
        if !self.minute.contains(&at.minute()) {
            return false;
        }
        if !self.hour.contains(&at.hour()) {
            return false;
        }
        if !self.month.contains(&at.month()) {
            return false;
        }
        let dom = self.dom.contains(&at.day());
        let dow = self.dow.contains(&at.weekday().num_days_from_sunday());
        if self.dom_and_dow_both_set {
            dom || dow
        } else {
            dom && dow
        }
    }

    /// The next firing time strictly after `from`. For display, not dispatch.
    pub fn next_after(&self, from: DateTime<Local>) -> Result<DateTime<Local>> {
        let mut t = from
            .with_second(0)
            .and_then(|t| t.with_nanosecond(0))
            .unwrap_or(from)
            + chrono::Duration::minutes(1);
        // Four years covers every leap-year case; past that it cannot fire.
        for _ in 0..(366 * 4 * 24 * 60) {
            if self.matches(t) {
                return Ok(t);
            }
            t += chrono::Duration::minutes(1);
        }
        bail!("cron: {:?} never fires", self.source)
    }
}

/// The wall-clock minute an instant falls in — the unit a schedule fires at
/// most once per.
pub fn minute_of(at: DateTime<Local>) -> i64 {
    at.timestamp().div_euclid(60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(s: &str) -> DateTime<Local> {
        let naive =
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").expect("test timestamp");
        Local
            .from_local_datetime(&naive)
            .single()
            .expect("unambiguous local time")
    }

    #[test]
    fn rejects_wrong_field_count() {
        assert!(parse("0 6 * *").is_err());
        assert!(parse("0 6 * * * *").is_err());
    }

    #[test]
    fn rejects_out_of_range_instead_of_never_firing() {
        // The failure this guards: an expression that parses, registers, and
        // then silently never fires.
        assert!(parse("0 25 * * *").is_err());
        assert!(parse("0 6 32 * *").is_err());
        assert!(parse("0 6 * 13 *").is_err());
        assert!(parse("0 6 * * 9").is_err());
        assert!(parse("*/0 * * * *").is_err());
    }

    #[test]
    fn daily_at_six_fires_only_in_that_minute() {
        let c = parse("0 6 * * *").unwrap();
        assert!(c.matches(at("2026-08-09 06:00:00")));
        assert!(!c.matches(at("2026-08-09 06:01:00")));
        assert!(!c.matches(at("2026-08-09 05:00:00")));
    }

    #[test]
    fn every_fifteen_minutes() {
        let c = parse("*/15 * * * *").unwrap();
        let hits: Vec<u32> = [0, 10, 15, 20, 30, 45, 50]
            .into_iter()
            .filter(|m| c.matches(at(&format!("2026-08-09 09:{m:02}:00"))))
            .collect();
        assert_eq!(hits, vec![0, 15, 30, 45]);
    }

    #[test]
    fn lists_and_ranges() {
        let c = parse("0 9,17 * * 1-5").unwrap();
        assert!(c.matches(at("2026-08-10 09:00:00"))); // Monday
        assert!(c.matches(at("2026-08-10 17:00:00")));
        assert!(!c.matches(at("2026-08-10 13:00:00")));
        assert!(!c.matches(at("2026-08-09 09:00:00"))); // Sunday
    }

    #[test]
    fn dom_and_dow_are_ored_when_both_restricted() {
        let c = parse("0 6 1 * 1").unwrap();
        assert!(c.matches(at("2026-08-01 06:00:00"))); // the 1st, a Saturday
        assert!(c.matches(at("2026-08-10 06:00:00"))); // a Monday
        assert!(!c.matches(at("2026-08-11 06:00:00"))); // Tuesday the 11th
    }

    #[test]
    fn next_after_finds_the_following_minute() {
        let c = parse("0 6 * * *").unwrap();
        assert_eq!(
            c.next_after(at("2026-08-09 05:59:00")).unwrap(),
            at("2026-08-09 06:00:00")
        );
    }
}
