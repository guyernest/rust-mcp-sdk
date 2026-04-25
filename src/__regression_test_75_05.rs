// INTENTIONALLY OVER-COMPLEX FOR PHASE 75 WAVE 5 GATE EMPIRICAL TEST
// This file MUST be removed before merge — it exists only to verify CI fails-closed
// on the new `pmat quality-gate --fail-on-violation --checks complexity` step in
// .github/workflows/ci.yml. Placed in src/ (not tests/) per Wave 5 plan post-review
// concern #5 — gate path filter inspects src/.
//
// On exit (Task 5-02), this file is removed by deleting the throwaway branch
// `regression-pr/75-05-gate-empirical-test` from origin and locally; nothing here
// ever lands on main.

#![allow(dead_code)]
#![allow(clippy::cognitive_complexity)]

#[cfg(test)]
pub fn deliberately_complex_for_gate_test(input: i32) -> i32 {
    match input {
        0 => match input + 1 {
            1 => match input + 2 {
                2 => match input + 3 {
                    3 => 1,
                    _ => match input + 4 {
                        4 => 2,
                        _ => 3,
                    },
                },
                _ => match input + 3 {
                    3 => 4,
                    _ => 5,
                },
            },
            _ => match input + 2 {
                2 => 6,
                _ => match input + 3 {
                    3 => 7,
                    _ => 8,
                },
            },
        },
        1 => match input + 1 {
            2 => match input + 2 {
                3 => match input + 3 {
                    4 => 9,
                    _ => match input + 4 {
                        5 => 10,
                        _ => 11,
                    },
                },
                _ => match input + 3 {
                    4 => 12,
                    _ => 13,
                },
            },
            _ => match input + 2 {
                3 => 14,
                _ => match input + 3 {
                    4 => 15,
                    _ => 16,
                },
            },
        },
        2 => match input + 1 {
            3 => match input + 2 {
                4 => match input + 3 {
                    5 => 17,
                    _ => 18,
                },
                _ => 19,
            },
            _ => match input + 2 {
                4 => 20,
                _ => 21,
            },
        },
        3 => match input + 1 {
            4 => match input + 2 {
                5 => 22,
                _ => 23,
            },
            _ => 24,
        },
        4 => {
            if input > 0 {
                if input > 1 {
                    if input > 2 {
                        25
                    } else {
                        26
                    }
                } else {
                    27
                }
            } else {
                28
            }
        }
        _ => 0,
    }
}
