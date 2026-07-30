# Supported analysis

Commander Deck Analyzer combines list structure, versioned policy data,
reviewed card semantics, known-line data, and bounded simulation.

## Reported areas

- Deck parsing and Commander legality
- Mana production and color access
- Opening-hand composition and mulligan behavior
- Card roles and synergy resources
- Known combo lines
- Modeled threat and win-attempt timing
- Interaction pressure and recovery
- Rules and execution coverage
- Bracket recommendation with supporting reasons

## Executable behavior

Strict analysis requires a complete runtime receipt for every card function
that can affect the metric. The bounded Oracle parser accepts a clause only
when one typed grammar path consumes the complete normalized paragraph. The
compiled result keeps the exact face and clause address together with its
timing, conditions, costs, targets, effects, and restrictions. Card names do
not select behavior.

An accepted clause still needs an exact execution path. The bounded consumer
checks conditions, costs, choices, targets, and restrictions before it commits
a supported action. State changes stay tied to physical objects, zones,
counters, attachments, and controller information. Supported pending actions
retain paid costs, locked choices, legal targets, and last known source
information between initiation and resolution. A failed transaction restores
the staged state.

The simulation bridge binds compiled clauses to stable physical object
identities and returns typed state changes only after a batch commits. Reviewed
printed mechanics are retained as typed procedures only when the mechanic
procedure and its executable clause agree. Face-level modal assembly ties each
branch to its header and preserves branch order. A missing branch, malformed
group, or unsupported branch prevents the modal group from becoming executable.

Runtime receipts bind exact source evidence, clause identity, consumer and
bridge versions, mechanic procedures, and the capabilities supplied to the
simulator. Parser recognition by itself is not execution coverage. A clause
with incomplete source evidence, an unsupported shape, a missing consumer, or
no live simulation bridge remains a visible coverage gap and cannot support a
strict functional claim.

The current runtime includes exact consumers for reviewed mana networks, land
behavior, alternate casts, characteristics, continuous triggers, object
lifecycles, modal effects, tutors, interaction, restrictions, protection, and
creature types.

## Interaction profiles

Every analysis uses the same prepared episodes for a No Interaction baseline
and for the selected aggregate interaction profile. Profile percentages apply
to each eligible opportunity after that profile becomes active:

- No Interaction never applies modeled disruption.
- Mild Interaction begins on turn 3. It uses a 3% engine disruption rate, an
  8% base attempt stop rate, a 1.5% board wipe rate, and no mana pressure.
- Moderate Interaction begins on turn 2. It uses an 8% engine disruption rate,
  a 22% base attempt stop rate, a 4% board wipe rate, and a 3% mana pressure
  rate.
- cEDH Interaction begins on turn 1. It uses a 14% engine disruption rate, a
  40% base attempt stop rate, a 6% board wipe rate, and an 8% mana pressure
  rate.

Board wipe opportunities begin on turn 5 and require at least three creatures
on the modeled battlefield. Mana pressure applies only through turn 4 and can
reduce available mana by at most one. Protection can reduce the chance that an
attempt is stopped.

The eight fixed response scenarios remain separate diagnostics. They do not
change with the selected aggregate profile. The report records the selected
profile, and the cache includes it in analysis identity so results from one
profile cannot be reused for another.

These profile rates are model settings, not measured tournament probabilities.
They are not empirically calibrated.

## Claim boundary

The bracket result is an estimate, not an official ruling. The current model is
not yet calibrated against a large independent corpus.

The analyzer distinguishes:

- structural observations taken directly from the submitted list
- source-backed card and policy data
- behavior represented by the executable model
- unsupported or ambiguous mechanics

Unsupported mechanics remain visible as coverage gaps. They do not become
fully modeled evidence simply because the card or combo is recognized.
