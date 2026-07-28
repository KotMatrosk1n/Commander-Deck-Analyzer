# Data sources

Commander Deck Analyzer keeps source identity and local storage behavior visible
in each workflow.

## Card definitions

Scryfall supplies card identities, Oracle information, and optional bulk card
data. A full update is user-requested. Missing-card resolution is optional and
disabled by default.

## Deck imports

The app can retrieve a single public deck after the user submits a supported
Archidekt, Deckstats, or Scryfall Decks URL. Moxfield links use Moxfield's own
export workflow.

## Commander policy

The bundled Commander policy snapshot records its effective date and source
URLs. It supports legality and recommendation context. It is not an official
ruling by Wizards of the Coast or the Commander Format Panel.

## Commander Spellbook

A user can request the documented Commander Spellbook export. The catalog is
stored locally and used for attributed known-line matching. It is not included
in this repository or the installer.

## Comprehensive Rules

A user can request the official Wizards text document. The app stores and
indexes the file locally for reference. Downloading the document does not mean
the analyzer executes every rule.
