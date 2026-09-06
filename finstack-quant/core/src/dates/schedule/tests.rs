use super::*;
use crate::dates::schedule::builder::strict_fail_closed_on_warnings;

fn warned_schedule() -> Schedule {
    Schedule {
        dates: Vec::new(),
        payment_dates: Vec::new(),
        fixing_dates: Vec::new(),
        warnings: vec![ScheduleWarning::MissingCalendarId {
            calendar_id: "nope".to_string(),
        }],
    }
}

#[test]
fn strict_policy_fails_closed_on_warnings() {
    let err = strict_fail_closed_on_warnings(ScheduleErrorPolicy::Strict, warned_schedule())
        .expect_err("strict must reject schedules carrying warnings");
    let msg = err.to_string();
    assert!(msg.contains("strict policy fails closed"), "got: {msg}");
    assert!(msg.contains("nope"), "got: {msg}");
}

#[test]
fn strict_policy_passes_clean_schedules_through() {
    let clean = Schedule {
        dates: Vec::new(),
        payment_dates: Vec::new(),
        fixing_dates: Vec::new(),
        warnings: Vec::new(),
    };
    assert!(strict_fail_closed_on_warnings(ScheduleErrorPolicy::Strict, clean).is_ok());
}

#[test]
fn non_strict_policies_pass_warnings_through() {
    for policy in [
        ScheduleErrorPolicy::MissingCalendarWarning,
        ScheduleErrorPolicy::GracefulEmpty,
    ] {
        let schedule = strict_fail_closed_on_warnings(policy, warned_schedule())
            .expect("non-strict policies keep warned schedules");
        assert!(schedule.has_warnings());
    }
}
