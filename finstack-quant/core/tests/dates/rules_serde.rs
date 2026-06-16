//! Tests for calendar rule serde support

mod serde_tests {
    use finstack_quant_core::dates::{Direction, Observed, Rule};
    use time::{Month, Weekday};

    #[test]
    fn test_observed_serde_roundtrip() {
        let test_cases = vec![
            Observed::None,
            Observed::NextMonday,
            Observed::FriIfSatMonIfSun,
        ];

        for observed in test_cases {
            let json = serde_json::to_string(&observed).unwrap();
            let deserialized: Observed = serde_json::from_str(&json).unwrap();
            assert_eq!(observed, deserialized);
        }
    }

    #[test]
    fn test_direction_serde_roundtrip() {
        let test_cases = vec![Direction::After, Direction::Before];

        for direction in test_cases {
            let json = serde_json::to_string(&direction).unwrap();
            let deserialized: Direction = serde_json::from_str(&json).unwrap();
            assert_eq!(direction, deserialized);
        }
    }

    #[test]
    fn test_rule_serde_roundtrip() {
        let test_cases = vec![
            // Fixed date rules with different observation modes
            Rule::fixed(Month::January, 1),
            Rule::fixed_next_monday(Month::December, 25),
            Rule::fixed_weekend(Month::July, 4),
            // Nth weekday rules
            Rule::NthWeekday {
                n: 3,
                weekday: Weekday::Monday,
                month: Month::January,
            },
            Rule::NthWeekday {
                n: -1,
                weekday: Weekday::Monday,
                month: Month::May,
            },
            // Weekday shift rules
            Rule::WeekdayShift {
                weekday: Weekday::Monday,
                month: Month::May,
                day: 25,
                dir: Direction::Before,
            },
            // Easter offset
            Rule::EasterOffset(-3), // Good Friday
            Rule::EasterOffset(0),  // Easter Monday
            // Chinese calendar rules
            Rule::ChineseNewYear,
            Rule::QingMing,
            Rule::BuddhasBirthday,
            // Japanese equinox rules
            Rule::VernalEquinoxJP,
            Rule::AutumnalEquinoxJP,
        ];

        for rule in test_cases {
            let json = serde_json::to_string(&rule).unwrap();
            let deserialized: Rule = serde_json::from_str(&json).unwrap();

            // Compare based on variant type
            match (rule, deserialized) {
                (
                    Rule::Fixed {
                        month: m1,
                        day: d1,
                        observed: o1,
                    },
                    Rule::Fixed {
                        month: m2,
                        day: d2,
                        observed: o2,
                    },
                ) => {
                    assert_eq!(m1, m2);
                    assert_eq!(d1, d2);
                    assert_eq!(o1, o2);
                }
                (
                    Rule::NthWeekday {
                        n: n1,
                        weekday: w1,
                        month: m1,
                    },
                    Rule::NthWeekday {
                        n: n2,
                        weekday: w2,
                        month: m2,
                    },
                ) => {
                    assert_eq!(n1, n2);
                    assert_eq!(w1, w2);
                    assert_eq!(m1, m2);
                }
                (
                    Rule::WeekdayShift {
                        weekday: w1,
                        month: m1,
                        day: d1,
                        dir: dir1,
                    },
                    Rule::WeekdayShift {
                        weekday: w2,
                        month: m2,
                        day: d2,
                        dir: dir2,
                    },
                ) => {
                    assert_eq!(w1, w2);
                    assert_eq!(m1, m2);
                    assert_eq!(d1, d2);
                    assert_eq!(dir1, dir2);
                }
                (Rule::EasterOffset(o1), Rule::EasterOffset(o2)) => {
                    assert_eq!(o1, o2);
                }
                (Rule::ChineseNewYear, Rule::ChineseNewYear)
                | (Rule::QingMing, Rule::QingMing)
                | (Rule::BuddhasBirthday, Rule::BuddhasBirthday)
                | (Rule::VernalEquinoxJP, Rule::VernalEquinoxJP)
                | (Rule::AutumnalEquinoxJP, Rule::AutumnalEquinoxJP) => {}
                _ => panic!("Rule variant mismatch"),
            }
        }
    }

    #[test]
    fn test_rule_json_format() {
        // Test that the JSON format is as expected with snake_case
        let rule = Rule::fixed_next_monday(Month::January, 1);
        let json = serde_json::to_string(&rule).unwrap();
        assert!(json.contains("\"fixed\""));
        assert!(json.contains("\"next_monday\""));

        let observed = Observed::FriIfSatMonIfSun;
        let json = serde_json::to_string(&observed).unwrap();
        assert_eq!(json, "\"fri_if_sat_mon_if_sun\"");
    }

    #[test]
    fn test_rule_collection_serde() {
        // Test serializing a collection of rules (like in a calendar definition)
        let rules = vec![
            Rule::fixed(Month::January, 1),
            Rule::EasterOffset(-3),
            Rule::NthWeekday {
                n: 4,
                weekday: Weekday::Thursday,
                month: Month::November,
            },
        ];

        let json = serde_json::to_string(&rules).unwrap();
        let deserialized: Vec<Rule> = serde_json::from_str(&json).unwrap();

        assert_eq!(rules.len(), deserialized.len());
        assert_eq!(
            serde_json::to_value(&rules).unwrap(),
            serde_json::to_value(&deserialized).unwrap()
        );
    }
}
