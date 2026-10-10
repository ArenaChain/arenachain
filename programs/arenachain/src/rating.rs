//! Settlement stats: integer Elo, win/loss record, reputation and forfeit penalties.

use crate::{constants::*, state::PlayerProfile, state::ResultKind};

/// Expected score (in basis points) for a player whose opponent is rated `d` points higher,
/// for d = 0, 25, ..., 800: `round(10_000 / (1 + 10^(d / 400)))`.
const EXPECTED_BPS: [i64; 33] = [
    5000, 4641, 4285, 3937, 3599, 3275, 2966, 2675, 2403, 2150, 1917, 1704, 1510, 1334, 1177, 1035,
    909, 797, 698, 610, 532, 464, 405, 352, 307, 267, 232, 201, 175, 152, 132, 114, 99,
];
const STEP: i64 = 25;
const MAX_DIFF: i64 = 800;

/// Expected score in basis points; `diff` is opponent rating minus own rating.
pub fn expected_score_bps(diff: i64) -> i64 {
    let d = diff.clamp(-MAX_DIFF, MAX_DIFF);
    if d < 0 {
        return 10_000 - expected_score_bps(-d);
    }
    let idx = (d / STEP) as usize;
    if idx + 1 >= EXPECTED_BPS.len() {
        return EXPECTED_BPS[EXPECTED_BPS.len() - 1];
    }
    let rem = d % STEP;
    EXPECTED_BPS[idx] - (EXPECTED_BPS[idx] - EXPECTED_BPS[idx + 1]) * rem / STEP
}

/// Points the winner gains (and the loser loses), rounded to nearest. 0..=K.
pub fn elo_delta(winner_elo: u32, loser_elo: u32) -> u32 {
    let expected = expected_score_bps(loser_elo as i64 - winner_elo as i64);
    ((ELO_K * (10_000 - expected) + 5_000) / 10_000) as u32
}

fn add_reputation(p: &mut PlayerProfile, amount: i32) {
    p.reputation = p.reputation.saturating_add(amount).clamp(0, MAX_REPUTATION);
}

/// Forfeit penalty ladder over the last 30 days: every forfeit costs reputation; the 2nd adds
/// a 24-hour ban and the 3rd or later a 7-day ban. Bans never shorten an existing one.
pub fn apply_forfeit_penalty(p: &mut PlayerProfile, now: i64) {
    let recent = p
        .recent_forfeits
        .iter()
        .filter(|&&t| t > 0 && now.saturating_sub(t) < FORFEIT_WINDOW_SECS)
        .count();
    let count_in_window = recent + 1;

    add_reputation(p, -FORFEIT_REPUTATION_PENALTY);
    let ban_secs = match count_in_window {
        1 => 0,
        2 => SECOND_FORFEIT_BAN_SECS,
        _ => THIRD_FORFEIT_BAN_SECS,
    };
    if ban_secs > 0 {
        p.banned_until = p.banned_until.max(now.saturating_add(ban_secs));
    }
    p.recent_forfeits = [now, p.recent_forfeits[0]];
    p.total_forfeits = p.total_forfeits.saturating_add(1);
}

