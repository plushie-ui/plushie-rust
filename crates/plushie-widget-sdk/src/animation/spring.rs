//! Spring solver using a damped harmonic oscillator.

/// Spring parameters.
pub struct SpringParams {
    /// Spring constant (higher values return to target faster).
    pub stiffness: f32,
    /// Damping coefficient (higher values suppress oscillation).
    pub damping: f32,
    /// Mass of the animated body (higher feels heavier).
    pub mass: f32,
}

/// Spring state (position + velocity).
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
/// Uses semi-implicit Euler integration for stability.
/// Returns `(new_state, settled)`.
pub fn advance(
    state: SpringState,
    target: f32,
    params: &SpringParams,
    dt: f32,
) -> (SpringState, bool) {
    if !params.mass.is_finite() || params.mass <= 0.0 {
        return (
            SpringState {
                position: target,
                velocity: 0.0,
            },
            true,
        );
    }

    let force = -params.stiffness * (state.position - target) - params.damping * state.velocity;
    let acceleration = force / params.mass;
    let new_velocity = state.velocity + acceleration * dt;
    let new_position = state.position + new_velocity * dt;

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
}
