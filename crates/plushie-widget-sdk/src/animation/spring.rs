//! Spring solver using a damped harmonic oscillator.

/// Spring parameters.
#[derive(Debug, Clone, Copy)]
pub struct SpringParams {
    /// Spring constant (higher values return to target faster).
    pub stiffness: f32,
    /// Damping coefficient (higher values suppress oscillation).
    pub damping: f32,
    /// Mass of the animated body (higher feels heavier).
    pub mass: f32,
}

/// Spring state (position + velocity).
#[derive(Debug, Clone, Copy)]
pub struct SpringState {
    /// Current position along the animated axis.
    pub position: f32,
    /// Current velocity.
    pub velocity: f32,
}

/// Settlement thresholds.
const VELOCITY_THRESHOLD: f32 = 0.01;
const POSITION_THRESHOLD: f32 = 0.001;

/// Advances a spring simulation by `dt` seconds.
///
/// Uses the closed-form damped oscillator solution so large frame
/// gaps and stiff custom springs do not depend on Euler step size.
/// Returns `(new_state, settled)`.
pub fn advance(
    state: SpringState,
    target: f32,
    params: &SpringParams,
    dt: f32,
) -> (SpringState, bool) {
    if !params.mass.is_finite()
        || params.mass <= 0.0
        || !params.stiffness.is_finite()
        || !params.damping.is_finite()
        || !dt.is_finite()
        || dt < 0.0
    {
        return (
            SpringState {
                position: target,
                velocity: 0.0,
            },
            true,
        );
    }

    let (new_position, new_velocity) = solve_damped_oscillator(state, target, params, dt);

    if !new_velocity.is_finite() || !new_position.is_finite() {
        return (
            SpringState {
                position: target,
                velocity: 0.0,
            },
            true,
        );
    }

    let settled = new_velocity.abs() < VELOCITY_THRESHOLD
        && (new_position - target).abs() < POSITION_THRESHOLD;

    if settled {
        (
            SpringState {
                position: target,
                velocity: 0.0,
            },
            true,
        )
    } else {
        (
            SpringState {
                position: new_position,
                velocity: new_velocity,
            },
            false,
        )
    }
}

fn solve_damped_oscillator(
    state: SpringState,
    target: f32,
    params: &SpringParams,
    dt: f32,
) -> (f32, f32) {
    if dt == 0.0 {
        return (state.position, state.velocity);
    }

    let x0 = f64::from(state.position - target);
    let v0 = f64::from(state.velocity);
    let t = f64::from(dt);
    let stiffness = f64::from(params.stiffness).max(0.0);
    let damping = f64::from(params.damping).max(0.0);
    let mass = f64::from(params.mass);

    let (x, v) = if stiffness == 0.0 {
        solve_damping_only(x0, v0, damping / mass, t)
    } else {
        let omega0 = (stiffness / mass).sqrt();
        let critical_damping = 2.0 * (stiffness * mass).sqrt();
        let zeta = damping / critical_damping;

        if zeta < 1.0 {
            solve_underdamped(x0, v0, omega0, zeta, t)
        } else if (zeta - 1.0).abs() < f64::EPSILON {
            solve_critical(x0, v0, omega0, t)
        } else {
            solve_overdamped(x0, v0, omega0, zeta, t)
        }
    };

    ((x + f64::from(target)) as f32, v as f32)
}

fn solve_damping_only(x0: f64, v0: f64, damping_per_mass: f64, t: f64) -> (f64, f64) {
    if damping_per_mass == 0.0 {
        (x0 + v0 * t, v0)
    } else {
        let decay = (-damping_per_mass * t).exp();
        let v = v0 * decay;
        let x = x0 + v0 * (1.0 - decay) / damping_per_mass;
        (x, v)
    }
}

