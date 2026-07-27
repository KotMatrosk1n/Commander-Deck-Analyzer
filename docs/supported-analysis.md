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
