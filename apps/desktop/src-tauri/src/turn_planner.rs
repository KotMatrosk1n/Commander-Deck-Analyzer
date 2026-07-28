//! Deterministic, fail-closed bounded planning for a single turn.
//!
//! This module deliberately knows nothing about cards, Oracle text, or the
//! strict execution engine. A caller supplies an observation-safe domain
//! implementation. The planner never receives a library order, an unrevealed
//! card, an opponent's private hand, or any other hidden future information
//! unless the domain incorrectly places it in [`TurnPlanningDomain::ObservableState`].
//!
//! Search is bounded by beam width, executable child expansions, and action
//! depth. An optional horizon may cross exactly one turn boundary through a
//! conservative, observation-safe transition supplied by the domain.

use std::cmp::Ordering;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

pub const TURN_PLANNER_VERSION: &str = "bounded-beam-0.6";

/// Fixed lexicographic objective for every planner integration.
///
/// Larger values are always better. Field declaration order is intentional:
/// derived ordering compares table conversion before threat creation, then
/// route progress, development, protection, and scarce-resource preservation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlannerValue {
    pub executable_table_conversion: i64,
    pub credible_executable_threat: i64,
    pub route_deficit_reduction: i64,
    pub card_mana_development: i64,
    pub protection_preservation: i64,
    pub scarce_resource_preservation: i64,
}

