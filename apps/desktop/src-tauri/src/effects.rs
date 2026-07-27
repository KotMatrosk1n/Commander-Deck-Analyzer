//! Versioned Oracle-text effect descriptors.
//!
//! These descriptors deliberately capture only behavior the bounded simulator
//! or policy evaluator can consume. Unmodeled clauses remain attached to the
//! card and lower confidence instead of being silently treated as irrelevant.

use std::sync::LazyLock;

use regex::Regex;

use crate::domain::CardDefinition;
use crate::mana::{ManaColorMask, parse_mana_cost};

pub(crate) const EFFECT_DESCRIPTOR_VERSION: &str = "oracle-effects-0.11";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum EffectMagnitude {
    #[default]
    None,
    Fixed(u8),
    Dynamic,
}

impl EffectMagnitude {
    pub fn conservative_value(self, dynamic_fallback: u8) -> u8 {
        match self {
            Self::None => 0,
            Self::Fixed(value) => value,
            Self::Dynamic => dynamic_fallback,
        }
    }

    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Dynamic, _) | (_, Self::Dynamic) => Self::Dynamic,
            (Self::Fixed(left), Self::Fixed(right)) => {
                Self::Fixed(left.saturating_add(right).min(12))
            }
            (Self::None, value) | (value, Self::None) => value,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TutorScope {
    #[default]
    None,
    Restricted,
    AnyCard,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TutorSourceZone {
    #[default]
    None,
    Library,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TutorTarget {
    #[default]
    None,
    AnyCard,
    BasicLand,
    Land,
    Creature,
    Artifact,
    Enchantment,
    Instant,
    Sorcery,
    InstantOrSorcery,
    ArtifactOrEnchantment,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TutorDestination {
    #[default]
    None,
    Hand,
    LibraryTop,
    BattlefieldUntapped,
    BattlefieldTapped,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TutorTiming {
    #[default]
    None,
    /// The search is a direct instruction of an instant or sorcery after its
    /// printed mana cost has been paid.
    SpellResolution,
    /// Search text exists, but its trigger, activation, additional cost, or
    /// condition is outside the bounded executor.
    Unsupported,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TutorInstruction {
    pub source: TutorSourceZone,
    pub target: TutorTarget,
    pub destination: TutorDestination,
    pub quantity: u8,
    pub reveal: bool,
    /// Randomize the remaining searched library after this instruction has
    /// finished moving cards. This is attached only to the final instruction
    /// produced by a single search clause, and only when that clause explicitly
    /// instructs its controller to shuffle.
    pub shuffle_after: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TutorDescriptor {
    pub timing: TutorTiming,
    pub instructions: Vec<TutorInstruction>,
    /// Fixed life loss that follows the complete search instruction during
    /// this spell's resolution. Zero means the reviewed Oracle text has no
    /// such clause; variable or differently ordered clauses remain
    /// unsupported instead of being approximated.
    pub life_loss_after_resolution: u8,
}

impl TutorDescriptor {
    pub fn is_executable_on_spell_resolution(&self) -> bool {
        self.timing == TutorTiming::SpellResolution && !self.instructions.is_empty()
    }
}

/// Structural card traits used by typed library-search matching. These are
/// compiled from the printed type line and cannot be supplied by semantic
/// role overrides.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CardTypeProfile {
    pub is_land: bool,
    pub is_basic_land: bool,
    pub is_creature: bool,
    pub is_artifact: bool,
    pub is_enchantment: bool,
    pub is_instant: bool,
    pub is_sorcery: bool,
}

/// Exact characteristics of the physical card while it is in its owner's
/// hand.
///
/// Double-faced cards use only their front face outside the stack and
/// battlefield (CR 712.8a). Adventure, Omen, and preparation cards likewise
/// use their normal characteristics in hand (CR 715.4, 720.4, and 722.4).
/// Keeping this separate from the root/combined Scryfall fields prevents a
/// modal double-faced card with a nonland front and a land back from being
/// treated as a land card in hand. The same distinction matters for effects
/// that inspect a hand card's colors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HandZoneCharacteristics {
    /// False means the source record did not contain enough face data to bind
    /// the hand characteristics exactly. Consumers must fail closed rather
    /// than infer eligibility from empty fields.
    pub exact: bool,
    pub type_line: String,
    pub card_types: CardTypeProfile,
    pub colors: Vec<String>,
}

impl TutorTarget {
    pub fn matches(self, card_types: CardTypeProfile) -> bool {
        match self {
            Self::None => false,
            Self::AnyCard => true,
            Self::BasicLand => card_types.is_basic_land,
            Self::Land => card_types.is_land,
            Self::Creature => card_types.is_creature,
            Self::Artifact => card_types.is_artifact,
            Self::Enchantment => card_types.is_enchantment,
            Self::Instant => card_types.is_instant,
            Self::Sorcery => card_types.is_sorcery,
            Self::InstantOrSorcery => card_types.is_instant || card_types.is_sorcery,
            Self::ArtifactOrEnchantment => card_types.is_artifact || card_types.is_enchantment,
        }
    }
}

/// The only mana-production lifecycles the bounded trajectory model can
/// execute without inventing a payment, trigger, or reusable permanent.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ManaProductionKind {
    #[default]
    None,
    /// A clean instant/sorcery Add-mana instruction after its mana cost has
    /// been paid (for example, Dark Ritual).
    SpellResolution,
    /// A permanent with a tap-only Add-mana activation. It remains available
    /// on later turns (for example, Arcane Signet or Llanowar Elves).
    ReusableActivated,
    /// A permanent whose entire modeled activation cost is sacrificing itself,
    /// optionally with a tap cost that is immediately legal for a noncreature
    /// permanent (for example, Lotus Petal).
    OneShotActivated,
    /// A reviewed permanent with a tap-only mana activation whose complete
    /// Oracle lifecycle prevents normal untapping. The trajectory model may
    /// use its first activation while retaining the permanent, but must not
    /// refresh that mana source on later turns.
    NonRefreshingActivated,
    /// Mana text exists, but at least one cost, restriction, trigger, or timing
    /// prerequisite is outside the bounded executor.
    Unsupported,
}

#[derive(Debug, Clone, Default)]
pub struct EffectDescriptor {
    pub card_types: CardTypeProfile,
    pub hand_zone: HandZoneCharacteristics,
    pub draw_cards: EffectMagnitude,
    pub impulse_access: EffectMagnitude,
    pub tutor_scope: TutorScope,
    pub tutor: TutorDescriptor,
    pub lands_to_battlefield: EffectMagnitude,
    pub mana_produced: EffectMagnitude,
    pub mana_production_kind: ManaProductionKind,
    pub creature_tokens: EffectMagnitude,
    pub treasure_tokens: EffectMagnitude,
    pub targeted_removal: bool,
    pub board_wipe: bool,
    pub mass_land_denial: bool,
    pub protection: bool,
    pub recursion: bool,
    pub extra_turns: EffectMagnitude,
    pub repeatable: bool,
    pub conditional: bool,
    pub requires_tap: bool,
    pub requires_sacrifice: bool,
    pub requires_discard: bool,
    pub life_cost: bool,
    pub unsupported_clauses: Vec<String>,
    pub confidence: f32,
}

pub fn compile_effect_descriptor(card: &CardDefinition) -> EffectDescriptor {
    let oracle = normalize_oracle(&card.oracle_text);
    let card_types = compile_card_types(&card.type_line);
    let hand_zone = compile_hand_zone_characteristics(card);
    if oracle.is_empty() {
        return EffectDescriptor {
            card_types,
            hand_zone,
            confidence: if card.type_line.to_ascii_lowercase().contains("land") {
                0.96
            } else {
                0.42
            },
            ..Default::default()
        };
    }

    let mut descriptor = EffectDescriptor {
        card_types,
        hand_zone,
        repeatable: oracle.contains("whenever ")
            || oracle.contains("at the beginning of ")
            || oracle.contains("{t}:")
            || oracle.contains(": draw ")
            || oracle.contains(": create "),
        conditional: contains_conditional_language(&oracle),
        requires_tap: oracle.contains("{t}"),
        requires_sacrifice: oracle.contains("sacrifice ")
            && (oracle.contains(':') || oracle.contains("as an additional cost")),
        requires_discard: oracle.contains("discard ")
            && (oracle.contains(':') || oracle.contains("as an additional cost")),
        life_cost: LIFE_COST.is_match(&oracle),
        ..Default::default()
    };

    for captures in DRAW_CARDS.captures_iter(&oracle) {
        descriptor.draw_cards = descriptor
            .draw_cards
            .merge(parse_magnitude(captures.get(1).map(|value| value.as_str())));
    }
    if oracle.contains("draw cards equal to")
        || oracle.contains("draw that many cards")
        || oracle.contains("draw x cards")
    {
        descriptor.draw_cards = EffectMagnitude::Dynamic;
    }

    for captures in LOOK_AT_TOP.captures_iter(&oracle) {
        descriptor.impulse_access = descriptor
            .impulse_access
            .merge(parse_magnitude(captures.get(1).map(|value| value.as_str())));
    }
    if oracle.contains("exile the top")
        && (oracle.contains("you may play") || oracle.contains("you may cast"))
    {
        descriptor.impulse_access = descriptor.impulse_access.merge(EffectMagnitude::Fixed(1));
    }

    if oracle.contains("search your library") {
        let tutor_compilation = compile_tutor_descriptor(card, &oracle);
        descriptor.tutor_scope = tutor_compilation.scope;
        descriptor.tutor = tutor_compilation.descriptor;
        descriptor.lands_to_battlefield = tutor_compilation.lands_to_battlefield;
        descriptor
            .unsupported_clauses
            .extend(tutor_compilation.unsupported_clauses);
    }

    descriptor.mana_produced = parse_add_mana(&oracle);
    descriptor.mana_production_kind =
        classify_mana_production(card, &oracle, descriptor.mana_produced);
    if descriptor.mana_production_kind == ManaProductionKind::ReusableActivated {
        descriptor.repeatable = true;
    } else if descriptor.mana_production_kind == ManaProductionKind::NonRefreshingActivated {
        descriptor.repeatable = false;
    }
    for captures in CREATE_TOKENS.captures_iter(&oracle) {
        let magnitude = parse_magnitude(captures.get(1).map(|value| value.as_str()));
        let token_text = captures
            .get(2)
            .map(|value| value.as_str())
            .unwrap_or_default();
        if token_text.contains("treasure") {
            descriptor.treasure_tokens = descriptor.treasure_tokens.merge(magnitude);
        }
        if token_text.contains("creature") {
            descriptor.creature_tokens = descriptor.creature_tokens.merge(magnitude);
        }
    }
    if oracle.contains("create x ") || oracle.contains("create that many ") {
        if oracle.contains("treasure") {
            descriptor.treasure_tokens = EffectMagnitude::Dynamic;
        }
        if oracle.contains("creature token") {
            descriptor.creature_tokens = EffectMagnitude::Dynamic;
        }
    }

    descriptor.targeted_removal = (oracle.contains("destroy target")
        || oracle.contains("exile target")
        || oracle.contains("return target") && oracle.contains("owner's hand")
        || oracle.contains("target creature gets -"))
        && !oracle.contains("you control");
    descriptor.board_wipe = oracle.contains("destroy all")
        || contains_mass_battlefield_exile(&oracle)
        || oracle.contains("all creatures get -")
        || oracle.contains("each player sacrifices all");
    descriptor.mass_land_denial = is_mass_land_denial(&oracle);
    descriptor.protection = oracle.contains("hexproof")
        || oracle.contains("indestructible")
        || oracle.contains("phase out")
        || oracle.contains("protection from")
        || oracle.contains("counter target spell or ability that targets");
    descriptor.recursion = oracle.contains("graveyard")
        && (oracle.contains("return target")
            || oracle.contains("return up to")
            || oracle.contains("you may cast")
            || oracle.contains("cast target"));
    descriptor.extra_turns = if oracle.contains("take two extra turns") {
        EffectMagnitude::Fixed(2)
    } else if oracle.contains("take an extra turn") || oracle.contains("take one extra turn") {
        EffectMagnitude::Fixed(1)
    } else {
        EffectMagnitude::None
    };

    descriptor
        .unsupported_clauses
        .extend(unsupported_clauses(&oracle));
    descriptor.unsupported_clauses.sort();
    descriptor.unsupported_clauses.dedup();
    if descriptor.mana_production_kind == ManaProductionKind::Unsupported {
        descriptor
            .unsupported_clauses
            .push("Mana activation cost or timing".into());
    }
    let modeled_signal_count = [
        descriptor.draw_cards != EffectMagnitude::None,
        descriptor.impulse_access != EffectMagnitude::None,
        descriptor.tutor_scope != TutorScope::None,
        descriptor.lands_to_battlefield != EffectMagnitude::None,
        descriptor.mana_produced != EffectMagnitude::None,
        descriptor.creature_tokens != EffectMagnitude::None,
        descriptor.treasure_tokens != EffectMagnitude::None,
        descriptor.targeted_removal,
        descriptor.board_wipe,
        descriptor.mass_land_denial,
        descriptor.protection,
        descriptor.recursion,
        descriptor.extra_turns != EffectMagnitude::None,
    ]
    .into_iter()
    .filter(|modeled| *modeled)
    .count();
    let dynamic_count = [
        descriptor.draw_cards,
        descriptor.impulse_access,
        descriptor.lands_to_battlefield,
        descriptor.mana_produced,
        descriptor.creature_tokens,
        descriptor.treasure_tokens,
        descriptor.extra_turns,
    ]
    .iter()
    .filter(|magnitude| matches!(magnitude, EffectMagnitude::Dynamic))
    .count();
    let activation_cost_count = [
        descriptor.requires_tap,
        descriptor.requires_sacrifice,
        descriptor.requires_discard,
        descriptor.life_cost,
    ]
    .into_iter()
    .filter(|required| *required)
    .count();
    let base_confidence = if modeled_signal_count == 0 {
        // A non-empty Oracle box without a supported effect must not be
        // reported as highly modeled merely because it parsed successfully.
        0.54
    } else {
        0.90 + modeled_signal_count.min(2) as f32 * 0.02
    };
    descriptor.confidence = (base_confidence
        - dynamic_count as f32 * 0.055
        - usize::from(descriptor.conditional) as f32 * 0.025
        - usize::from(descriptor.repeatable) as f32 * 0.025
        - activation_cost_count as f32 * 0.0125
        - descriptor.unsupported_clauses.len().min(4) as f32 * 0.075)
        .clamp(0.38, 0.97);
    descriptor
}

#[derive(Debug, Default)]
struct TutorCompilation {
    scope: TutorScope,
    descriptor: TutorDescriptor,
    lands_to_battlefield: EffectMagnitude,
    unsupported_clauses: Vec<String>,
}

fn compile_card_types(type_line: &str) -> CardTypeProfile {
    let normalized = type_line
        .replace(['\u{2014}', '-'], " ")
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let has = |word: &str| normalized.iter().any(|part| part == word);
    CardTypeProfile {
        is_land: has("land"),
        is_basic_land: has("basic") && has("land"),
        is_creature: has("creature"),
        is_artifact: has("artifact"),
        is_enchantment: has("enchantment"),
        is_instant: has("instant"),
        is_sorcery: has("sorcery"),
    }
}

fn compile_hand_zone_characteristics(card: &CardDefinition) -> HandZoneCharacteristics {
    let layout = card.layout.trim().to_ascii_lowercase();
    let face = match layout.as_str() {
        // These layouts have one rules-defined front/main face that supplies
        // the physical card's characteristics in hand. Scryfall preserves that
        // face at index zero and supplies face-local colors for these layouts.
        "transform" | "modal_dfc" | "double_faced_token" | "reversible_card" => {
            let Some(front) = card.faces.first() else {
                return HandZoneCharacteristics::default();
            };
            Some((
                front.name.as_str(),
                &front.type_line,
                &front.colors,
                &front.color_indicator,
                front.mana_cost.as_deref(),
                front.oracle_text.as_str(),
                front.keywords.as_slice(),
            ))
        }
        // Scryfall represents the normal/front characteristics of these
        // single-card, multi-frame layouts at face index zero, but currently
        // reports their actual card color at the root rather than per face.
        // `adventure` also covers Omen cards; `prepare` is the newer
        // preparation-card layout.
        "flip" | "adventure" | "prepare" => {
            let Some(normal) = card.faces.first() else {
                return HandZoneCharacteristics::default();
            };
            Some((
                normal.name.as_str(),
                &normal.type_line,
                &card.colors,
                &card.color_indicator,
                normal.mana_cost.as_deref().or(card.mana_cost.as_deref()),
                normal.oracle_text.as_str(),
                normal.keywords.as_slice(),
            ))
        }
        // Split-card characteristics are combined in hand (CR 709.4), which
        // is exactly what Scryfall's root fields contain.
        "split" => None,
        // A new multiface layout must be reviewed before combined root fields
        // can be called exact hand characteristics. This makes future card
        // frames fail closed instead of repeating the MDFC land-back bug.
        _ if !card.faces.is_empty() => return HandZoneCharacteristics::default(),
        _ => None,
    };

    let (card_name, type_line, colors, color_indicator, mana_cost, oracle_text, keywords) = face
        .unwrap_or((
            card.name.as_str(),
            &card.type_line,
            &card.colors,
            &card.color_indicator,
            card.mana_cost.as_deref(),
            card.oracle_text.as_str(),
            card.keywords.as_slice(),
        ));
    let type_line = type_line.trim();
    if type_line.is_empty() {
        return HandZoneCharacteristics::default();
    }

    HandZoneCharacteristics {
        exact: true,
        type_line: type_line.to_string(),
        card_types: compile_card_types(type_line),
        colors: exact_hand_colors(
            card_name,
            colors,
            color_indicator,
            mana_cost,
            oracle_text,
            keywords,
        ),
    }
}

fn oracle_has_self_color_characteristic(
    normalized_oracle: &str,
    card_name: &str,
    characteristic: &str,
) -> bool {
    let normalized_name = normalize_oracle(card_name);
    if normalized_name.is_empty() {
        return false;
    }
    let named_clause = format!("{normalized_name} {characteristic}");
    let this_card_clause = format!("this card {characteristic}");
    normalized_oracle
        .split('.')
        .map(str::trim)
        .any(|clause| clause == named_clause || clause == this_card_clause)
}

/// Recover the rules-defined color characteristic when a retained local
/// record has an exact hand face and mana cost but an empty Scryfall `colors`
/// array. Card color normally follows colored mana symbols and/or a color
/// indicator (CR 105.2); explicit color-setting characteristic abilities such
/// as Devoid or “is colorless” take precedence. This is card color, not color
/// identity, so off-color rules/reminder text is never consulted.
fn exact_hand_colors(
    card_name: &str,
    reported_colors: &[String],
    color_indicator: &[String],
    mana_cost: Option<&str>,
    oracle_text: &str,
    keywords: &[String],
) -> Vec<String> {
    let normalized_oracle = normalize_oracle(oracle_text);
    let explicitly_colorless = keywords
        .iter()
        .any(|keyword| keyword.eq_ignore_ascii_case("Devoid"))
        || oracle_text.lines().any(|line| {
            matches!(
                normalize_oracle(line).as_str(),
                "devoid" | "devoid (this card has no color.)"
            )
        })
        || oracle_has_self_color_characteristic(&normalized_oracle, card_name, "is colorless");
    if explicitly_colorless {
        return Vec::new();
    }

    if oracle_has_self_color_characteristic(&normalized_oracle, card_name, "is all colors") {
        return ["W", "U", "B", "R", "G"]
            .into_iter()
            .map(str::to_string)
            .collect();
    }

    let parsed_cost = parse_mana_cost(mana_cost);
    [
        ("W", ManaColorMask::WHITE),
        ("U", ManaColorMask::BLUE),
        ("B", ManaColorMask::BLACK),
        ("R", ManaColorMask::RED),
        ("G", ManaColorMask::GREEN),
    ]
    .into_iter()
    .filter(|(symbol, color)| {
        reported_colors
            .iter()
            .chain(color_indicator)
            .any(|reported| reported.eq_ignore_ascii_case(symbol))
            || reported_colors.is_empty()
                && color_indicator.is_empty()
                && parsed_cost.colors.intersects(*color)
    })
    .map(|(symbol, _)| symbol.to_string())
    .collect()
}

fn compile_tutor_descriptor(card: &CardDefinition, oracle: &str) -> TutorCompilation {
    let mut result = TutorCompilation {
        descriptor: TutorDescriptor {
            timing: TutorTiming::Unsupported,
            ..Default::default()
        },
        ..Default::default()
    };

    if oracle.matches("search your library").count() != 1 {
        result
            .unsupported_clauses
            .push("Multiple or linked library searches".into());
        return result;
    }

    let reveal_clause_is_simple = [
        "reveal it",
        "reveal that card",
        "reveal the card",
        "reveal those cards",
        "reveal them",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase));
    if oracle.contains("reveal ") && !reveal_clause_is_simple {
        result
            .unsupported_clauses
            .push("Search reveal or selection sequencing".into());
    }
    if oracle.contains("instead") {
        result
            .unsupported_clauses
            .push("Search replacement effect".into());
    }
    if [
        "at random",
        "an opponent chooses",
        "target opponent chooses",
        "separate those cards",
        "separate them",
        "face down",
        "under an opponent's control",
        "under target opponent's control",
        "choose one or more",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        result
            .unsupported_clauses
            .push("Search reveal or selection sequencing".into());
    }

    let Some(after_search) = oracle
        .split_once("search your library for ")
        .map(|(_, rest)| rest)
    else {
        result
            .unsupported_clauses
            .push("Search source or target scope".into());
        return result;
    };
    let target_phrase = after_search
        .split([',', '.'])
        .next()
        .unwrap_or_default()
        .trim();
    let Some((target, quantity)) = parse_tutor_target(target_phrase) else {
        result
            .unsupported_clauses
            .push("Search target predicate".into());
        result.unsupported_clauses.sort();
        result.unsupported_clauses.dedup();
        return result;
    };
    result.scope = if target == TutorTarget::AnyCard {
        TutorScope::AnyCard
    } else {
        TutorScope::Restricted
    };

    let reveal = oracle.contains("reveal ");
    let shuffle_after = search_clause_explicitly_shuffles(oracle);
    let life_loss_after_resolution = parse_tutor_resolution_life_loss(oracle);
    if oracle.contains("you lose ") && life_loss_after_resolution.is_none() {
        result
            .unsupported_clauses
            .push("Search-linked life loss".into());
    }
    if reveal
        && ![
            "reveal it",
            "reveal that card",
            "reveal the card",
            "reveal those cards",
            "reveal them",
        ]
        .iter()
        .any(|phrase| oracle.contains(phrase))
    {
        result
            .unsupported_clauses
            .push("Search reveal or selection sequencing".into());
    }

    let type_line = card.type_line.to_ascii_lowercase();
    let is_spell = type_line.contains("instant") || type_line.contains("sorcery");
    let has_variable_cost = card
        .mana_cost
        .as_deref()
        .is_some_and(|cost| cost.to_ascii_lowercase().contains("{x}"));
    let lacks_payable_printed_cost = card
        .mana_cost
        .as_deref()
        .is_none_or(|cost| cost.trim().is_empty());
    let unsupported_timing_or_cost = !is_spell
        || has_variable_cost
        || lacks_payable_printed_cost
        || contains_conditional_language(oracle)
        || oracle.contains("as an additional cost")
        || oracle.contains("when ")
        || oracle.contains("whenever ")
        || oracle.contains("at the beginning of ")
        || oracle
            .split_once("search your library")
            .is_some_and(|(before, _)| before.contains(':'));
    if unsupported_timing_or_cost {
        result
            .unsupported_clauses
            .push("Search timing or cost".into());
    }

    if oracle.contains("instead") {
        result
            .unsupported_clauses
            .push("Search replacement effect".into());
    }
    if [
        "at random",
        "an opponent chooses",
        "target opponent chooses",
        "separate those cards",
        "separate them",
        "face down",
        "under an opponent's control",
        "under target opponent's control",
        "choose one or more",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        result
            .unsupported_clauses
            .push("Search reveal or selection sequencing".into());
    }

    let mut instructions = Vec::new();
    if target == TutorTarget::BasicLand
        && quantity == 2
        && oracle.contains("put one onto the battlefield tapped")
        && (oracle.contains("the other into your hand")
            || oracle.contains("another into your hand"))
    {
        instructions.push(TutorInstruction {
            source: TutorSourceZone::Library,
            target,
            destination: TutorDestination::BattlefieldTapped,
            quantity: 1,
            reveal,
            shuffle_after: false,
        });
        instructions.push(TutorInstruction {
            source: TutorSourceZone::Library,
            target,
            destination: TutorDestination::Hand,
            quantity: 1,
            reveal,
            shuffle_after,
        });
    } else if let Some(destination) = parse_tutor_destination(oracle) {
        if matches!(
            destination,
            TutorDestination::BattlefieldTapped | TutorDestination::BattlefieldUntapped
        ) && !matches!(target, TutorTarget::BasicLand | TutorTarget::Land)
        {
            result
                .unsupported_clauses
                .push("Search battlefield target execution".into());
        }
        instructions.push(TutorInstruction {
            source: TutorSourceZone::Library,
            target,
            destination,
            quantity,
            reveal,
            shuffle_after,
        });
    } else {
        result.unsupported_clauses.push("Search destination".into());
    }

    result.unsupported_clauses.sort();
    result.unsupported_clauses.dedup();
    if result.unsupported_clauses.is_empty() && !instructions.is_empty() {
        result.descriptor = TutorDescriptor {
            timing: TutorTiming::SpellResolution,
            instructions,
            life_loss_after_resolution: life_loss_after_resolution.unwrap_or_default(),
        };
        let land_count = result
            .descriptor
            .instructions
            .iter()
            .filter(|instruction| {
                matches!(
                    instruction.target,
                    TutorTarget::BasicLand | TutorTarget::Land
                ) && matches!(
                    instruction.destination,
                    TutorDestination::BattlefieldTapped | TutorDestination::BattlefieldUntapped
                )
            })
            .fold(0u8, |total, instruction| {
                total.saturating_add(instruction.quantity)
            });
        result.lands_to_battlefield = match land_count {
            0 => EffectMagnitude::None,
            count => EffectMagnitude::Fixed(count),
        };
    }
    result
}

fn parse_tutor_resolution_life_loss(oracle: &str) -> Option<u8> {
    let captures = TUTOR_RESOLUTION_LIFE_LOSS.captures(oracle)?;
    captures
        .get(1)?
        .as_str()
        .parse::<u16>()
        .ok()
        .and_then(|value| u8::try_from(value).ok())
}

fn search_clause_explicitly_shuffles(oracle: &str) -> bool {
    let Some((_, after_search)) = oracle.split_once("search your library") else {
        return false;
    };

    let mut sentences = after_search.split('.');
    let search_sentence = sentences.next().unwrap_or_default();
    if search_sentence.contains("then shuffle") || search_sentence.contains("shuffle your library")
    {
        return true;
    }

    // Some printed wordings put the shuffle in a standalone sentence directly
    // after the search instructions. Do not scan farther: a later shuffle may
    // belong to an unrelated effect or ability.
    let immediately_following_sentence = sentences.next().unwrap_or_default().trim();
    immediately_following_sentence.starts_with("then shuffle")
        || immediately_following_sentence.starts_with("shuffle your library")
}

fn parse_tutor_target(phrase: &str) -> Option<(TutorTarget, u8)> {
    match phrase {
        "a card" | "one card" | "up to one card" => Some((TutorTarget::AnyCard, 1)),
        "a basic land card" | "one basic land card" | "up to one basic land card" => {
            Some((TutorTarget::BasicLand, 1))
        }
        "up to two basic land cards" | "two basic land cards" => Some((TutorTarget::BasicLand, 2)),
        "a land card" | "one land card" | "up to one land card" => Some((TutorTarget::Land, 1)),
        "a creature card" | "one creature card" | "up to one creature card" => {
            Some((TutorTarget::Creature, 1))
        }
        "an artifact card" | "one artifact card" | "up to one artifact card" => {
            Some((TutorTarget::Artifact, 1))
        }
        "an enchantment card" | "one enchantment card" | "up to one enchantment card" => {
            Some((TutorTarget::Enchantment, 1))
        }
        "an instant card" | "one instant card" | "up to one instant card" => {
            Some((TutorTarget::Instant, 1))
        }
        "a sorcery card" | "one sorcery card" | "up to one sorcery card" => {
            Some((TutorTarget::Sorcery, 1))
        }
        "an instant or sorcery card" => Some((TutorTarget::InstantOrSorcery, 1)),
        "an artifact or enchantment card" => Some((TutorTarget::ArtifactOrEnchantment, 1)),
        _ => None,
    }
}

fn parse_tutor_destination(oracle: &str) -> Option<TutorDestination> {
    if [
        "put that card into your hand",
        "put the card into your hand",
        "put it into your hand",
        "put those cards into your hand",
        "put them into your hand",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        Some(TutorDestination::Hand)
    } else if [
        "put that card on top of your library",
        "put the card on top of your library",
        "put it on top of your library",
        "put that card on top",
        "put the card on top",
        "put it on top",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        Some(TutorDestination::LibraryTop)
    } else if [
        "put that card onto the battlefield tapped",
        "put the card onto the battlefield tapped",
        "put it onto the battlefield tapped",
        "put those cards onto the battlefield tapped",
        "put them onto the battlefield tapped",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        Some(TutorDestination::BattlefieldTapped)
    } else if [
        "put that card onto the battlefield",
        "put the card onto the battlefield",
        "put it onto the battlefield",
        "put those cards onto the battlefield",
        "put them onto the battlefield",
    ]
    .iter()
    .any(|phrase| oracle.contains(phrase))
    {
        Some(TutorDestination::BattlefieldUntapped)
    } else {
        None
    }
}

fn classify_mana_production(
    card: &CardDefinition,
    oracle: &str,
    magnitude: EffectMagnitude,
) -> ManaProductionKind {
    if magnitude == EffectMagnitude::None {
        return ManaProductionKind::None;
    }

    let type_line = card.type_line.to_ascii_lowercase();
    let is_spell = type_line.contains("instant") || type_line.contains("sorcery");
    if is_spell {
        return if spell_mana_resolution_is_supported(oracle, magnitude) {
            ManaProductionKind::SpellResolution
        } else {
            ManaProductionKind::Unsupported
        };
    }

    if reviewed_nonrefreshing_mana_activation(card, oracle, magnitude) {
        return ManaProductionKind::NonRefreshingActivated;
    }

    if oracle.contains("activate only")
        || oracle.contains("spend this mana only")
        || oracle.contains("spend that mana only")
        || oracle.contains("suspend ")
        || has_nonrefreshing_mana_lifecycle(oracle)
        || oracle.contains("would enter the battlefield") && oracle.contains("instead")
    {
        return ManaProductionKind::Unsupported;
    }

    let activation_costs = ADD_MANA_ABILITY
        .captures_iter(oracle)
        .filter_map(|captures| captures.get(1).map(|cost| cost.as_str().trim()))
        .collect::<Vec<_>>();
    if activation_costs.is_empty() {
        // Triggered, replacement, and static mana text needs an event/state
        // model. Do not turn it into an ETB burst or a reusable source.
        return ManaProductionKind::Unsupported;
    }

    let mut kind = None;
    for cost in activation_costs {
        let Some(candidate) = classify_mana_activation_cost(card, cost) else {
            return ManaProductionKind::Unsupported;
        };
        if kind.is_some_and(|current| current != candidate) {
            return ManaProductionKind::Unsupported;
        }
        kind = Some(candidate);
    }
    kind.unwrap_or(ManaProductionKind::Unsupported)
}

fn reviewed_nonrefreshing_mana_activation(
    card: &CardDefinition,
    oracle: &str,
    magnitude: EffectMagnitude,
) -> bool {
    if magnitude != EffectMagnitude::Fixed(3)
        || !card.type_line.trim().eq_ignore_ascii_case("artifact")
    {
        return false;
    }

    let card_name = normalize_rules_name(&card.name);
    let canonical = if card_name.is_empty() || card_name == "this artifact" {
        oracle.to_string()
    } else {
        oracle.replace(&card_name, "this artifact")
    };
    let mut clauses = canonical
        .split('.')
        .map(str::trim)
        .filter(|clause| !clause.is_empty())
        .collect::<Vec<_>>();
    clauses.sort_unstable();
    let mut expected = vec![
        "this artifact doesn't untap during your untap step",
        "at the beginning of your upkeep, you may pay {4}",
        "if you do, untap this artifact",
        "at the beginning of your draw step, if this artifact is tapped, it deals 1 damage to you",
        "{t}: add {c}{c}{c}",
    ];
    expected.sort_unstable();
    clauses == expected
}

fn has_nonrefreshing_mana_lifecycle(oracle: &str) -> bool {
    oracle.contains("doesn't untap during your untap step")
        || oracle.contains("doesn't untap during its controller's untap step")
        || oracle.contains("doesn't untap during their controller's untap step")
        || oracle.contains("at the beginning of your upkeep") && oracle.contains("untap")
        || oracle.contains("at the beginning of your draw step")
            && oracle.contains("tapped")
            && oracle.contains("damage")
}

fn spell_mana_resolution_is_supported(oracle: &str, magnitude: EffectMagnitude) -> bool {
    if magnitude == EffectMagnitude::Dynamic
        || contains_conditional_language(oracle)
        || oracle.contains("instead")
        || oracle.contains("as an additional cost to cast")
        || oracle.contains("spend this mana only")
        || oracle.contains("spend that mana only")
    {
        return false;
    }

    let before_add = oracle
        .split_once("add ")
        .map(|(before, _)| before)
        .unwrap_or(oracle);
    ![
        "sacrifice ",
        "discard ",
        "exile ",
        "remove ",
        "pay ",
        "reveal ",
    ]
    .iter()
    .any(|cost| before_add.contains(cost))
}

fn classify_mana_activation_cost(
    card: &CardDefinition,
    activation_cost: &str,
) -> Option<ManaProductionKind> {
    let card_name = normalize_rules_name(&card.name);
    let mut requires_tap = false;
    let mut sacrifices_self = false;

    for raw_term in activation_cost.split(',') {
        let term = raw_term.trim();
        if term == "{t}" {
            requires_tap = true;
            continue;
        }
        if term == format!("sacrifice {card_name}")
            || matches!(
                term,
                "sacrifice this artifact"
                    | "sacrifice this creature"
                    | "sacrifice this permanent"
                    | "sacrifice this token"
            )
        {
            sacrifices_self = true;
            continue;
        }
        return None;
    }

    if sacrifices_self {
        let is_creature = card.type_line.to_ascii_lowercase().contains("creature");
        if requires_tap && is_creature {
            // The source would normally have summoning sickness when the
            // simulator casts it. A future activation needs staged object
            // state, so it receives no trajectory credit yet.
            None
        } else {
            Some(ManaProductionKind::OneShotActivated)
        }
    } else if requires_tap {
        Some(ManaProductionKind::ReusableActivated)
    } else {
        None
    }
}

fn normalize_rules_name(name: &str) -> String {
    name.replace('’', "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn parse_add_mana(oracle: &str) -> EffectMagnitude {
    if oracle.contains("add an amount of")
        || oracle.contains("add x mana")
        || oracle.contains("for each")
            && oracle
                .split('.')
                .any(|clause| clause.contains("add ") && clause.contains("for each"))
    {
        return EffectMagnitude::Dynamic;
    }
    let mut total = EffectMagnitude::None;
    for captures in ADD_MANA.captures_iter(oracle) {
        let output = captures
            .get(1)
            .map(|value| value.as_str())
            .unwrap_or_default();
        let symbol_count = MANA_SYMBOL.find_iter(output).count() as u8;
        let magnitude = if symbol_count > 0 {
            EffectMagnitude::Fixed(symbol_count.min(12))
        } else if output.starts_with("one ") || output.starts_with("a ") {
            EffectMagnitude::Fixed(1)
        } else if output.starts_with("two ") {
            EffectMagnitude::Fixed(2)
        } else if output.starts_with("three ") {
            EffectMagnitude::Fixed(3)
        } else {
            EffectMagnitude::Dynamic
        };
        total = total.merge(magnitude);
    }
    total
}

fn parse_magnitude(raw: Option<&str>) -> EffectMagnitude {
    match raw.unwrap_or_default().trim() {
        "a" | "an" | "one" => EffectMagnitude::Fixed(1),
        "two" => EffectMagnitude::Fixed(2),
        "three" => EffectMagnitude::Fixed(3),
        "four" => EffectMagnitude::Fixed(4),
        "five" => EffectMagnitude::Fixed(5),
        "six" => EffectMagnitude::Fixed(6),
        "x" | "that many" | "a number of" | "any number of" => EffectMagnitude::Dynamic,
        value => value
            .parse::<u8>()
            .ok()
            .map(|value| EffectMagnitude::Fixed(value.min(12)))
            .unwrap_or(EffectMagnitude::Dynamic),
    }
}

fn unsupported_clauses(oracle: &str) -> Vec<String> {
    const COMPLEX_SIGNALS: [(&str, &str); 21] = [
        ("copy target", "Spell or ability copying"),
        ("copy that spell", "Spell copying"),
        ("instead", "Replacement effect"),
        ("choose one or more", "Multi-mode choice"),
        ("vote", "Voting"),
        ("venture into the dungeon", "Dungeon progression"),
        ("take the initiative", "Initiative progression"),
        ("daybound", "Day/night state"),
        ("mutate", "Mutate pile"),
        ("suspend", "Suspend timing"),
        ("cascade", "Cascade resolution"),
        ("discover ", "Discover resolution"),
        ("you win the game", "Alternate-win condition"),
        ("loses the game", "Player-loss condition"),
        ("opponent loses ", "Opponent life-total pressure"),
        ("opponents lose ", "Opponent life-total pressure"),
        ("player loses ", "Player life-total pressure"),
        ("players lose ", "Player life-total pressure"),
        ("damage to each opponent", "Opponent life-total pressure"),
        ("damage to target opponent", "Opponent life-total pressure"),
        (" mill", "Library depletion"),
    ];
    let mut clauses = Vec::new();
    for (signal, label) in COMPLEX_SIGNALS {
        if oracle.contains(signal) && !clauses.iter().any(|existing| existing == label) {
            clauses.push(label.to_string());
        }
    }
    clauses
}

fn normalize_oracle(text: &str) -> String {
    text.replace('’', "'")
        .replace(['\u{2013}', '\u{2014}'], "-")
        .replace(['\r', '\n'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn contains_mass_battlefield_exile(oracle: &str) -> bool {
    const BATTLEFIELD_OBJECT_WORDS: [&str; 16] = [
        "artifact",
        "artifacts",
        "battle",
        "battles",
        "creature",
        "creatures",
        "enchantment",
        "enchantments",
        "land",
        "lands",
        "permanent",
        "permanents",
        "planeswalker",
        "planeswalkers",
        "token",
        "tokens",
    ];

    oracle.match_indices("exile all").any(|(offset, _)| {
        let clause = oracle[offset..]
            .split(['.', ',', ';', '\n'])
            .next()
            .unwrap_or_default();
        let words = clause
            .split(|character: char| !character.is_ascii_alphabetic())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        !words.contains(&"card")
            && !words.contains(&"cards")
            && BATTLEFIELD_OBJECT_WORDS
                .iter()
                .any(|object| words.contains(object))
    })
}

fn contains_conditional_language(oracle: &str) -> bool {
    oracle.contains(" if ")
        || oracle.contains(" unless ")
        || oracle.contains(" only if ")
        || oracle.contains(" for each ")
        || oracle.contains(" equal to ")
        || oracle.contains(" where x ")
        || oracle.contains(" whenever ")
}

fn is_mass_land_denial(oracle: &str) -> bool {
    MASS_LAND_RESET.is_match(oracle)
        || MASS_LAND_SACRIFICE.is_match(oracle)
        || oracle.contains("each player chooses a land") && oracle.contains("sacrifices the rest")
        || oracle.contains("lands don't untap during their controllers' untap steps")
        || oracle.contains("players can't untap more than one land during their untap steps")
        || oracle.contains("players can't untap more than two permanents during their untap steps")
        || NONBASIC_LAND_REWRITE.is_match(oracle)
        || oracle.contains("if a land is tapped for mana, it produces")
            && oracle.contains("instead of any other type")
}

static DRAW_CARDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\bdraw (a|an|one|two|three|four|five|six|[0-9]+|x|that many) cards?\b")
        .expect("draw regex")
});
static LOOK_AT_TOP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\blook at the top (one|two|three|four|five|six|[0-9]+|x) cards?\b")
        .expect("selection regex")
});
static CREATE_TOKENS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\bcreate (a|an|one|two|three|four|five|six|[0-9]+|x|that many) ([^.]{0,100}?tokens?)\b",
    )
    .expect("token regex")
});
static ADD_MANA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\badd ([^.;\n]{1,80})").expect("mana regex"));
static ADD_MANA_ABILITY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|[.;])\s*([^.;]{1,180}?):\s*add\b").expect("mana activation regex")
});
static MANA_SYMBOL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\{(?:w|u|b|r|g|c|[0-9]+)\}").expect("mana symbol regex"));
static LIFE_COST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:pay|lose) (?:[0-9]+|x|that much) life").expect("life regex"));
static TUTOR_RESOLUTION_LIFE_LOSS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|\. )you lose ([0-9]+) life\.$").expect("tutor life-loss regex")
});
static MASS_LAND_RESET: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:destroy|exile|return) all [^.]{0,100}\blands?\b|\b(?:destroy|exile|return) [^.]{0,100}\ball lands?\b",
    )
    .expect("mass land reset regex")
});
static MASS_LAND_SACRIFICE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\beach player sacrifices (?:all|x|four|five|six|seven|eight|nine|ten|[4-9]|[1-9][0-9]+) lands?\b",
    )
    .expect("mass land sacrifice regex")
});
static NONBASIC_LAND_REWRITE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\bnonbasic lands (?:are|become) (?:plains|islands|swamps|mountains|forests)(?:\.| |$)",
    )
    .expect("nonbasic land rewrite regex")
});