fn solve_underdamped(x0: f64, v0: f64, omega0: f64, zeta: f64, t: f64) -> (f64, f64) {
    let decay_rate = zeta * omega0;
    let omega_d = omega0 * (1.0 - zeta * zeta).sqrt();
    let decay = (-decay_rate * t).exp();
    let cos = (omega_d * t).cos();
    let sin = (omega_d * t).sin();
    let b = (v0 + decay_rate * x0) / omega_d;
    let inner = x0 * cos + b * sin;
    let x = decay * inner;
    let v = decay * (-decay_rate * inner + (-x0 * omega_d * sin + b * omega_d * cos));
    (x, v)
}

fn solve_critical(x0: f64, v0: f64, omega0: f64, t: f64) -> (f64, f64) {
    let decay = (-omega0 * t).exp();
    let b = v0 + omega0 * x0;
    let inner = x0 + b * t;
    let x = decay * inner;
    let v = decay * (b - omega0 * inner);
    (x, v)
}

fn solve_overdamped(x0: f64, v0: f64, omega0: f64, zeta: f64, t: f64) -> (f64, f64) {
    let root = (zeta * zeta - 1.0).sqrt();
    let r1 = -omega0 * (zeta - root);
    let r2 = -omega0 * (zeta + root);
    let c1 = (v0 - r2 * x0) / (r1 - r2);
    let c2 = x0 - c1;
    let e1 = (r1 * t).exp();
    let e2 = (r2 * t).exp();
    let x = c1 * e1 + c2 * e2;
    let v = c1 * r1 * e1 + c2 * r2 * e2;
    (x, v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_mass_stops_at_target_with_finite_state() {
        for mass in [0.0, -1.0, f32::INFINITY, f32::NAN] {
            let (state, settled) = advance(
                SpringState {
                    position: 0.0,
                    velocity: 5.0,
                },
                10.0,
                &SpringParams {
                    stiffness: 100.0,
                    damping: 10.0,
                    mass,
                },
                0.016,
            );

            assert!(settled);
            assert_eq!(state.position, 10.0);
            assert_eq!(state.velocity, 0.0);
            assert!(state.position.is_finite());
            assert!(state.velocity.is_finite());
        }
    }

    #[test]
    fn overflowing_integration_stops_at_target_with_finite_state() {
        let (state, settled) = advance(
            SpringState {
                position: 0.0,
                velocity: 0.0,
            },
            10.0,
            &SpringParams {
                stiffness: f32::MAX,
                damping: 0.0,
                mass: f32::MIN_POSITIVE,
            },
            1.0,
        );

        assert!(settled);
        assert_eq!(state.position, 10.0);
        assert_eq!(state.velocity, 0.0);
        assert!(state.position.is_finite());
        assert!(state.velocity.is_finite());
    }

    #[test]
    fn large_delta_matches_repeated_small_deltas_for_stiff_spring() {
        let params = SpringParams {
            stiffness: 4_000.0,
            damping: 80.0,
            mass: 1.0,
        };
        let start = SpringState {
            position: 0.0,
            velocity: 0.0,
        };

        let (single, _) = advance(start, 100.0, &params, 0.1);

        let mut repeated = start;
        for _ in 0..100 {
            repeated = advance(repeated, 100.0, &params, 0.001).0;
        }

        assert!((single.position - repeated.position).abs() < 0.001);
        assert!((single.velocity - repeated.velocity).abs() < 0.001);
    }

    #[test]
    fn high_stiffness_dropped_frame_stays_finite_and_bounded() {
        let (state, settled) = advance(
            SpringState {
                position: 0.0,
                velocity: 0.0,
            },
            1.0,
            &SpringParams {
                stiffness: 100_000.0,
                damping: 900.0,
                mass: 1.0,
            },
            0.1,
        );

        assert!(state.position.is_finite());
        assert!(state.velocity.is_finite());
        assert!((state.position - 1.0).abs() < 0.01);
        assert!(state.velocity.abs() < 0.1);
        assert!(settled || (state.position - 1.0).abs() < 0.01);
    }
}