/// Applies a settled result to both profiles. `Void` changes nothing.
pub fn apply_result(
    winner: &mut PlayerProfile,
    loser: &mut PlayerProfile,
    kind: ResultKind,
    now: i64,
) {
    if kind == ResultKind::Void {
        return;
    }
    let delta = elo_delta(winner.elo, loser.elo);
    winner.elo = winner.elo.saturating_add(delta);
    loser.elo = loser.elo.saturating_sub(delta).max(MIN_ELO);
    winner.wins = winner.wins.saturating_add(1);
    loser.losses = loser.losses.saturating_add(1);

    add_reputation(winner, COMPLETED_MATCH_REPUTATION);
    match kind {
        ResultKind::Win => add_reputation(loser, COMPLETED_MATCH_REPUTATION),
        ResultKind::Forfeit => apply_forfeit_penalty(loser, now),
        ResultKind::Void => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::prelude::Pubkey;

    const DAY: i64 = 24 * 3600;

    fn profile(elo: u32) -> PlayerProfile {
        PlayerProfile {
            authority: Pubkey::default(),
            elo,
            wins: 0,
            losses: 0,
            total_forfeits: 0,
            recent_forfeits: [0; 2],
            reputation: MAX_REPUTATION,
            banned_until: 0,
            steam_id: 0,
            created_at: 0,
            bump: 0,
        }
    }

    #[test]
    fn table_is_monotonic_and_symmetric() {
        let mut prev = i64::MAX;
        for d in -1000..=1000 {
            // Stronger opponent (larger d) never raises the expected score.
            let e = expected_score_bps(d);
            assert!((0..=10_000).contains(&e));
            assert!(e <= prev);
            prev = e;
            assert_eq!(expected_score_bps(d) + expected_score_bps(-d), 10_000);
        }
        assert_eq!(expected_score_bps(0), 5000);
        assert_eq!(expected_score_bps(400), 909);
        assert_eq!(expected_score_bps(5000), 99);
    }

    #[test]
    fn equal_ratings_move_sixteen() {
        assert_eq!(elo_delta(1000, 1000), 16);
    }

    #[test]
    fn upset_moves_more_than_expected_win() {
        assert_eq!(elo_delta(1000, 1400), 29); // underdog wins
        assert_eq!(elo_delta(1400, 1000), 3); // favourite wins
        assert_eq!(elo_delta(3000, 100), 0); // gap capped at 800; favourite gains ~0.3
        assert!(elo_delta(1000, 1400) <= ELO_K as u32);
    }

    #[test]
    fn win_is_zero_sum_above_floor() {
        for (w, l) in [(1000, 1000), (1200, 900), (850, 1500), (2400, 2390)] {
            let (mut a, mut b) = (profile(w), profile(l));
            apply_result(&mut a, &mut b, ResultKind::Win, 0);
            assert_eq!(a.elo + b.elo, w + l);
            assert_eq!((a.wins, a.losses, b.wins, b.losses), (1, 0, 0, 1));
        }
    }

    #[test]
    fn loser_floors_at_min_elo() {
        let (mut a, mut b) = (profile(MIN_ELO + 10), profile(MIN_ELO + 5)); // delta ~16
        apply_result(&mut a, &mut b, ResultKind::Win, 0);
        assert_eq!(b.elo, MIN_ELO);
    }

    #[test]
    fn win_gives_both_reputation_capped() {
        let (mut a, mut b) = (profile(1000), profile(1000));
        b.reputation = 50;
        apply_result(&mut a, &mut b, ResultKind::Win, 0);
        assert_eq!(a.reputation, MAX_REPUTATION);
        assert_eq!(b.reputation, 51);
    }

    #[test]
    fn void_changes_nothing() {
        let (mut a, mut b) = (profile(1000), profile(1000));
        apply_result(&mut a, &mut b, ResultKind::Void, 0);
        assert_eq!((a.elo, b.elo, a.wins, b.losses), (1000, 1000, 0, 0));
    }

    #[test]
    fn forfeit_ladder() {
        let now = 100 * DAY;
        let mut p = profile(1000);
        apply_forfeit_penalty(&mut p, now);
        assert_eq!((p.reputation, p.banned_until, p.total_forfeits), (90, 0, 1));

        apply_forfeit_penalty(&mut p, now + DAY);
        assert_eq!(p.reputation, 80);
        assert_eq!(p.banned_until, now + DAY + SECOND_FORFEIT_BAN_SECS);

        apply_forfeit_penalty(&mut p, now + 2 * DAY);
        assert_eq!(p.reputation, 70);
        assert_eq!(p.banned_until, now + 2 * DAY + THIRD_FORFEIT_BAN_SECS);

        // Still 3 in the window: another 7-day ban.
        apply_forfeit_penalty(&mut p, now + 10 * DAY);
        assert_eq!(p.banned_until, now + 10 * DAY + THIRD_FORFEIT_BAN_SECS);
        assert_eq!(p.total_forfeits, 4);
    }

    #[test]
    fn old_forfeits_leave_the_window() {
        let mut p = profile(1000);
        apply_forfeit_penalty(&mut p, DAY);
        apply_forfeit_penalty(&mut p, 32 * DAY); // first is 31 days old
        assert_eq!(p.banned_until, 0);
        apply_forfeit_penalty(&mut p, 33 * DAY); // only day 32 is in the window -> 2nd
        assert_eq!(p.banned_until, 33 * DAY + SECOND_FORFEIT_BAN_SECS);
    }

    #[test]
    fn forfeit_exactly_thirty_days_later_starts_a_new_window() {
        let mut p = profile(1000);
        apply_forfeit_penalty(&mut p, DAY);
        apply_forfeit_penalty(&mut p, DAY + FORFEIT_WINDOW_SECS);
        assert_eq!(
            p.banned_until, 0,
            "a forfeit 30 days old is outside the window"
        );
        apply_forfeit_penalty(&mut p, DAY + FORFEIT_WINDOW_SECS + 1);
        assert_eq!(
            p.banned_until,
            DAY + FORFEIT_WINDOW_SECS + 1 + SECOND_FORFEIT_BAN_SECS
        );
    }

    #[test]
    fn ban_never_shortens() {
        let mut p = profile(1000);
        p.banned_until = 1_000 * DAY;
        apply_forfeit_penalty(&mut p, DAY);
        apply_forfeit_penalty(&mut p, 2 * DAY);
        assert_eq!(p.banned_until, 1_000 * DAY);
    }

    #[test]
    fn reputation_floors_at_zero() {
        let mut p = profile(1000);
        p.reputation = 5;
        apply_forfeit_penalty(&mut p, DAY);
        assert_eq!(p.reputation, 0);
    }

    #[test]
    fn forfeit_result_penalizes_loser_and_rewards_winner() {
        let (mut a, mut b) = (profile(1000), profile(1000));
        a.reputation = 90;
        apply_result(&mut a, &mut b, ResultKind::Forfeit, DAY);
        assert_eq!((a.elo, b.elo), (1016, 984));
        assert_eq!((a.reputation, b.reputation), (91, 90));
        assert_eq!(b.total_forfeits, 1);
    }
}