/// The same ordered vector is used to decide which strategically equivalent
/// state dominates another.
pub type DominanceVector = PlannerValue;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerStateEvaluation<Endpoint> {
    pub endpoint: Option<Endpoint>,
    pub value: PlannerValue,
    pub dominance: DominanceVector,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanningHorizon {
    CurrentTurnOnly,
    CurrentTurnPlusConservativeNextTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlannerConfig {
    /// Maximum number of states retained at each executable action depth.
    pub beam_width: usize,
    /// Maximum number of executable child states produced by `apply_action`.
    pub max_node_expansions: usize,
    /// Maximum number of executable actions in a returned line.
    pub max_actions: usize,
    /// Whether a completed current turn may cross one conservative boundary.
    pub horizon: PlanningHorizon,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            beam_width: 64,
            max_node_expansions: 10_000,
            max_actions: 16,
            horizon: PlanningHorizon::CurrentTurnOnly,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlannerDiagnostics {
    /// Number of executable children successfully produced.
    pub expanded: usize,
    /// Total candidates excluded by report-only filtering, canonical
    /// deduplication/dominance, or beam truncation.
    pub pruned: usize,
    /// Candidates that shared a canonical state key at the same search depth.
    pub deduplicated: usize,
    /// Non-executable/report-only actions rejected before application.
    pub report_only_excluded: usize,
    /// Candidate actions whose transactional application rejected the current
    /// branch without invalidating the rest of the search.
    pub application_rejected: usize,
    /// Candidates removed specifically by beam-width truncation.
    pub beam_pruned: usize,
    /// Search had more executable children but could not apply one because the
    /// configured expansion budget was exhausted.
    pub node_cap_reached: bool,
    /// Cooperative cancellation was observed before search naturally ended.
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedLine<State, Action, Endpoint> {
    pub actions: Vec<Action>,
    pub final_state: State,
    pub endpoint: Option<Endpoint>,
    pub value: PlannerValue,
    pub dominance: DominanceVector,
    pub used_conservative_next_turn: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerResult<State, Action, Endpoint> {
    pub best: PlannedLine<State, Action, Endpoint>,
    pub diagnostics: PlannerDiagnostics,
}

pub type DomainPlannerResult<D> = Result<
    PlannerResult<
        <D as TurnPlanningDomain>::ObservableState,
        <D as TurnPlanningDomain>::Action,
        <D as TurnPlanningDomain>::Endpoint,
    >,
    PlannerError<<D as TurnPlanningDomain>::Error>,
>;

#[derive(Debug, PartialEq, Eq)]
pub enum PlannerError<DomainError> {
    InvalidConfig(&'static str),
    /// Stable action keys must be unique within one observed state. Silently
    /// retaining either duplicate would make results depend on enumeration
    /// order.
    DuplicateActionTieBreakKey,
    Domain(DomainError),
}

/// Observation-safe domain contract for bounded turn planning.
///
/// `ObservableState` must contain only information the acting player may use at
/// the decision point. In particular, `legal_actions` and
/// `conservative_next_turn_state` must not inspect or reveal hidden future
/// information. Enumerated actions should represent concrete executable
/// candidates; [`Self::is_executable_action`] is a second fail-closed gate for
/// report-only inferences that share an upstream collection with executable
/// actions. A domain may explicitly classify a transactional application
/// error as a local rejection when exact payment or another late-bound public
/// precondition can invalidate one otherwise well-formed candidate.
pub trait TurnPlanningDomain {
    type ObservableState: Clone;
    type Action: Clone;
    type StateKey: Clone + Ord;
    type ActionKey: Clone + Ord;
    type Endpoint: Clone + PartialEq + Eq;
    type Error;

    /// Enumerate actions using only the supplied observable state.
    fn legal_actions(&self, state: &Self::ObservableState) -> Vec<Self::Action>;

    /// Exclude report-only, inferred, or otherwise non-executable actions.
    fn is_executable_action(&self, _state: &Self::ObservableState, _action: &Self::Action) -> bool {
        true
    }

    /// Apply one executable action transactionally. The planner supplies a
    /// fresh clone, so mutation can never affect a sibling branch or the
    /// caller's initial state.
    fn apply_action(
        &self,
        state: &mut Self::ObservableState,
        action: &Self::Action,
    ) -> Result<(), Self::Error>;

    /// Classify a transactional application failure as a local candidate
    /// rejection. The default preserves fail-closed propagation: domains must
    /// opt in explicitly, and unexpected errors still abort the search.
    fn action_error_is_recoverable(&self, _error: &Self::Error) -> bool {
        false
    }

    /// Canonical public-state identity used for deterministic deduplication.
    fn canonical_state_key(&self, state: &Self::ObservableState) -> Self::StateKey;

    /// Stable, semantic action key. It must not depend on display names or
    /// enumeration order and must be unique among a state's legal actions.
    fn action_tie_break_key(&self, action: &Self::Action) -> Self::ActionKey;

    /// Ordered strategic value used to rank different canonical states.
    fn value_vector(&self, state: &Self::ObservableState) -> PlannerValue;

    /// Ordered resource/position value used when canonical keys collide.
    fn dominance_vector(&self, state: &Self::ObservableState) -> DominanceVector {
        self.value_vector(state)
    }

    /// Classify a terminal endpoint. `None` means planning may continue.
    fn terminal_endpoint(&self, state: &Self::ObservableState) -> Option<Self::Endpoint>;

    /// Evaluate a state for the three values stored on a search node.
    ///
    /// Domains whose endpoint, strategic value, and dominance share expensive
    /// intermediate work may override this method to compute that work once.
    /// The default preserves the independent trait methods exactly.
    fn evaluate_state(
        &self,
        state: &Self::ObservableState,
    ) -> PlannerStateEvaluation<Self::Endpoint> {
        PlannerStateEvaluation {
            endpoint: self.terminal_endpoint(state),
            value: self.value_vector(state),
            dominance: self.dominance_vector(state),
        }
    }

    /// Signals that no more current-turn actions should be generated.
    fn current_turn_complete(&self, _state: &Self::ObservableState) -> bool {
        false
    }

    /// Advance a completed current turn without revealing a random draw or any
    /// other hidden future fact. It is called at most once per line and only
    /// when the configured horizon opts in.
    fn conservative_next_turn_state(
        &self,
        _state: &Self::ObservableState,
    ) -> Result<Option<Self::ObservableState>, Self::Error> {
        Ok(None)
    }
}

struct ActionPathStep<Action> {
    parent: Option<usize>,
    action: Action,
}

// Each successful expansion owns one action here. Search nodes retain only a
// tail index, so sibling branches do not allocate and clone complete action
// histories. The separate flattened action key path remains unchanged because
// it is used directly by the deterministic ordering hot path.
struct ActionPathArena<Action> {
    steps: Vec<ActionPathStep<Action>>,
}

impl<Action> ActionPathArena<Action> {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            steps: Vec::with_capacity(capacity),
        }
    }

    fn append(&mut self, parent: Option<usize>, action: Action) -> usize {
        let index = self.steps.len();
        self.steps.push(ActionPathStep { parent, action });
        index
    }

    fn materialize(&self, mut tail: Option<usize>) -> Vec<Action>
    where
        Action: Clone,
    {
        let mut actions = Vec::new();
        while let Some(index) = tail {
            let step = &self.steps[index];
            actions.push(step.action.clone());
            tail = step.parent;
        }
        actions.reverse();
        actions
    }
}

#[derive(Clone)]
struct SearchNode<State, StateKey, ActionKey, Endpoint> {
    state: State,
    state_key: StateKey,
    action_path_tail: Option<usize>,
    action_keys: Vec<ActionKey>,
    endpoint: Option<Endpoint>,
    value: PlannerValue,
    dominance: DominanceVector,
    used_conservative_next_turn: bool,
}

type DomainSearchNode<D> = SearchNode<
    <D as TurnPlanningDomain>::ObservableState,
    <D as TurnPlanningDomain>::StateKey,
    <D as TurnPlanningDomain>::ActionKey,
    <D as TurnPlanningDomain>::Endpoint,
>;

/// Run deterministic bounded beam search.
///
/// Cancellation returns the strongest line found so far with
/// `diagnostics.cancelled = true`. Domain application failures and ambiguous
/// tie-break keys are returned as errors because either condition invalidates
/// the executable search contract.
pub fn plan_turn<D>(
    domain: &D,
    initial_state: D::ObservableState,
    config: PlannerConfig,
    cancellation: &AtomicBool,
) -> DomainPlannerResult<D>
where
    D: TurnPlanningDomain,
{
    validate_config(config)?;

    let mut diagnostics = PlannerDiagnostics::default();
    let mut action_paths =
        ActionPathArena::with_capacity(config.beam_width.min(config.max_node_expansions));
    let root = make_node(domain, initial_state, None, Vec::new(), false);
    let mut best = root.clone();
    let mut frontier = vec![root];

    for _action_depth in 0..config.max_actions {
        if cancellation.load(AtomicOrdering::Relaxed) {
            diagnostics.cancelled = true;
            break;
        }

        sort_nodes(&mut frontier);
        let mut generated = Vec::new();
        let mut stop_search = false;

        for mut node in frontier.drain(..) {
            if cancellation.load(AtomicOrdering::Relaxed) {
                diagnostics.cancelled = true;
                stop_search = true;
                break;
            }

            if node.endpoint.is_some() {
                continue;
            }

            if domain.current_turn_complete(&node.state) {
                if config.horizon == PlanningHorizon::CurrentTurnPlusConservativeNextTurn
                    && !node.used_conservative_next_turn
                {
                    let Some(next_state) = domain
                        .conservative_next_turn_state(&node.state)
                        .map_err(PlannerError::Domain)?
                    else {
                        continue;
                    };
                    node = make_node(
                        domain,
                        next_state,
                        node.action_path_tail,
                        node.action_keys,
                        true,
                    );
                    consider_best(&node, &mut best);
                    if node.endpoint.is_some() || domain.current_turn_complete(&node.state) {
                        continue;
                    }
                } else {
                    continue;
                }
            }

            let mut keyed_actions = domain
                .legal_actions(&node.state)
                .into_iter()
                .map(|action| (domain.action_tie_break_key(&action), action))
                .collect::<Vec<_>>();
            keyed_actions.sort_by(|left, right| left.0.cmp(&right.0));
            if keyed_actions
                .windows(2)
                .any(|window| window[0].0 == window[1].0)
            {
                return Err(PlannerError::DuplicateActionTieBreakKey);
            }

            for (action_key, action) in keyed_actions {
                if cancellation.load(AtomicOrdering::Relaxed) {
                    diagnostics.cancelled = true;
                    stop_search = true;
                    break;
                }
                if !domain.is_executable_action(&node.state, &action) {
                    diagnostics.report_only_excluded += 1;
                    diagnostics.pruned += 1;
                    continue;
                }
                if diagnostics.expanded == config.max_node_expansions {
                    diagnostics.node_cap_reached = true;
                    stop_search = true;
                    break;
                }

                let mut next_state = node.state.clone();
                if let Err(error) = domain.apply_action(&mut next_state, &action) {
                    if domain.action_error_is_recoverable(&error) {
                        diagnostics.application_rejected += 1;
                        diagnostics.pruned += 1;
                        continue;
                    }
                    return Err(PlannerError::Domain(error));
                }
                diagnostics.expanded += 1;
                let mut action_keys = node.action_keys.clone();
                action_keys.push(action_key);
                let action_path_tail = Some(action_paths.append(node.action_path_tail, action));
                let child = make_node(
                    domain,
                    next_state,
                    action_path_tail,
                    action_keys,
                    node.used_conservative_next_turn,
                );
                generated.push(child);
            }

            if stop_search {
                break;
            }
        }

        if generated.is_empty() {
            break;
        }

        // Sorting once avoids cloning every potentially large state key into
        // a tree node and replaces O(n log n) map insertion with one stable
        // sort plus a linear grouped pass. `sort_by` is stable, so candidates
        // with the same canonical key are compared for dominance in the same
        // generation order as the former ordered-map implementation.
        generated.sort_by(|left, right| {
            left.used_conservative_next_turn
                .cmp(&right.used_conservative_next_turn)
                .then_with(|| left.state_key.cmp(&right.state_key))
        });
        let mut unique = Vec::<DomainSearchNode<D>>::with_capacity(generated.len());
        for candidate in generated {
            let duplicate = unique.last_mut().filter(|incumbent| {
                incumbent.used_conservative_next_turn == candidate.used_conservative_next_turn
                    && incumbent.state_key == candidate.state_key
            });
            if let Some(incumbent) = duplicate {
                diagnostics.deduplicated += 1;
                diagnostics.pruned += 1;
                if node_dominates(&candidate, incumbent) {
                    *incumbent = candidate;
                }
            } else {
                unique.push(candidate);
            }
        }

        frontier = unique;
        sort_nodes(&mut frontier);
        if frontier.len() > config.beam_width {
            let removed = frontier.len() - config.beam_width;
            diagnostics.beam_pruned += removed;
            diagnostics.pruned += removed;
            frontier.truncate(config.beam_width);
        }
        for candidate in &frontier {
            consider_best(candidate, &mut best);
        }
        if stop_search {
            break;
        }
    }

    let actions = action_paths.materialize(best.action_path_tail);
    Ok(PlannerResult {
        best: PlannedLine {
            actions,
            final_state: best.state,
            endpoint: best.endpoint,
            value: best.value,
            dominance: best.dominance,
            used_conservative_next_turn: best.used_conservative_next_turn,
        },
        diagnostics,
    })
}

fn validate_config<DomainError>(config: PlannerConfig) -> Result<(), PlannerError<DomainError>> {
    if config.beam_width == 0 {
        return Err(PlannerError::InvalidConfig(
            "planner beam_width must be greater than zero",
        ));
    }
    if config.max_node_expansions == 0 {
        return Err(PlannerError::InvalidConfig(
            "planner max_node_expansions must be greater than zero",
        ));
    }
    if config.max_actions == 0 {
        return Err(PlannerError::InvalidConfig(
            "planner max_actions must be greater than zero",
        ));
    }
    Ok(())
}

fn make_node<D>(
    domain: &D,
    state: D::ObservableState,
    action_path_tail: Option<usize>,
    action_keys: Vec<D::ActionKey>,
    used_conservative_next_turn: bool,
) -> DomainSearchNode<D>
where
    D: TurnPlanningDomain,
{
    let evaluation = domain.evaluate_state(&state);
    SearchNode {
        state_key: domain.canonical_state_key(&state),
        endpoint: evaluation.endpoint,
        value: evaluation.value,
        dominance: evaluation.dominance,
        state,
        action_path_tail,
        action_keys,
        used_conservative_next_turn,
    }
}

fn consider_best<State, StateKey, ActionKey, Endpoint>(
    candidate: &SearchNode<State, StateKey, ActionKey, Endpoint>,
    best: &mut SearchNode<State, StateKey, ActionKey, Endpoint>,
) where
    State: Clone,
    StateKey: Clone + Ord,
    ActionKey: Clone + Ord,
    Endpoint: Clone,
{
    if node_is_better(candidate, best) {
        *best = candidate.clone();
    }
}

fn sort_nodes<State, StateKey, ActionKey, Endpoint>(
    nodes: &mut [SearchNode<State, StateKey, ActionKey, Endpoint>],
) where
    StateKey: Ord,
    ActionKey: Ord,
{
    nodes.sort_by(compare_nodes);
}

fn compare_nodes<State, StateKey, ActionKey, Endpoint>(
    left: &SearchNode<State, StateKey, ActionKey, Endpoint>,
    right: &SearchNode<State, StateKey, ActionKey, Endpoint>,
) -> Ordering
where
    StateKey: Ord,
    ActionKey: Ord,
{
    right
        .value
        .cmp(&left.value)
        .then_with(|| right.dominance.cmp(&left.dominance))
        .then_with(|| left.action_keys.cmp(&right.action_keys))
        .then_with(|| left.state_key.cmp(&right.state_key))
}

fn node_is_better<State, StateKey, ActionKey, Endpoint>(
    candidate: &SearchNode<State, StateKey, ActionKey, Endpoint>,
    incumbent: &SearchNode<State, StateKey, ActionKey, Endpoint>,
) -> bool
where
    StateKey: Ord,
    ActionKey: Ord,
{
    compare_nodes(candidate, incumbent) == Ordering::Less
}

fn node_dominates<State, StateKey, ActionKey, Endpoint>(
    candidate: &SearchNode<State, StateKey, ActionKey, Endpoint>,
    incumbent: &SearchNode<State, StateKey, ActionKey, Endpoint>,
) -> bool
where
    StateKey: Ord,
    ActionKey: Ord,
{
    candidate.dominance > incumbent.dominance
        || (candidate.dominance == incumbent.dominance && node_is_better(candidate, incumbent))
}
