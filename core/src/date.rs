//! Datas do jogo ("AAAA-MM-DD", como no banco), sem dependências externas.
//! Conversão dia ↔ data pelo algoritmo `days_from_civil` de Howard Hinnant.

/// Dias desde 1970-01-01.
pub fn days_from_civil(y: i32, m: u32, d: u32) -> i32 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = m as i32;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i32 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Inverso de [`days_from_civil`].
pub fn civil_from_days(z: i32) -> (i32, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

/// "2025-07-01" → dias desde 1970; `None` se não for uma data válida.
pub fn parse(iso: &str) -> Option<i32> {
    let mut it = iso.splitn(3, '-');
    let y: i32 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || d == 0 || d > 31 {
        return None;
    }
    let days = days_from_civil(y, m, d);
    // Rejeita 31 de abril e afins
    (civil_from_days(days) == (y, m, d)).then_some(days)
}

pub fn format(days: i32) -> String {
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Soma `n` dias a uma data ISO.
pub fn add_days(iso: &str, n: i32) -> Option<String> {
    Some(format(parse(iso)? + n))
}

/// 0 = domingo … 6 = sábado.
pub fn weekday(iso: &str) -> Option<u32> {
    // 1970-01-01 foi uma quinta-feira (4)
    Some((parse(iso)? + 4).rem_euclid(7) as u32)
}

/// Idade completa em `on` de quem nasceu em `birth`.
pub fn age(birth: &str, on: &str) -> Option<i32> {
    let (by, bm, bd) = civil_from_days(parse(birth)?);
    let (y, m, d) = civil_from_days(parse(on)?);
    Some(y - by - if (m, d) < (bm, bd) { 1 } else { 0 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_arithmetic() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(format(parse("2025-07-01").unwrap()), "2025-07-01");
        assert_eq!(add_days("2025-06-30", 1).as_deref(), Some("2025-07-01"));
        assert_eq!(add_days("2028-02-28", 1).as_deref(), Some("2028-02-29")); // bissexto
        assert_eq!(add_days("2025-12-31", 1).as_deref(), Some("2026-01-01"));
        assert_eq!(parse("2025-04-31"), None);
        assert_eq!(parse("abc"), None);
    }

    #[test]
    fn weekday_and_age() {
        assert_eq!(weekday("2025-07-01"), Some(2)); // terça-feira, como no CM
        assert_eq!(age("1987-06-24", "2025-07-01"), Some(38));
        assert_eq!(age("2007-07-13", "2025-07-01"), Some(17));
    }
}
