# Third-party notices

Commander Deck Analyzer is an unofficial fan-made analysis tool. It is not
approved by, sponsored by, or affiliated with Wizards of the Coast.

## Open-source software

The application uses open-source Rust and JavaScript packages under their
respective licenses. Exact package versions are recorded in `Cargo.lock` and
`package-lock.json`. Copyright and license terms distributed with those
packages continue to apply.

## Magic and Wizards of the Coast

Magic: The Gathering, card names, Oracle text, rules, and related intellectual
property are owned by Wizards of the Coast and their respective rights holders.
The application does not bundle card artwork or the complete Comprehensive
Rules.

Wizards Fan Content Policy:
https://company.wizards.com/en/legal/fancontentpolicy

Official Magic rules:
https://magic.wizards.com/en/rules

## Card and deck data

Card definitions and optional bulk updates come from Scryfall. Scryfall is not
affiliated with Wizards of the Coast.

Scryfall:
https://scryfall.com/

Scryfall API and bulk data:
https://scryfall.com/docs/api

User-requested public deck imports may contact Archidekt, Deckstats, or
Scryfall Decks. Moxfield links use Moxfield's own export workflow.

Archidekt:
https://archidekt.com/

Deckstats:
https://deckstats.net/

Moxfield:
https://moxfield.com/

Commander Spellbook data can be downloaded only after a user starts the update.
The downloaded catalog is stored and matched locally. It is not included in
the application source or installer.

Commander Spellbook:
https://commanderspellbook.com/

## Optional aggregate data

The source includes an optional TopDeck.gg aggregate import boundary. It uses
the documented tournaments API only after a user supplies a key and starts an
update. The key is not persisted. Player details and raw decklists are not
stored.

Data provided by TopDeck.gg:
https://topdeck.gg/

EDHTop16 is credited as a research reference. No EDHTop16 payload is included
or fetched through an undocumented endpoint.

EDHTop16:
https://edhtop16.com/

EDHREC data is not scraped or bundled. The optional aggregate importer accepts
only a local dataset whose metadata records permission or a published license
for the intended use.

EDHREC:
https://edhrec.com/

## Optional engine boundary

The source includes a versioned host boundary for a future separately installed
phase-rs engine pack. No phase-rs source, worker executable, or card data is
included or active.

phase-rs:
https://github.com/phase-rs/phase
