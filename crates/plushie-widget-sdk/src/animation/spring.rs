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
    let force = -params.stiffness * (state.position - target) - params.damping * state.velocity;
    let acceleration = force / params.mass;
    let new_velocity = state.velocity + acceleration * dt;
    let new_position = state.position + new_velocity * dt;

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
