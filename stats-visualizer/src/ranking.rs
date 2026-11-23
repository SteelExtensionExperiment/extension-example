//! Ranking and W/L ratio.

use std::cmp::Ordering;

/// Number of players shown on a board. A fixed layout size, not configuration.
pub const TOP_SIZE: usize = 5;

/// Ranking input; callers adapt their records to it.
pub struct RankingEntry<'a> {
    pub name: &'a str,
    pub wins: u32,
    pub losses: u32,
}

/// One ranked row, derived from the entries.
#[derive(Debug, Clone, PartialEq)]
pub struct RankingLine {
    pub rank: u8,
    pub name: String,
    pub wins: u32,
    pub ratio: f64,
}

/// Wins per loss; with no losses the ratio is the win count.
#[must_use]
pub fn ratio(wins: u32, losses: u32) -> f64 {
    f64::from(wins) / f64::from(losses.max(1))
}

#[must_use]
pub fn format_ratio(ratio: f64) -> String {
    format!("{ratio:.2}")
}

/// Orders by ratio descending, comparing `wins / max(losses, 1)` exactly by cross-multiplication.
fn ratio_desc(a: &RankingEntry, b: &RankingEntry) -> Ordering {
    let left = u64::from(a.wins) * u64::from(b.losses.max(1));
    let right = u64::from(b.wins) * u64::from(a.losses.max(1));
    right.cmp(&left)
}

/// Case-insensitive then case-sensitive, so the order is total and stable.
fn name_asc(a: &str, b: &str) -> Ordering {
    let lower = |s: &str| s.chars().flat_map(char::to_lowercase).collect::<Vec<_>>();
    lower(a).cmp(&lower(b)).then_with(|| a.cmp(b))
}

/// Top [`TOP_SIZE`] players with at least one win: wins desc, then ratio desc, then name asc.
#[must_use]
pub fn rank<'a>(entries: impl IntoIterator<Item = RankingEntry<'a>>) -> Vec<RankingLine> {
    let mut winners: Vec<_> = entries.into_iter().filter(|e| e.wins > 0).collect();
    winners.sort_by(|a, b| {
        b.wins
            .cmp(&a.wins)
            .then_with(|| ratio_desc(a, b))
            .then_with(|| name_asc(a.name, b.name))
    });
    // The rank range bounds the output, so no numeric conversion is needed.
    (1..=u8::MAX)
        .zip(winners.into_iter().take(TOP_SIZE))
        .map(|(rank, e)| RankingLine {
            rank,
            name: e.name.to_owned(),
            wins: e.wins,
            ratio: ratio(e.wins, e.losses),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, wins: u32, losses: u32) -> RankingEntry<'_> {
        RankingEntry { name, wins, losses }
    }

    fn names(lines: &[RankingLine]) -> Vec<&str> {
        lines.iter().map(|l| l.name.as_str()).collect()
    }

    #[test]
    fn equal_wins_fall_to_ratio_then_name() {
        let lines = rank([
            entry("zed", 10, 5),
            entry("bob", 10, 2),
            entry("Amy", 10, 5),
            entry("amy", 10, 5),
            entry("top", 11, 100),
        ]);
        assert_eq!(names(&lines), ["top", "bob", "Amy", "amy", "zed"]);
        assert_eq!(
            lines.iter().map(|l| l.rank).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn zero_losses_uses_wins_as_ratio() {
        let lines = rank([entry("a", 7, 0)]);
        assert!((lines[0].ratio - 7.0).abs() < f64::EPSILON);
        // 7 wins / 0 losses ranks above 7 wins / 2 losses.
        let lines = rank([entry("b", 7, 2), entry("c", 7, 0)]);
        assert_eq!(names(&lines), ["c", "b"]);
    }

    #[test]
    fn players_without_wins_are_excluded() {
        let lines = rank([entry("loser", 0, 9), entry("idle", 0, 0), entry("w", 1, 0)]);
        assert_eq!(names(&lines), ["w"]);
    }

    #[test]
    fn fewer_than_five_gives_only_those_and_more_are_cut() {
        assert_eq!(rank([entry("a", 2, 0), entry("b", 1, 0)]).len(), 2);
        assert_eq!(rank(std::iter::empty()).len(), 0);
        let many = (1..=8).map(|w| entry("p", w, 0));
        let lines = rank(many);
        assert_eq!(lines.len(), TOP_SIZE);
        assert_eq!(lines[0].wins, 8);
    }

    #[test]
    fn max_counts_do_not_overflow() {
        let lines = rank([
            entry("a", u32::MAX, u32::MAX),
            entry("b", u32::MAX, u32::MAX - 1),
        ]);
        assert_eq!(names(&lines), ["b", "a"]);
        assert!((lines[1].ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ratio_has_two_decimals() {
        assert_eq!(format_ratio(ratio(2, 3)), "0.67");
        assert_eq!(format_ratio(ratio(5, 0)), "5.00");
        assert_eq!(format_ratio(ratio(1, 6)), "0.17");
    }
}
