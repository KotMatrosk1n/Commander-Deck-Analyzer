//! Exact Oracle envelopes for casting, zone permissions, costs, and delayed instructions.
//!
//! A program is produced only when the complete normalized Oracle source is
//! consumed by typed grammar. There is no opaque text node and no partial
//! recognition fallback. The runtime stages the complete cost, cast, and
//! delayed lifecycle in a cloned state and commits only after all required
//! evidence has been validated.
//!
//! Semantic identity is derived from the exact Oracle content, the relevant
//! source context, the typed program, and versioned compiler, runtime, and
//! rules contracts. Card names, Oracle IDs, database rows, clause addresses,
//! snapshot hashes, timestamps, and source order are never identity inputs.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use regex::Regex;
use sha2::{Digest, Sha256};

pub const ORACLE_CAST_ZONE_ENVELOPE_COMPILER_VERSION: &str =
    "oracle-cast-zone-envelope-compiler-0.4";
pub const ORACLE_CAST_ZONE_ENVELOPE_RUNTIME_VERSION: &str = "oracle-cast-zone-envelope-runtime-0.4";
pub const ORACLE_CAST_ZONE_ENVELOPE_RULES_CONTEXT_VERSION: &str = "magic-comprehensive-rules-2026-06-19:101.4,104.3,107.3,108.3,117,118.9,400.7,601.2,608.2,609.3,614.1,616.1,701.4,701.6,701.14,701.17,701.19,701.21,701.50";

/// Recognition is not production coverage until the host binds the complete
/// transactional contract in this module.
pub const fn oracle_cast_zone_envelope_production_adapter_connected() -> bool {
    false
}

pub type PlayerId = u16;
pub type ObjectId = u64;
pub type TurnId = u64;
pub type PendingInstructionId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncarnationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Stack,
    Command,
}

impl Zone {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Hand => "hand",
            Self::Battlefield => "battlefield",
            Self::Graveyard => "graveyard",
            Self::Exile => "exile",
            Self::Stack => "stack",
            Self::Command => "command",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardType {
    Artifact,
    Battle,
    Creature,
    Enchantment,
    Instant,
    Kindred,
    Land,
    Planeswalker,
    Sorcery,
}

impl CardType {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Artifact => "artifact",
            Self::Battle => "battle",
            Self::Creature => "creature",
            Self::Enchantment => "enchantment",
            Self::Instant => "instant",
            Self::Kindred => "kindred",
            Self::Land => "land",
            Self::Planeswalker => "planeswalker",
            Self::Sorcery => "sorcery",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastZoneSemanticContext {
    pub card_types: BTreeSet<CardType>,
    pub is_front_face: bool,
}

impl CastZoneSemanticContext {
    pub fn from_type_line(type_line: &str, is_front_face: bool) -> Option<Self> {
        if type_line.trim() != type_line
            || type_line.is_empty()
            || collapse_whitespace(type_line) != type_line
        {
            return None;
        }
        let type_part = type_line
            .split_once(" \u{2014} ")
            .or_else(|| type_line.split_once(" - "))
            .map_or(type_line, |(types, _)| types);
        let mut card_types = BTreeSet::new();
        for token in type_part.split_ascii_whitespace() {
            let card_type = match token {
                "Artifact" => Some(CardType::Artifact),
                "Battle" => Some(CardType::Battle),
                "Creature" => Some(CardType::Creature),
                "Enchantment" => Some(CardType::Enchantment),
                "Instant" => Some(CardType::Instant),
                "Kindred" | "Tribal" => Some(CardType::Kindred),
                "Land" => Some(CardType::Land),
                "Planeswalker" => Some(CardType::Planeswalker),
                "Sorcery" => Some(CardType::Sorcery),
                _ => None,
            };
            if let Some(card_type) = card_type {
                card_types.insert(card_type);
            }
        }
        (!card_types.is_empty()).then_some(Self {
            card_types,
            is_front_face,
        })
    }

    fn stable_id(&self) -> String {
        format!(
            "types={};front={}",
            self.card_types
                .iter()
                .map(|card_type| card_type.stable_id())
                .collect::<Vec<_>>()
                .join(","),
            self.is_front_face
        )
    }

    fn is_land(&self) -> bool {
        self.card_types.contains(&CardType::Land)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaColor {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

impl ManaColor {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::White => "w",
            Self::Blue => "u",
            Self::Black => "b",
            Self::Red => "r",
            Self::Green => "g",
            Self::Colorless => "c",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ManaSymbol {
    Generic(u32),
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
    Snow,
    Hybrid(ManaColor, ManaColor),
    Phyrexian(ManaColor),
    VariableX,
}

impl ManaSymbol {
    fn stable_id(self) -> String {
        match self {
            Self::Generic(value) => format!("generic/{value}"),
            Self::White => "w".into(),
            Self::Blue => "u".into(),
            Self::Black => "b".into(),
            Self::Red => "r".into(),
            Self::Green => "g".into(),
            Self::Colorless => "c".into(),
            Self::Snow => "s".into(),
            Self::Hybrid(first, second) => {
                format!("hybrid/{}/{}", first.stable_id(), second.stable_id())
            }
            Self::Phyrexian(color) => format!("phyrexian/{}", color.stable_id()),
            Self::VariableX => "x".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaCost {
    pub exact: String,
    pub symbols: Vec<ManaSymbol>,
}

impl ManaCost {
    fn stable_id(&self) -> String {
        self.symbols
            .iter()
            .copied()
            .map(ManaSymbol::stable_id)
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlayerOperand {
    You,
    ItsController,
    ItsOwner,
    ThatPlayer,
    TargetPlayer,
    TargetOpponent,
    EachPlayer,
}

impl PlayerOperand {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::You => "you",
            Self::ItsController => "its-controller",
            Self::ItsOwner => "its-owner",
            Self::ThatPlayer => "that-player",
            Self::TargetPlayer => "target-player",
            Self::TargetOpponent => "target-opponent",
            Self::EachPlayer => "each-player",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObjectOperand {
    ThisCard,
    ThisSpell,
    It,
    ThatCard,
    ThatSpell,
    TargetSpell,
    TargetInstantOrSorcerySpell,
    TargetSpellYouControl,
    TargetCardInYourGraveyard,
    TopCardOfYourLibrary,
    CardsFromYourGraveyard,
    CardsFromExileYouOwn,
    ExiledCardsWithSource,
}

impl ObjectOperand {
    fn stable_id(&self) -> &'static str {
        match self {
            Self::ThisCard => "this-card",
            Self::ThisSpell => "this-spell",
            Self::It => "it",
            Self::ThatCard => "that-card",
            Self::ThatSpell => "that-spell",
            Self::TargetSpell => "target-spell",
            Self::TargetInstantOrSorcerySpell => "target-instant-or-sorcery-spell",
            Self::TargetSpellYouControl => "target-spell-you-control",
            Self::TargetCardInYourGraveyard => "target-card-in-your-graveyard",
            Self::TopCardOfYourLibrary => "top-card-of-your-library",
            Self::CardsFromYourGraveyard => "cards-from-your-graveyard",
            Self::CardsFromExileYouOwn => "cards-from-exile-you-own",
            Self::ExiledCardsWithSource => "exiled-cards-with-source",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TimingWindow {
    Normal,
    ThisTurn,
    UntilEndOfTurn,
    UntilYourNextTurn,
    DuringYourTurn,
    AsThoughFlash,
    Sorcery,
    LaterTurn,
}

impl TimingWindow {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::ThisTurn => "this-turn",
            Self::UntilEndOfTurn => "until-end-of-turn",
            Self::UntilYourNextTurn => "until-your-next-turn",
            Self::DuringYourTurn => "during-your-turn",
            Self::AsThoughFlash => "as-though-flash",
            Self::Sorcery => "sorcery",
            Self::LaterTurn => "later-turn",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlayKind {
    Cast,
    Play,
    PlayLand,
}

impl PlayKind {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Cast => "cast",
            Self::Play => "play",
            Self::PlayLand => "play-land",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CostAtom {
    Mana(ManaCost),
    PayLife(u32),
    DiscardCard,
    DiscardXCards,
    DiscardLandCard,
    SacrificeCreature,
    SacrificeArtifactOrCreature,
    SacrificePermanent,
    ExileCardFromYourGraveyard,
    ExileCreatureCardFromYourGraveyard,
    ExileCardsFromYourGraveyard(u32),
    RemoveCounter {
        counter: String,
        amount: u32,
        from: ObjectOperand,
    },
    TapUntappedPermanentYouControl,
    RevealCardFromYourHand,
}

impl CostAtom {
    fn stable_id(&self) -> String {
        match self {
            Self::Mana(cost) => format!("mana/{}", cost.stable_id()),
            Self::PayLife(amount) => format!("life/{amount}"),
            Self::DiscardCard => "discard/card".into(),
            Self::DiscardXCards => "discard/x-cards".into(),
            Self::DiscardLandCard => "discard/land".into(),
            Self::SacrificeCreature => "sacrifice/creature".into(),
            Self::SacrificeArtifactOrCreature => "sacrifice/artifact-or-creature".into(),
            Self::SacrificePermanent => "sacrifice/permanent".into(),
            Self::ExileCardFromYourGraveyard => "exile/graveyard-card".into(),
            Self::ExileCreatureCardFromYourGraveyard => "exile/graveyard-creature-card".into(),
            Self::ExileCardsFromYourGraveyard(amount) => {
                format!("exile/graveyard-cards/{amount}")
            }
            Self::RemoveCounter {
                counter,
                amount,
                from,
            } => format!(
                "remove-counter/{}/{}/{}",
                canonical_word(counter),
                amount,
                from.stable_id()
            ),
            Self::TapUntappedPermanentYouControl => "tap/untapped-permanent-you-control".into(),
            Self::RevealCardFromYourHand => "reveal/card-from-your-hand".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostExpression {
    pub atoms: Vec<CostAtom>,
}

impl CostExpression {
    fn stable_id(&self) -> String {
        self.atoms
            .iter()
            .map(CostAtom::stable_id)
            .collect::<Vec<_>>()
            .join("+")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AmountOperand {
    Fixed(u32),
    X,
    ManaValue,
    NumberOfCardsInYourHand,
    NumberOfOpponents,
    NumberOfObjectsMatching,
}

impl AmountOperand {
    fn stable_id(self) -> String {
        match self {
            Self::Fixed(value) => format!("fixed/{value}"),
            Self::X => "x".into(),
            Self::ManaValue => "mana-value".into(),
            Self::NumberOfCardsInYourHand => "cards-in-your-hand".into(),
            Self::NumberOfOpponents => "opponents".into(),
            Self::NumberOfObjectsMatching => "objects-matching".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CostAdjustmentDirection {
    Reduce,
    Increase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CostAdjustmentSubject {
    ThisSpell,
    SpellsYouCast,
    CreatureSpellsYouCast,
    ArtifactSpellsYouCast,
    CommanderSpellsYouCast,
}

impl CostAdjustmentSubject {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::ThisSpell => "this-spell",
            Self::SpellsYouCast => "spells-you-cast",
            Self::CreatureSpellsYouCast => "creature-spells-you-cast",
            Self::ArtifactSpellsYouCast => "artifact-spells-you-cast",
            Self::CommanderSpellsYouCast => "commander-spells-you-cast",
        }
    }
}

impl CostAdjustmentDirection {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::Reduce => "reduce",
            Self::Increase => "increase",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CastCondition {
    YouControlCommander,
    YouControlCreature,
    YouControlArtifact,
    YouControlPermanentOfEachNamedType,
    OpponentLostLifeThisTurn,
    YouCastAnotherSpellThisTurn,
    CardWasDiscardedThisTurn,
    SourceWasCast,
    SourceWasCastFromGraveyard,
    SourceWasCastFromExile,
}

impl CastCondition {
    fn stable_id(&self) -> &'static str {
        match self {
            Self::YouControlCommander => "you-control-commander",
            Self::YouControlCreature => "you-control-creature",
            Self::YouControlArtifact => "you-control-artifact",
            Self::YouControlPermanentOfEachNamedType => "you-control-required-permanent-types",
            Self::OpponentLostLifeThisTurn => "opponent-lost-life-this-turn",
            Self::YouCastAnotherSpellThisTurn => "you-cast-another-spell-this-turn",
            Self::CardWasDiscardedThisTurn => "card-was-discarded-this-turn",
            Self::SourceWasCast => "source-was-cast",
            Self::SourceWasCastFromGraveyard => "source-was-cast-from-graveyard",
            Self::SourceWasCastFromExile => "source-was-cast-from-exile",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AdjustmentQualifier {
    ForEachOpponent,
    ForEachCardInYourHand,
    ForEachCardInYourGraveyard,
    ForEachCreatureYouControl,
    ForEachArtifactYouControl,
    ForEachColoredManaSymbolInItsManaCost,
    ForEachPreviousCommanderCast,
}

impl AdjustmentQualifier {
    fn stable_id(&self) -> &'static str {
        match self {
            Self::ForEachOpponent => "for-each-opponent",
            Self::ForEachCardInYourHand => "for-each-card-in-your-hand",
            Self::ForEachCardInYourGraveyard => "for-each-card-in-your-graveyard",
            Self::ForEachCreatureYouControl => "for-each-creature-you-control",
            Self::ForEachArtifactYouControl => "for-each-artifact-you-control",
            Self::ForEachColoredManaSymbolInItsManaCost => "for-each-colored-symbol-in-mana-cost",
            Self::ForEachPreviousCommanderCast => "for-each-previous-commander-cast",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostAdjustment {
    pub subject: CostAdjustmentSubject,
    pub direction: CostAdjustmentDirection,
    pub amount: AmountOperand,
    pub minimum_one: bool,
    pub qualifier: Option<AdjustmentQualifier>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DelayedMoment {
    BeginningOfNextEndStep,
    BeginningOfYourNextEndStep,
    BeginningOfNextUpkeep,
    BeginningOfYourNextUpkeep,
    EndOfTurn,
    EndOfCombat,
    ThatTurnsEndStep,
}

impl DelayedMoment {
    const fn stable_id(self) -> &'static str {
        match self {
            Self::BeginningOfNextEndStep => "beginning-next-end-step",
            Self::BeginningOfYourNextEndStep => "beginning-your-next-end-step",
            Self::BeginningOfNextUpkeep => "beginning-next-upkeep",
            Self::BeginningOfYourNextUpkeep => "beginning-your-next-upkeep",
            Self::EndOfTurn => "end-of-turn",
            Self::EndOfCombat => "end-of-combat",
            Self::ThatTurnsEndStep => "that-turns-end-step",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelayedAction {
    Exile(ObjectOperand),
    ReturnToHand(ObjectOperand),
    ReturnToBattlefield(ObjectOperand),
    Sacrifice(ObjectOperand),
    LoseGame(PlayerOperand),
}

impl DelayedAction {
    fn stable_id(&self) -> String {
        match self {
            Self::Exile(object) => format!("exile/{}", object.stable_id()),
            Self::ReturnToHand(object) => format!("return-hand/{}", object.stable_id()),
            Self::ReturnToBattlefield(object) => {
                format!("return-battlefield/{}", object.stable_id())
            }
            Self::Sacrifice(object) => format!("sacrifice/{}", object.stable_id()),
            Self::LoseGame(player) => format!("lose-game/{}", player.stable_id()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedInstruction {
    pub moment: DelayedMoment,
    pub action: DelayedAction,
    pub only_if_cast: bool,
    pub expected_incarnation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastPermission {
    pub actor: PlayerOperand,
    pub kind: PlayKind,
    pub object: ObjectOperand,
    pub from_zones: BTreeSet<Zone>,
    pub timing: TimingWindow,
    pub without_paying_mana_cost: bool,
    pub alternative_cost: Option<CostExpression>,
    pub additional_cost: Option<CostExpression>,
    pub other_costs_retained: bool,
    pub delayed: Vec<DelayedInstruction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdditionalCostEnvelope {
    pub optional: bool,
    pub cost: CostExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternativeCostEnvelope {
    pub condition: Option<CastCondition>,
    pub cost: Option<CostExpression>,
    pub without_paying_mana_cost: bool,
    pub other_costs_retained: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastRestrictionEnvelope {
    pub during_declare_attackers_step: bool,
    pub caster_was_attacked_this_step: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedResolutionAction {
    DrawCard,
    CreateTokenCopy,
    EnterWithCounter { counter: String, amount: u32 },
    ReturnToOwnersHand(ObjectOperand),
    Exile(ObjectOperand),
    Sacrifice(ObjectOperand),
}

impl LinkedResolutionAction {
    fn stable_id(&self) -> String {
        match self {
            Self::DrawCard => "draw-card".into(),
            Self::CreateTokenCopy => "create-token-copy".into(),
            Self::EnterWithCounter { counter, amount } => {
                format!("enter-counter/{}/{amount}", canonical_word(counter))
            }
            Self::ReturnToOwnersHand(object) => {
                format!("return-owner-hand/{}", object.stable_id())
            }
            Self::Exile(object) => format!("exile/{}", object.stable_id()),
            Self::Sacrifice(object) => format!("sacrifice/{}", object.stable_id()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedCostPaidEnvelope {
    pub cost_reference: PaidCostReference,
    pub action: LinkedResolutionAction,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PaidCostReference {
    Kicker,
    AdditionalCost,
    AlternativeCost,
    Mana(ManaCost),
    Life(u32),
}

impl PaidCostReference {
    fn stable_id(&self) -> String {
        match self {
            Self::Kicker => "kicker".into(),
            Self::AdditionalCost => "additional-cost".into(),
            Self::AlternativeCost => "alternative-cost".into(),
            Self::Mana(cost) => format!("mana/{}", cost.stable_id()),
            Self::Life(amount) => format!("life/{amount}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastZoneEnvelopeKind {
    Permission(CastPermission),
    CastRestriction(CastRestrictionEnvelope),
    AdditionalCost(AdditionalCostEnvelope),
    AlternativeCost(AlternativeCostEnvelope),
    CostAdjustment(CostAdjustment),
    CopySpell {
        target: ObjectOperand,
        may_choose_new_targets: bool,
    },
    Delayed(DelayedInstruction),
    ExtraTurnWithDelayedLoss {
        player: PlayerOperand,
        delayed_loss: DelayedInstruction,
    },
    LinkedCostPaid(LinkedCostPaidEnvelope),
}

impl CastZoneEnvelopeKind {
    fn stable_id(&self) -> String {
        match self {
            Self::Permission(permission) => format!(
                "permission/v1;actor={};kind={};object={};zones={};timing={};without-mana={};alternative={};additional={};other-costs={};delayed={}",
                permission.actor.stable_id(),
                permission.kind.stable_id(),
                permission.object.stable_id(),
                permission
                    .from_zones
                    .iter()
                    .map(|zone| zone.stable_id())
                    .collect::<Vec<_>>()
                    .join(","),
                permission.timing.stable_id(),
                permission.without_paying_mana_cost,
                permission
                    .alternative_cost
                    .as_ref()
                    .map_or("none".into(), CostExpression::stable_id),
                permission
                    .additional_cost
                    .as_ref()
                    .map_or("none".into(), CostExpression::stable_id),
                permission.other_costs_retained,
                permission
                    .delayed
                    .iter()
                    .map(delayed_instruction_stable_id)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::CastRestriction(restriction) => format!(
                "cast-restriction/v1;declare-attackers={};caster-attacked-this-step={}",
                restriction.during_declare_attackers_step,
                restriction.caster_was_attacked_this_step
            ),
            Self::AdditionalCost(envelope) => format!(
                "additional-cost/v1;optional={};cost={}",
                envelope.optional,
                envelope.cost.stable_id()
            ),
            Self::AlternativeCost(envelope) => format!(
                "alternative-cost/v1;condition={};cost={};without-mana={};other-costs={}",
                envelope
                    .condition
                    .as_ref()
                    .map_or("none", CastCondition::stable_id),
                envelope
                    .cost
                    .as_ref()
                    .map_or("none".into(), CostExpression::stable_id),
                envelope.without_paying_mana_cost,
                envelope.other_costs_retained
            ),
            Self::CostAdjustment(adjustment) => format!(
                "cost-adjustment/v1;subject={};direction={};amount={};minimum-one={};qualifier={}",
                adjustment.subject.stable_id(),
                adjustment.direction.stable_id(),
                adjustment.amount.stable_id(),
                adjustment.minimum_one,
                adjustment
                    .qualifier
                    .as_ref()
                    .map_or("none", AdjustmentQualifier::stable_id)
            ),
            Self::CopySpell {
                target,
                may_choose_new_targets,
            } => format!(
                "copy-spell/v1;target={};new-targets={may_choose_new_targets}",
                target.stable_id()
            ),
            Self::Delayed(instruction) => {
                format!("delayed/v1;{}", delayed_instruction_stable_id(instruction))
            }
            Self::ExtraTurnWithDelayedLoss {
                player,
                delayed_loss,
            } => format!(
                "extra-turn-loss/v1;player={};{}",
                player.stable_id(),
                delayed_instruction_stable_id(delayed_loss)
            ),
            Self::LinkedCostPaid(envelope) => format!(
                "linked-cost-paid/v1;cost={};action={}",
                envelope.cost_reference.stable_id(),
                envelope.action.stable_id()
            ),
        }
    }
}

fn delayed_instruction_stable_id(instruction: &DelayedInstruction) -> String {
    format!(
        "moment={};action={};only-if-cast={};incarnation={}",
        instruction.moment.stable_id(),
        instruction.action.stable_id(),
        instruction.only_if_cast,
        instruction.expected_incarnation
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastZoneEnvelopeProgram {
    exact_source: String,
    normalized_source: String,
    context: CastZoneSemanticContext,
    kind: CastZoneEnvelopeKind,
    semantic_digest: String,
}

impl CastZoneEnvelopeProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn normalized_source(&self) -> &str {
        &self.normalized_source
    }

    pub fn context(&self) -> &CastZoneSemanticContext {
        &self.context
    }

    pub fn kind(&self) -> &CastZoneEnvelopeKind {
        &self.kind
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub const fn production_adapter_connected(&self) -> bool {
        oracle_cast_zone_envelope_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CastZoneEnvelopeRejection {
    EmptyOrMalformedSource,
    NormalizationMismatch,
    NotCandidate,
    IncompleteEnvelope,
    UnsupportedActor,
    UnsupportedObject,
    UnsupportedZone,
    UnsupportedTiming,
    UnsupportedCost,
    UnsupportedCondition,
    UnsupportedDelayedInstruction,
    UnsupportedLinkedAction,
    AmbiguousGrammar,
    UnconsumedSource,
}

impl fmt::Display for CastZoneEnvelopeRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CastZoneEnvelopeRejection {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastZoneEnvelopeClassification {
    Program(CastZoneEnvelopeProgram),
    Rejected(CastZoneEnvelopeRejection),
}

pub fn reviewed_cast_zone_normalized_source(exact_source: &str) -> String {
    collapse_whitespace(exact_source)
}

pub fn compile_cast_zone_envelope_program(
    exact_source: &str,
    normalized_source: &str,
    context: CastZoneSemanticContext,
) -> Result<CastZoneEnvelopeProgram, CastZoneEnvelopeRejection> {
    match classify_cast_zone_envelope(exact_source, normalized_source, context) {
        CastZoneEnvelopeClassification::Program(program) => Ok(program),
        CastZoneEnvelopeClassification::Rejected(reason) => Err(reason),
    }
}

pub fn classify_cast_zone_envelope(
    exact_source: &str,
    normalized_source: &str,
    context: CastZoneSemanticContext,
) -> CastZoneEnvelopeClassification {
    if !is_complete_source(exact_source) || !is_complete_source(normalized_source) {
        return CastZoneEnvelopeClassification::Rejected(
            CastZoneEnvelopeRejection::EmptyOrMalformedSource,
        );
    }
    if reviewed_cast_zone_normalized_source(exact_source) != normalized_source {
        return CastZoneEnvelopeClassification::Rejected(
            CastZoneEnvelopeRejection::NormalizationMismatch,
        );
    }
    if !is_candidate(normalized_source) {
        return CastZoneEnvelopeClassification::Rejected(CastZoneEnvelopeRejection::NotCandidate);
    }

    let parsers: [fn(
        &str,
        &CastZoneSemanticContext,
    ) -> Result<Option<CastZoneEnvelopeKind>, CastZoneEnvelopeRejection>; 9] = [
        parse_cast_restriction,
        parse_permission,
        parse_additional_cost,
        parse_alternative_cost,
        parse_cost_adjustment,
        parse_spell_copy,
        parse_delayed,
        parse_extra_turn,
        parse_linked_cost_paid,
    ];
    let mut accepted = Vec::new();
    let mut strongest_rejection = CastZoneEnvelopeRejection::UnconsumedSource;
    for parser in parsers {
        match parser(normalized_source, &context) {
            Ok(Some(kind)) => accepted.push(kind),
            Ok(None) => {}
            Err(reason) => strongest_rejection = reason,
        }
    }
    if accepted.len() != 1 {
        return CastZoneEnvelopeClassification::Rejected(if accepted.len() > 1 {
            CastZoneEnvelopeRejection::AmbiguousGrammar
        } else {
            strongest_rejection
        });
    }
    let kind = accepted.pop().expect("one accepted grammar");
    let semantic_digest =
        cast_zone_semantic_digest(exact_source, normalized_source, &context, &kind);
    CastZoneEnvelopeClassification::Program(CastZoneEnvelopeProgram {
        exact_source: exact_source.to_owned(),
        normalized_source: normalized_source.to_owned(),
        context,
        kind,
        semantic_digest,
    })
}

fn is_candidate(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    [
        "additional cost to cast",
        "cast this spell only during",
        "rather than pay",
        "without paying",
        "cast this card from",
        "cast it from",
        "play this card from",
        "play the top card",
        "cast the top card",
        "costs {",
        "costs x",
        "spells you cast cost {",
        "spells you cast cost x",
        "copy target",
        "copy it",
        "at the beginning of the next end step",
        "at the beginning of your next end step",
        "at the beginning of the next upkeep",
        "at the beginning of your next upkeep",
        "at end of combat",
        "take an extra turn",
        "takes an extra turn",
        "was paid",
        "were paid",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn parse_cast_restriction(
    source: &str,
    _context: &CastZoneSemanticContext,
) -> Result<Option<CastZoneEnvelopeKind>, CastZoneEnvelopeRejection> {
    if source
        == "Cast this spell only during the declare attackers step and only if you've been attacked this step."
    {
        return Ok(Some(CastZoneEnvelopeKind::CastRestriction(
            CastRestrictionEnvelope {
                during_declare_attackers_step: true,
                caster_was_attacked_this_step: true,
            },
        )));
    }
    Ok(None)
}

fn parse_permission(
    source: &str,
    context: &CastZoneSemanticContext,
) -> Result<Option<CastZoneEnvelopeKind>, CastZoneEnvelopeRejection> {
    let mut core = strip_terminal_period(source)?;
    let mut delayed = Vec::new();
    if let Some((permission, consequence)) = core.split_once(". If you do, ") {
        let instruction = parse_delayed_instruction_body(consequence, true)?
            .ok_or(CastZoneEnvelopeRejection::UnsupportedDelayedInstruction)?;
        delayed.push(instruction);
        core = permission;
    } else if let Some((permission, consequence)) = core.split_once(", and if you do, ") {
        let instruction = parse_delayed_instruction_body(consequence, true)?
            .ok_or(CastZoneEnvelopeRejection::UnsupportedDelayedInstruction)?;
        delayed.push(instruction);
        core = permission;
    }

    let (prefix_timing, core) = if let Some(rest) = core.strip_prefix("Until end of turn, ") {
        (Some(TimingWindow::UntilEndOfTurn), rest)
    } else if let Some(rest) = core.strip_prefix("This turn, ") {
        (Some(TimingWindow::ThisTurn), rest)
    } else if let Some(rest) = core.strip_prefix("During your turn, ") {
        (Some(TimingWindow::DuringYourTurn), rest)
    } else {
        (None, core)
    };

    if let Some(rest) = core.strip_prefix("You may play the top card of your library") {
        let timing = parse_permission_timing_suffix(rest, prefix_timing)?;
        return Ok(Some(CastZoneEnvelopeKind::Permission(CastPermission {
            actor: PlayerOperand::You,
            kind: PlayKind::Play,
            object: ObjectOperand::TopCardOfYourLibrary,
            from_zones: BTreeSet::from([Zone::Library]),
            timing,
            without_paying_mana_cost: false,
            alternative_cost: None,
            additional_cost: None,
            other_costs_retained: true,
            delayed,
        })));
    }
    if let Some(rest) = core.strip_prefix("You may cast the top card of your library") {
        let timing = parse_permission_timing_suffix(rest, prefix_timing)?;
        return Ok(Some(CastZoneEnvelopeKind::Permission(CastPermission {
            actor: PlayerOperand::You,
            kind: PlayKind::Cast,
            object: ObjectOperand::TopCardOfYourLibrary,
            from_zones: BTreeSet::from([Zone::Library]),
            timing,
            without_paying_mana_cost: false,
            alternative_cost: None,
            additional_cost: None,
            other_costs_retained: true,
            delayed,
        })));
    }

    let Some(captures) = static_regex(
        r"^You may (cast|play) (this card|it|that card|the top card of your library|cards|cards you own) from (your graveyard|exile|your command zone|the top of your library)(.*)$",
    )
    .captures(core)
    else {
        return Ok(None);
    };
    let verb = captures.get(1).expect("verb").as_str();
    let object_text = captures.get(2).expect("object").as_str();
    let zone_text = captures.get(3).expect("zone").as_str();
    let mut suffix = captures.get(4).expect("suffix").as_str();

    let from_zone = match zone_text {
        "your graveyard" => Zone::Graveyard,
        "exile" => Zone::Exile,
        "your command zone" => Zone::Command,
        "the top of your library" => Zone::Library,
        _ => return Err(CastZoneEnvelopeRejection::UnsupportedZone),
    };
    let object = match object_text {
        "this card" => ObjectOperand::ThisCard,
        "it" => ObjectOperand::It,
        "that card" => ObjectOperand::ThatCard,
        "the top card of your library" => ObjectOperand::TopCardOfYourLibrary,
        "cards" if from_zone == Zone::Graveyard => ObjectOperand::CardsFromYourGraveyard,
        "cards" | "cards you own" if from_zone == Zone::Exile => {
            ObjectOperand::CardsFromExileYouOwn
        }
        _ => return Err(CastZoneEnvelopeRejection::UnsupportedObject),
    };
    let kind = match verb {
        "cast" => PlayKind::Cast,
        "play" => PlayKind::Play,
        _ => return Err(CastZoneEnvelopeRejection::UnconsumedSource),
    };
    if kind == PlayKind::Cast && context.is_land() && object == ObjectOperand::ThisCard {
        return Err(CastZoneEnvelopeRejection::UnsupportedObject);
    }

    let mut without_paying_mana_cost = false;
    let mut alternative_cost = None;
    let mut additional_cost = None;
    let mut other_costs_retained = true;
    let mut suffix_timing = None;

    if let Some(rest) = suffix.strip_prefix(" by paying ") {
        let (cost_source, remainder) = rest
            .split_once(" rather than paying its mana cost")
            .ok_or(CastZoneEnvelopeRejection::UnsupportedCost)?;
        alternative_cost = Some(parse_cost_expression(cost_source)?);
        suffix = remainder;
    } else if let Some(rest) = suffix.strip_prefix(" by ") {
        let (cost_source, remainder) = rest
            .split_once(" in addition to paying its other costs")
            .ok_or(CastZoneEnvelopeRejection::UnsupportedCost)?;
        additional_cost = Some(parse_cost_expression(cost_source)?);
        suffix = remainder;
    } else if let Some(rest) = suffix.strip_prefix(" without paying its mana cost") {
        without_paying_mana_cost = true;
        suffix = rest;
    } else if let Some(rest) = suffix.strip_prefix(" without paying that card's mana cost") {
        without_paying_mana_cost = true;
        suffix = rest;
    }

    if let Some(rest) = suffix.strip_prefix(" this turn") {
        suffix_timing = Some(TimingWindow::ThisTurn);
        suffix = rest;
    } else if let Some(rest) = suffix.strip_prefix(" until end of turn") {
        suffix_timing = Some(TimingWindow::UntilEndOfTurn);
        suffix = rest;
    } else if let Some(rest) = suffix.strip_prefix(" during your turn") {
        suffix_timing = Some(TimingWindow::DuringYourTurn);
        suffix = rest;
    } else if let Some(rest) = suffix.strip_prefix(" on a later turn") {
        suffix_timing = Some(TimingWindow::LaterTurn);
        suffix = rest;
    } else if let Some(rest) = suffix.strip_prefix(" as though it had flash") {
        suffix_timing = Some(TimingWindow::AsThoughFlash);
        suffix = rest;
    }

    if let Some(rest) = suffix.strip_prefix(". You still pay its additional costs") {
        other_costs_retained = true;
        suffix = rest;
    }
    if !suffix.is_empty() {
        return Err(CastZoneEnvelopeRejection::UnconsumedSource);
    }
    if prefix_timing.is_some() && suffix_timing.is_some() {
        return Err(CastZoneEnvelopeRejection::AmbiguousGrammar);
    }

    Ok(Some(CastZoneEnvelopeKind::Permission(CastPermission {
        actor: PlayerOperand::You,
        kind,
        object,
        from_zones: BTreeSet::from([from_zone]),
        timing: prefix_timing
            .or(suffix_timing)
            .unwrap_or(TimingWindow::Normal),
        without_paying_mana_cost,
        alternative_cost,
        additional_cost,
        other_costs_retained,
        delayed,
    })))
}

fn parse_permission_timing_suffix(
    suffix: &str,
    prefix: Option<TimingWindow>,
) -> Result<TimingWindow, CastZoneEnvelopeRejection> {
    let suffix_timing = match suffix {
        "" => None,
        " this turn" => Some(TimingWindow::ThisTurn),
        " until end of turn" => Some(TimingWindow::UntilEndOfTurn),
        " during your turn" => Some(TimingWindow::DuringYourTurn),
        " as though it had flash" => Some(TimingWindow::AsThoughFlash),
        _ => return Err(CastZoneEnvelopeRejection::UnsupportedTiming),
    };
    if prefix.is_some() && suffix_timing.is_some() {
        return Err(CastZoneEnvelopeRejection::AmbiguousGrammar);
    }
    Ok(prefix.or(suffix_timing).unwrap_or(TimingWindow::Normal))
}

fn parse_additional_cost(
    source: &str,
    _context: &CastZoneSemanticContext,
) -> Result<Option<CastZoneEnvelopeKind>, CastZoneEnvelopeRejection> {
    let core = strip_terminal_period(source)?;
    let body = if let Some(body) = core.strip_prefix("As an additional cost to cast this spell, ") {
        body
    } else if let Some(body) = core.strip_prefix("As an additional cost to cast this card, ") {
        body
    } else if let Some(body) = core.strip_prefix("To cast this spell, ") {
        body
    } else {
        return Ok(None);
    };
    let (optional, body) = if let Some(body) = body.strip_prefix("you may ") {
        (true, body)
    } else {
        (false, body)
    };
    let cost = parse_cost_expression(body)?;
    Ok(Some(CastZoneEnvelopeKind::AdditionalCost(
        AdditionalCostEnvelope { optional, cost },
    )))
}

fn parse_alternative_cost(
    source: &str,
    _context: &CastZoneSemanticContext,
) -> Result<Option<CastZoneEnvelopeKind>, CastZoneEnvelopeRejection> {
    let core = strip_terminal_period(source)?;

    if let Some(captures) =
        static_regex(r"^You may (.+) rather than pay (?:this spell's|its) mana cost(?: if (.+))?$")
            .captures(core)
    {
        let cost = parse_cost_expression(captures.get(1).expect("cost").as_str())?;
        let condition = captures
            .get(2)
            .map(|capture| parse_cast_condition(capture.as_str()))
            .transpose()?;
        return Ok(Some(CastZoneEnvelopeKind::AlternativeCost(
            AlternativeCostEnvelope {
                condition,
                cost: Some(cost),
                without_paying_mana_cost: false,
                other_costs_retained: true,
            },
        )));
    }

    if let Some(captures) =
        static_regex(r"^You may cast this spell without paying its mana cost(?: if (.+))?$")
            .captures(core)
    {
        let condition = captures
            .get(1)
            .map(|capture| parse_cast_condition(capture.as_str()))
            .transpose()?;
        return Ok(Some(CastZoneEnvelopeKind::AlternativeCost(
            AlternativeCostEnvelope {
                condition,
                cost: None,
                without_paying_mana_cost: true,
                other_costs_retained: true,
            },
        )));
    }

    if let Some(captures) =
        static_regex(r"^If (.+), you may cast this spell without paying its mana cost$")
            .captures(core)
    {
        let condition = parse_cast_condition(captures.get(1).expect("condition").as_str())?;
        return Ok(Some(CastZoneEnvelopeKind::AlternativeCost(
            AlternativeCostEnvelope {
                condition: Some(condition),
                cost: None,
                without_paying_mana_cost: true,
                other_costs_retained: true,
            },
        )));
    }
    Ok(None)
}

fn parse_cost_adjustment(
    source: &str,
    _context: &CastZoneSemanticContext,
) -> Result<Option<CastZoneEnvelopeKind>, CastZoneEnvelopeRejection> {
    let mut core = strip_terminal_period(source)?;
    let mut minimum_one = false;
    for suffix in [
        ". This effect can't reduce the mana in that cost to less than one mana",
        ". This effect can't reduce the amount of mana an ability costs to activate to less than one mana",
    ] {
        if let Some(prefix) = core.strip_suffix(suffix) {
            minimum_one = true;
            core = prefix;
            break;
        }
    }

    let Some(captures) = static_regex(
        r"^(This spell|Spells you cast|Creature spells you cast|Artifact spells you cast|Commander spells you cast) costs? (\{[0-9XWUBRGCS/]+\}|X) (less|more) to cast(.*)$",
    )
    .captures(core)
    else {
        return Ok(None);
    };
    let subject = match captures.get(1).expect("subject").as_str() {
        "This spell" => CostAdjustmentSubject::ThisSpell,
        "Spells you cast" => CostAdjustmentSubject::SpellsYouCast,
        "Creature spells you cast" => CostAdjustmentSubject::CreatureSpellsYouCast,
        "Artifact spells you cast" => CostAdjustmentSubject::ArtifactSpellsYouCast,
        "Commander spells you cast" => CostAdjustmentSubject::CommanderSpellsYouCast,
        _ => return Err(CastZoneEnvelopeRejection::UnconsumedSource),
    };
    let amount = parse_adjustment_amount(captures.get(2).expect("amount").as_str())?;
    let direction = match captures.get(3).expect("direction").as_str() {
        "less" => CostAdjustmentDirection::Reduce,
        "more" => CostAdjustmentDirection::Increase,
        _ => return Err(CastZoneEnvelopeRejection::UnconsumedSource),
    };
    let qualifier = parse_adjustment_qualifier(captures.get(4).expect("qualifier").as_str())?;
    Ok(Some(CastZoneEnvelopeKind::CostAdjustment(CostAdjustment {
        subject,
        direction,
        amount,
        minimum_one,
        qualifier,
    })))
}

fn parse_spell_copy(
    source: &str,
    _context: &CastZoneSemanticContext,
) -> Result<Option<CastZoneEnvelopeKind>, CastZoneEnvelopeRejection> {
    let core = strip_terminal_period(source)?;
    let (copy_clause, may_choose_new_targets) =
        if let Some(copy_clause) = core.strip_suffix(". You may choose new targets for the copy") {
            (copy_clause, true)
        } else {
            (core, false)
        };
    let target = match copy_clause {
        "Copy target spell" => ObjectOperand::TargetSpell,
        "Copy target instant or sorcery spell" => ObjectOperand::TargetInstantOrSorcerySpell,
        "Copy target instant or sorcery spell you control" => ObjectOperand::TargetSpellYouControl,
        _ => return Ok(None),
    };
    Ok(Some(CastZoneEnvelopeKind::CopySpell {
        target,
        may_choose_new_targets,
    }))
}

fn parse_delayed(
    source: &str,
    _context: &CastZoneSemanticContext,
) -> Result<Option<CastZoneEnvelopeKind>, CastZoneEnvelopeRejection> {
    let core = strip_terminal_period(source)?;
    Ok(parse_delayed_instruction_body(core, false)?.map(CastZoneEnvelopeKind::Delayed))
}

fn parse_extra_turn(
    source: &str,
    _context: &CastZoneSemanticContext,
) -> Result<Option<CastZoneEnvelopeKind>, CastZoneEnvelopeRejection> {
    let core = strip_terminal_period(source)?;
    let Some((turn_clause, loss_clause)) = core.split_once(". ") else {
        return Ok(None);
    };
    let player = match turn_clause {
        "Take an extra turn after this one" => PlayerOperand::You,
        "Target player takes an extra turn after this one" => PlayerOperand::TargetPlayer,
        "Target opponent takes an extra turn after this one" => PlayerOperand::TargetOpponent,
        _ => return Ok(None),
    };
    let expected_loss = match player {
        PlayerOperand::You => "At the beginning of that turn's end step, you lose the game",
        PlayerOperand::TargetPlayer => {
            "At the beginning of that turn's end step, that player loses the game"
        }
        PlayerOperand::TargetOpponent => {
            "At the beginning of that turn's end step, that player loses the game"
        }
        _ => return Err(CastZoneEnvelopeRejection::UnsupportedActor),
    };
    if loss_clause != expected_loss {
        return Err(CastZoneEnvelopeRejection::UnsupportedDelayedInstruction);
    }
    let delayed_loss = DelayedInstruction {
        moment: DelayedMoment::ThatTurnsEndStep,
        action: DelayedAction::LoseGame(match player {
            PlayerOperand::You => PlayerOperand::You,
            _ => PlayerOperand::ThatPlayer,
        }),
        only_if_cast: false,
        expected_incarnation: false,
    };
    Ok(Some(CastZoneEnvelopeKind::ExtraTurnWithDelayedLoss {
        player,
        delayed_loss,
    }))
}

fn parse_linked_cost_paid(
    source: &str,
    _context: &CastZoneSemanticContext,
) -> Result<Option<CastZoneEnvelopeKind>, CastZoneEnvelopeRejection> {
    let core = strip_terminal_period(source)?;
    let Some(captures) = static_regex(r"^If (.+) (?:was|were) paid, (.+)$").captures(core) else {
        return Ok(None);
    };
    let cost_reference = parse_paid_cost_reference(captures.get(1).expect("cost").as_str())?;
    let action = parse_linked_resolution_action(captures.get(2).expect("action").as_str())?;
    Ok(Some(CastZoneEnvelopeKind::LinkedCostPaid(
        LinkedCostPaidEnvelope {
            cost_reference,
            action,
        },
    )))
}

fn parse_delayed_instruction_body(
    source: &str,
    only_if_cast: bool,
) -> Result<Option<DelayedInstruction>, CastZoneEnvelopeRejection> {
    let source = source.strip_suffix('.').unwrap_or(source);
    let moments = [
        (
            "at the beginning of the next end step",
            DelayedMoment::BeginningOfNextEndStep,
        ),
        (
            "at the beginning of your next end step",
            DelayedMoment::BeginningOfYourNextEndStep,
        ),
        (
            "at the beginning of the next upkeep",
            DelayedMoment::BeginningOfNextUpkeep,
        ),
        (
            "at the beginning of your next upkeep",
            DelayedMoment::BeginningOfYourNextUpkeep,
        ),
        ("at end of combat", DelayedMoment::EndOfCombat),
        ("at the end of combat", DelayedMoment::EndOfCombat),
        ("at end of turn", DelayedMoment::EndOfTurn),
    ];

    for (phrase, moment) in moments {
        if let Some(action_source) = source.strip_suffix(&format!(" {phrase}")) {
            let action = parse_delayed_action(action_source)?;
            return Ok(Some(DelayedInstruction {
                moment,
                action,
                only_if_cast,
                expected_incarnation: true,
            }));
        }
        let prefix = sentence_case(phrase);
        if let Some(action_source) = source.strip_prefix(&format!("{prefix}, ")) {
            let action = parse_delayed_action(action_source)?;
            return Ok(Some(DelayedInstruction {
                moment,
                action,
                only_if_cast,
                expected_incarnation: true,
            }));
        }
    }
    Ok(None)
}

fn parse_delayed_action(source: &str) -> Result<DelayedAction, CastZoneEnvelopeRejection> {
    if let Some(object) = source.strip_prefix("exile ") {
        return Ok(DelayedAction::Exile(parse_object_operand(object)?));
    }
    if let Some(object) = source.strip_prefix("Exile ") {
        return Ok(DelayedAction::Exile(parse_object_operand(object)?));
    }
    if let Some(object) = source.strip_prefix("sacrifice ") {
        return Ok(DelayedAction::Sacrifice(parse_object_operand(object)?));
    }
    if let Some(object) = source.strip_prefix("Sacrifice ") {
        return Ok(DelayedAction::Sacrifice(parse_object_operand(object)?));
    }
    if let Some(object) = source.strip_prefix("return ").and_then(|body| {
        body.strip_suffix(" to its owner's hand")
            .or_else(|| body.strip_suffix(" to their owner's hand"))
    }) {
        return Ok(DelayedAction::ReturnToHand(parse_object_operand(object)?));
    }
    if let Some(object) = source.strip_prefix("Return ").and_then(|body| {
        body.strip_suffix(" to its owner's hand")
            .or_else(|| body.strip_suffix(" to their owner's hand"))
    }) {
        return Ok(DelayedAction::ReturnToHand(parse_object_operand(object)?));
    }
    if let Some(object) = source
        .strip_prefix("return ")
        .and_then(|body| body.strip_suffix(" to the battlefield under its owner's control"))
    {
        return Ok(DelayedAction::ReturnToBattlefield(parse_object_operand(
            object,
        )?));
    }
    if let Some(object) = source
        .strip_prefix("Return ")
        .and_then(|body| body.strip_suffix(" to the battlefield under its owner's control"))
    {
        return Ok(DelayedAction::ReturnToBattlefield(parse_object_operand(
            object,
        )?));
    }
    Err(CastZoneEnvelopeRejection::UnsupportedDelayedInstruction)
}

fn parse_object_operand(source: &str) -> Result<ObjectOperand, CastZoneEnvelopeRejection> {
    match source {
        "this card" => Ok(ObjectOperand::ThisCard),
        "this spell" => Ok(ObjectOperand::ThisSpell),
        "it" => Ok(ObjectOperand::It),
        "that card" => Ok(ObjectOperand::ThatCard),
        "that spell" => Ok(ObjectOperand::ThatSpell),
        "target spell" => Ok(ObjectOperand::TargetSpell),
        "target instant or sorcery spell" => Ok(ObjectOperand::TargetInstantOrSorcerySpell),
        "target instant or sorcery spell you control" => Ok(ObjectOperand::TargetSpellYouControl),
        _ => Err(CastZoneEnvelopeRejection::UnsupportedObject),
    }
}

fn parse_cost_expression(source: &str) -> Result<CostExpression, CastZoneEnvelopeRejection> {
    if source.is_empty() || source.trim() != source {
        return Err(CastZoneEnvelopeRejection::UnsupportedCost);
    }
    let normalized = source
        .strip_prefix("pay ")
        .or_else(|| source.strip_prefix("Pay "))
        .unwrap_or(source);
    let parts = split_cost_atoms(normalized);
    if parts.is_empty() {
        return Err(CastZoneEnvelopeRejection::UnsupportedCost);
    }
    let atoms = parts
        .into_iter()
        .map(parse_cost_atom)
        .collect::<Result<Vec<_>, _>>()?;
    if atoms.is_empty() {
        return Err(CastZoneEnvelopeRejection::UnsupportedCost);
    }
    Ok(CostExpression { atoms })
}

fn split_cost_atoms(source: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut brace_depth = 0i32;
    for (index, character) in source.char_indices() {
        match character {
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            _ => {}
        }
        if brace_depth == 0 && source[index..].starts_with(" and ") {
            parts.push(&source[start..index]);
            start = index + " and ".len();
        }
    }
    parts.push(&source[start..]);
    parts
}

fn parse_cost_atom(source: &str) -> Result<CostAtom, CastZoneEnvelopeRejection> {
    let source = source
        .strip_prefix("pay ")
        .or_else(|| source.strip_prefix("Pay "))
        .unwrap_or(source);
    if source.starts_with('{') {
        return parse_mana_cost(source)
            .map(CostAtom::Mana)
            .ok_or(CastZoneEnvelopeRejection::UnsupportedCost);
    }
    if let Some(amount) = source.strip_suffix(" life").and_then(parse_number) {
        return Ok(CostAtom::PayLife(amount));
    }
    match source {
        "discard a card" | "Discard a card" => Ok(CostAtom::DiscardCard),
        "discard X cards" | "Discard X cards" => Ok(CostAtom::DiscardXCards),
        "discard a land card" | "Discard a land card" => Ok(CostAtom::DiscardLandCard),
        "sacrifice a creature" | "Sacrifice a creature" => Ok(CostAtom::SacrificeCreature),
        "sacrifice an artifact or creature" | "Sacrifice an artifact or creature" => {
            Ok(CostAtom::SacrificeArtifactOrCreature)
        }
        "sacrifice a permanent" | "Sacrifice a permanent" => Ok(CostAtom::SacrificePermanent),
        "exile a card from your graveyard" | "Exile a card from your graveyard" => {
            Ok(CostAtom::ExileCardFromYourGraveyard)
        }
        "exile a creature card from your graveyard"
        | "Exile a creature card from your graveyard" => {
            Ok(CostAtom::ExileCreatureCardFromYourGraveyard)
        }
        "tap an untapped permanent you control" | "Tap an untapped permanent you control" => {
            Ok(CostAtom::TapUntappedPermanentYouControl)
        }
        "reveal a card from your hand" | "Reveal a card from your hand" => {
            Ok(CostAtom::RevealCardFromYourHand)
        }
        _ => {
            if let Some(captures) =
                static_regex(r"^[Ee]xile ([0-9]+|one|two|three|four|five|six|seven|eight|nine|ten) (?:other )?cards? from your graveyard$")
                    .captures(source)
            {
                let amount = parse_number(captures.get(1).expect("amount").as_str())
                    .ok_or(CastZoneEnvelopeRejection::UnsupportedCost)?;
                return Ok(if amount == 1 {
                    CostAtom::ExileCardFromYourGraveyard
                } else {
                    CostAtom::ExileCardsFromYourGraveyard(amount)
                });
            }
            if let Some(captures) =
                static_regex(r"^[Rr]emove ([0-9]+|one|two|three|four|five|six|seven|eight|nine|ten) ([A-Za-z0-9+/-]+) counters? from (this card|this spell|it)$")
                    .captures(source)
            {
                let amount = parse_number(captures.get(1).expect("amount").as_str())
                    .ok_or(CastZoneEnvelopeRejection::UnsupportedCost)?;
                let counter = captures.get(2).expect("counter").as_str().to_owned();
                let from = parse_object_operand(captures.get(3).expect("object").as_str())?;
                return Ok(CostAtom::RemoveCounter {
                    counter,
                    amount,
                    from,
                });
            }
            Err(CastZoneEnvelopeRejection::UnsupportedCost)
        }
    }
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    if source.is_empty() || source.trim() != source {
        return None;
    }
    let mut symbols = Vec::new();
    let mut remainder = source;
    while let Some(rest) = remainder.strip_prefix('{') {
        let close = rest.find('}')?;
        let symbol = &rest[..close];
        if symbol.is_empty() {
            return None;
        }
        let parsed = match symbol {
            "W" => ManaSymbol::White,
            "U" => ManaSymbol::Blue,
            "B" => ManaSymbol::Black,
            "R" => ManaSymbol::Red,
            "G" => ManaSymbol::Green,
            "C" => ManaSymbol::Colorless,
            "S" => ManaSymbol::Snow,
            "X" => ManaSymbol::VariableX,
            value if value.chars().all(|character| character.is_ascii_digit()) => {
                ManaSymbol::Generic(value.parse().ok()?)
            }
            value if value.contains('/') => {
                let (first, second) = value.split_once('/')?;
                if second == "P" {
                    ManaSymbol::Phyrexian(parse_mana_color(first)?)
                } else {
                    ManaSymbol::Hybrid(parse_mana_color(first)?, parse_mana_color(second)?)
                }
            }
            _ => return None,
        };
        symbols.push(parsed);
        remainder = &rest[close + 1..];
    }
    if !remainder.is_empty() || symbols.is_empty() {
        return None;
    }
    Some(ManaCost {
        exact: source.to_owned(),
        symbols,
    })
}

fn parse_mana_color(source: &str) -> Option<ManaColor> {
    match source {
        "W" => Some(ManaColor::White),
        "U" => Some(ManaColor::Blue),
        "B" => Some(ManaColor::Black),
        "R" => Some(ManaColor::Red),
        "G" => Some(ManaColor::Green),
        "C" => Some(ManaColor::Colorless),
        _ => None,
    }
}

fn parse_cast_condition(source: &str) -> Result<CastCondition, CastZoneEnvelopeRejection> {
    match source {
        "you control a commander" => Ok(CastCondition::YouControlCommander),
        "you control a creature" => Ok(CastCondition::YouControlCreature),
        "you control an artifact" => Ok(CastCondition::YouControlArtifact),
        "you control an artifact, a creature, an enchantment, and a land" => {
            Ok(CastCondition::YouControlPermanentOfEachNamedType)
        }
        "an opponent lost life this turn" => Ok(CastCondition::OpponentLostLifeThisTurn),
        "you cast another spell this turn" | "you've cast another spell this turn" => {
            Ok(CastCondition::YouCastAnotherSpellThisTurn)
        }
        "a card was discarded this turn" => Ok(CastCondition::CardWasDiscardedThisTurn),
        "it was cast" | "this spell was cast" => Ok(CastCondition::SourceWasCast),
        "it was cast from a graveyard" | "this spell was cast from a graveyard" => {
            Ok(CastCondition::SourceWasCastFromGraveyard)
        }
        "it was cast from exile" | "this spell was cast from exile" => {
            Ok(CastCondition::SourceWasCastFromExile)
        }
        _ => Err(CastZoneEnvelopeRejection::UnsupportedCondition),
    }
}

fn parse_adjustment_amount(source: &str) -> Result<AmountOperand, CastZoneEnvelopeRejection> {
    let cost = parse_mana_cost(source).ok_or(CastZoneEnvelopeRejection::UnsupportedCost)?;
    if cost.symbols.len() != 1 {
        return Err(CastZoneEnvelopeRejection::UnsupportedCost);
    }
    match cost.symbols[0] {
        ManaSymbol::Generic(value) if value > 0 => Ok(AmountOperand::Fixed(value)),
        // A bare X is not self-defining. The complete Oracle clause must also
        // bind X before this compiler may produce an executable adjustment.
        ManaSymbol::VariableX => Err(CastZoneEnvelopeRejection::UnsupportedCost),
        _ => Err(CastZoneEnvelopeRejection::UnsupportedCost),
    }
}

fn parse_adjustment_qualifier(
    source: &str,
) -> Result<Option<AdjustmentQualifier>, CastZoneEnvelopeRejection> {
    match source {
        "" => Ok(None),
        " for each opponent you have" => Ok(Some(AdjustmentQualifier::ForEachOpponent)),
        " for each card in your hand" => Ok(Some(AdjustmentQualifier::ForEachCardInYourHand)),
        " for each card in your graveyard" => {
            Ok(Some(AdjustmentQualifier::ForEachCardInYourGraveyard))
        }
        " for each creature you control" => {
            Ok(Some(AdjustmentQualifier::ForEachCreatureYouControl))
        }
        " for each artifact you control" => {
            Ok(Some(AdjustmentQualifier::ForEachArtifactYouControl))
        }
        " for each colored mana symbol in its mana cost" => Ok(Some(
            AdjustmentQualifier::ForEachColoredManaSymbolInItsManaCost,
        )),
        " for each time you've cast your commander from the command zone this game" => {
            Ok(Some(AdjustmentQualifier::ForEachPreviousCommanderCast))
        }
        _ => Err(CastZoneEnvelopeRejection::UnsupportedCondition),
    }
}

fn parse_paid_cost_reference(source: &str) -> Result<PaidCostReference, CastZoneEnvelopeRejection> {
    match source {
        "the kicker cost" | "its kicker cost" => Ok(PaidCostReference::Kicker),
        "the additional cost" | "that additional cost" => Ok(PaidCostReference::AdditionalCost),
        "the alternative cost" | "that alternative cost" => Ok(PaidCostReference::AlternativeCost),
        value if value.starts_with('{') => parse_mana_cost(value)
            .map(PaidCostReference::Mana)
            .ok_or(CastZoneEnvelopeRejection::UnsupportedCost),
        value if value.ends_with(" life") => value
            .strip_suffix(" life")
            .and_then(parse_number)
            .map(PaidCostReference::Life)
            .ok_or(CastZoneEnvelopeRejection::UnsupportedCost),
        _ => Err(CastZoneEnvelopeRejection::UnsupportedCost),
    }
}

fn parse_linked_resolution_action(
    source: &str,
) -> Result<LinkedResolutionAction, CastZoneEnvelopeRejection> {
    match source {
        "draw a card" | "you draw a card" => Ok(LinkedResolutionAction::DrawCard),
        "create a token that's a copy of it" | "create a token copy of it" => {
            Ok(LinkedResolutionAction::CreateTokenCopy)
        }
        "return it to its owner's hand" => Ok(LinkedResolutionAction::ReturnToOwnersHand(
            ObjectOperand::It,
        )),
        "exile it" => Ok(LinkedResolutionAction::Exile(ObjectOperand::It)),
        "sacrifice it" => Ok(LinkedResolutionAction::Sacrifice(ObjectOperand::It)),
        _ => {
            if let Some(captures) = static_regex(
                r"^it enters the battlefield with ([0-9]+|one|two|three|four|five|six|seven|eight|nine|ten) ([A-Za-z0-9+/-]+) counters? on it$",
            )
            .captures(source)
            {
                let amount = parse_number(captures.get(1).expect("amount").as_str())
                    .ok_or(CastZoneEnvelopeRejection::UnsupportedLinkedAction)?;
                let counter = captures.get(2).expect("counter").as_str().to_owned();
                return Ok(LinkedResolutionAction::EnterWithCounter { counter, amount });
            }
            Err(CastZoneEnvelopeRejection::UnsupportedLinkedAction)
        }
    }
}

fn cast_zone_semantic_digest(
    exact_source: &str,
    normalized_source: &str,
    context: &CastZoneSemanticContext,
    kind: &CastZoneEnvelopeKind,
) -> String {
    let mut hasher = Sha256::new();
    for component in [
        "oracle-cast-zone-envelope-content/v1",
        ORACLE_CAST_ZONE_ENVELOPE_COMPILER_VERSION,
        ORACLE_CAST_ZONE_ENVELOPE_RUNTIME_VERSION,
        ORACLE_CAST_ZONE_ENVELOPE_RULES_CONTEXT_VERSION,
        exact_source,
        normalized_source,
        &context.stable_id(),
        &kind.stable_id(),
    ] {
        hasher.update((component.len() as u64).to_be_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:X}", hasher.finalize())
}

fn strip_terminal_period(source: &str) -> Result<&str, CastZoneEnvelopeRejection> {
    source
        .strip_suffix('.')
        .ok_or(CastZoneEnvelopeRejection::IncompleteEnvelope)
}

fn is_complete_source(source: &str) -> bool {
    !source.is_empty()
        && source.trim() == source
        && !source
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        && balanced_delimiters(source)
}

fn balanced_delimiters(source: &str) -> bool {
    let mut parentheses = 0i32;
    let mut braces = 0i32;
    let mut quoted = false;
    for character in source.chars() {
        match character {
            '(' if !quoted => parentheses += 1,
            ')' if !quoted => {
                parentheses -= 1;
                if parentheses < 0 {
                    return false;
                }
            }
            '{' if !quoted => braces += 1,
            '}' if !quoted => {
                braces -= 1;
                if braces < 0 {
                    return false;
                }
            }
            '"' => quoted = !quoted,
            _ => {}
        }
    }
    parentheses == 0 && braces == 0 && !quoted
}

fn collapse_whitespace(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn canonical_word(source: &str) -> String {
    source
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '/') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn parse_number(source: &str) -> Option<u32> {
    match source {
        "one" | "a" | "an" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        value => value.parse().ok().filter(|value| *value > 0),
    }
}

fn sentence_case(source: &str) -> String {
    let mut characters = source.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    first.to_uppercase().chain(characters).collect()
}

fn static_regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("cast-zone regex is valid")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaUnitId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManaUnit {
    pub id: ManaUnitId,
    pub color: ManaColor,
    pub snow: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub life: i64,
    pub lost_game: bool,
    pub mana_pool: BTreeMap<ManaUnitId, ManaUnit>,
    pub land_plays_remaining: u32,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            life: 40,
            lost_game: false,
            mana_pool: BTreeMap::new(),
            land_plays_remaining: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameObject {
    pub reference: ObjectRef,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub card_types: BTreeSet<CardType>,
    pub printed_mana_cost: Option<ManaCost>,
    pub counters: BTreeMap<String, u32>,
    pub tapped: bool,
    pub is_commander: bool,
    pub is_token: bool,
    pub is_copy: bool,
    pub copied_from: Option<ObjectRef>,
}

impl GameObject {
    fn is_permanent(&self) -> bool {
        self.card_types.iter().any(|card_type| {
            matches!(
                card_type,
                CardType::Artifact
                    | CardType::Battle
                    | CardType::Creature
                    | CardType::Enchantment
                    | CardType::Land
                    | CardType::Planeswalker
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CastId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CastLifecycleStatus {
    OnStack,
    Resolved,
    Countered,
    LeftExpectedZone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaidCostLedger {
    pub base_mana_paid: bool,
    pub without_paying_mana_cost: bool,
    pub alternative_cost_paid: bool,
    pub additional_cost_paid: bool,
    pub paid_mana_costs: Vec<ManaCost>,
    pub life_paid: u32,
    /// Exact nonmana X chosen for a variable additional or alternative cost.
    pub chosen_x: Option<u32>,
    pub paid_atoms: Vec<CostAtom>,
    pub externally_verified_paid_costs: BTreeSet<PaidCostReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastLifecycle {
    pub cast_id: CastId,
    pub program_digest: String,
    pub actor: PlayerId,
    pub original_reference: ObjectRef,
    pub stack_reference: ObjectRef,
    pub resolved_reference: Option<ObjectRef>,
    pub status: CastLifecycleStatus,
    pub paid_costs: PaidCostLedger,
    pub delayed_templates: Vec<DelayedInstruction>,
    pub pending_entry_counters: Vec<(String, u32)>,
    pub cast_turn: TurnId,
    pub origin_zone: Zone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingDelayedInstruction {
    pub id: PendingInstructionId,
    pub program_digest: String,
    pub moment: DelayedMoment,
    pub action: DelayedAction,
    pub controller: PlayerId,
    pub expected_object: Option<ObjectRef>,
    pub affected_player: Option<PlayerId>,
    pub created_turn: TurnId,
    pub extra_turn: Option<TurnId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraTurn {
    pub turn_id: TurnId,
    pub player: PlayerId,
    pub after_turn: TurnId,
    pub source_program_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CostConditionState {
    pub satisfied: BTreeSet<CastCondition>,
    pub qualifier_counts: BTreeMap<AdjustmentQualifier, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveCostAdjustment {
    pub controller: PlayerId,
    pub source: ObjectRef,
    pub program_digest: String,
    pub adjustment: CostAdjustment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastZoneWorldState {
    pub players: BTreeMap<PlayerId, PlayerState>,
    pub objects: BTreeMap<ObjectRef, GameObject>,
    /// Top card is the last element.
    pub libraries: BTreeMap<PlayerId, Vec<ObjectRef>>,
    pub stack: Vec<ObjectRef>,
    pub casts: BTreeMap<CastId, CastLifecycle>,
    pub pending_delayed: BTreeMap<PendingInstructionId, PendingDelayedInstruction>,
    pub extra_turns: Vec<ExtraTurn>,
    pub active_cost_adjustments: Vec<ActiveCostAdjustment>,
    pub applied_linked_cost_actions: BTreeSet<(CastId, String)>,
    pub current_turn: TurnId,
    pub active_player: PlayerId,
    pub next_object_id: ObjectId,
    pub next_cast_id: u64,
    pub next_pending_id: PendingInstructionId,
    pub next_turn_id: TurnId,
    /// The host must explicitly prove these boundaries before execution.
    pub no_applicable_replacement_effects: bool,
    pub timing_evidence_complete: bool,
    pub hidden_zone_evidence_complete: bool,
    pub spell_target_evidence_complete: bool,
}

impl Default for CastZoneWorldState {
    fn default() -> Self {
        Self {
            players: BTreeMap::new(),
            objects: BTreeMap::new(),
            libraries: BTreeMap::new(),
            stack: Vec::new(),
            casts: BTreeMap::new(),
            pending_delayed: BTreeMap::new(),
            extra_turns: Vec::new(),
            active_cost_adjustments: Vec::new(),
            applied_linked_cost_actions: BTreeSet::new(),
            current_turn: 1,
            active_player: 0,
            next_object_id: 1,
            next_cast_id: 1,
            next_pending_id: 1,
            next_turn_id: 2,
            no_applicable_replacement_effects: false,
            timing_evidence_complete: false,
            hidden_zone_evidence_complete: false,
            spell_target_evidence_complete: false,
        }
    }
}

pub trait CastZoneStateAdapter: Clone {
    fn cast_zone_world(&self) -> &CastZoneWorldState;
    fn cast_zone_world_mut(&mut self) -> &mut CastZoneWorldState;
}

impl CastZoneStateAdapter for CastZoneWorldState {
    fn cast_zone_world(&self) -> &CastZoneWorldState {
        self
    }

    fn cast_zone_world_mut(&mut self) -> &mut CastZoneWorldState {
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ManaPaymentInput {
    pub units: Vec<ManaUnitId>,
    pub x_value: u32,
    /// Zero-based indexes into the printed symbol list.
    pub phyrexian_life_symbols: BTreeSet<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CostAtomPaymentInput {
    Mana(ManaPaymentInput),
    Life(u32),
    Objects(Vec<ObjectRef>),
    Reveal(ObjectRef),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CostPaymentInput {
    pub atoms: BTreeMap<usize, CostAtomPaymentInput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttackDeclarationWitness {
    pub attacking_player: PlayerId,
    pub attacked_player: PlayerId,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeclareAttackersStepEvidence {
    pub currently_in_declare_attackers_step: bool,
    pub declarations: BTreeSet<AttackDeclarationWitness>,
    /// True only when `declarations` contains every attack declared during
    /// the current declare-attackers step.
    pub declaration_history_complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastExecutionInput {
    pub actor: PlayerId,
    pub source: ObjectRef,
    pub permission_object: Option<ObjectRef>,
    pub base_mana: Option<ManaPaymentInput>,
    pub alternative_cost: Option<CostPaymentInput>,
    pub additional_cost: Option<CostPaymentInput>,
    /// Exact chosen X for a variable nonmana cost. It is rejected unless one
    /// of this cast's typed cost expressions consumes X.
    pub chosen_x: Option<u32>,
    pub declare_attackers: Option<DeclareAttackersStepEvidence>,
    pub external_other_costs_satisfied: bool,
    pub externally_verified_paid_costs: BTreeSet<PaidCostReference>,
    pub conditions: CostConditionState,
    pub timing: TimingExecutionEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimingExecutionEvidence {
    pub during_actors_turn: bool,
    pub sorcery_timing: bool,
    pub later_turn_than_permission_creation: bool,
    pub permission_created_turn: Option<TurnId>,
    pub permission_window_open: bool,
}

impl Default for TimingExecutionEvidence {
    fn default() -> Self {
        Self {
            during_actors_turn: true,
            sorcery_timing: true,
            later_turn_than_permission_creation: true,
            permission_created_turn: None,
            permission_window_open: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastInitiationReceipt {
    pub program_digest: String,
    pub cast_id: CastId,
    pub original_reference: ObjectRef,
    pub stack_or_battlefield_reference: ObjectRef,
    pub kind: PlayKind,
    pub paid_costs: PaidCostLedger,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastResolutionReceipt {
    pub program_digest: String,
    pub cast_id: CastId,
    pub old_stack_reference: ObjectRef,
    pub new_reference: ObjectRef,
    pub destination: Zone,
    pub scheduled_delayed: Vec<PendingInstructionId>,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedResolutionReceipt {
    pub pending_id: PendingInstructionId,
    pub program_digest: String,
    pub object_before: Option<ObjectRef>,
    pub object_after: Option<ObjectRef>,
    pub affected_player: Option<PlayerId>,
    pub applied: bool,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCopyReceipt {
    pub program_digest: String,
    pub original_spell: ObjectRef,
    pub copy: ObjectRef,
    pub is_cast: bool,
    pub may_choose_new_targets: bool,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedCostReceipt {
    pub program_digest: String,
    pub cast_id: CastId,
    pub action: LinkedResolutionAction,
    pub object_after: Option<ObjectRef>,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostAdjustmentInstallationInput {
    pub controller: PlayerId,
    pub source: ObjectRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostAdjustmentReceipt {
    pub program_digest: String,
    pub controller: PlayerId,
    pub source: ObjectRef,
    pub installed: bool,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellCopyExecutionInput {
    pub controller: PlayerId,
    pub target: ObjectRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedScheduleInput {
    pub controller: PlayerId,
    pub expected_object: Option<ObjectRef>,
    pub affected_player: Option<PlayerId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayedScheduleReceipt {
    pub program_digest: String,
    pub pending_id: PendingInstructionId,
    pub expected_object: Option<ObjectRef>,
    pub affected_player: Option<PlayerId>,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraTurnExecutionInput {
    pub controller: PlayerId,
    pub affected_player: Option<PlayerId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtraTurnReceipt {
    pub program_digest: String,
    pub turn_id: TurnId,
    pub player: PlayerId,
    pub delayed_loss_id: PendingInstructionId,
    pub committed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastZoneRuntimeError {
    ProductionAdapterDisconnected,
    WrongProgramKind,
    IncompleteReplacementEvidence,
    IncompleteTimingEvidence,
    IncompleteAttackDeclarationEvidence,
    IncompleteHiddenZoneEvidence,
    IncompleteSpellTargetEvidence,
    MissingPlayer(PlayerId),
    MissingObject(ObjectRef),
    StaleObject(ObjectRef),
    WrongOwner,
    WrongZone {
        expected: BTreeSet<Zone>,
        actual: Zone,
    },
    IllegalTiming,
    IllegalLandPlay,
    MissingPrintedManaCost,
    MissingBaseManaPayment,
    UnexpectedBaseManaPayment,
    MissingCostPayment,
    UnexpectedCostPayment,
    WrongCostPaymentKind(usize),
    WrongPaymentCardinality {
        atom: usize,
        expected: usize,
        actual: usize,
    },
    MissingManaUnit(ManaUnitId),
    DuplicateManaUnit(ManaUnitId),
    ManaPaymentDoesNotMatch,
    UnexpectedX,
    MissingChosenX,
    CannotPayLife,
    IllegalCostObject(ObjectRef),
    InsufficientCounters(ObjectRef),
    UnsatisfiedCondition(CastCondition),
    UnsatisfiedExternalCosts,
    CastIdOverflow,
    ObjectIdOverflow,
    IncarnationOverflow(ObjectRef),
    PendingIdOverflow,
    TurnIdOverflow,
    MissingCast(CastId),
    WrongCastStatus(CastLifecycleStatus),
    MissingPendingInstruction(PendingInstructionId),
    WrongDelayedMoment,
    DelayedObjectChangedZone,
    MissingLinkedCostEvidence,
    LinkedCostActionAlreadyApplied,
    DuplicateCostAdjustment,
    InactiveCostAdjustmentSource,
    TargetConstraintViolation,
    MissingLibraryCard,
    WrongTurn,
    ArithmeticOverflow,
    UnsupportedAdjustmentState,
    UnsupportedLinkedActionState,
    StateInvariantViolation,
}

impl fmt::Display for CastZoneRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CastZoneRuntimeError {}

pub fn begin_cast_transactionally<S: CastZoneStateAdapter>(
    program: &CastZoneEnvelopeProgram,
    input: &CastExecutionInput,
    state: &mut S,
) -> Result<CastInitiationReceipt, CastZoneRuntimeError> {
    let mut staged = state.clone();
    validate_world(staged.cast_zone_world())?;
    let receipt = begin_cast_in_world(program, input, staged.cast_zone_world_mut())?;
    validate_world(staged.cast_zone_world())?;
    *state = staged;
    Ok(receipt)
}

fn begin_cast_in_world(
    program: &CastZoneEnvelopeProgram,
    input: &CastExecutionInput,
    state: &mut CastZoneWorldState,
) -> Result<CastInitiationReceipt, CastZoneRuntimeError> {
    require_execution_boundaries(state)?;
    let permission = match program.kind() {
        CastZoneEnvelopeKind::Permission(permission) => permission,
        CastZoneEnvelopeKind::CastRestriction(restriction) => {
            validate_cast_restriction(restriction, input, state)?;
            return begin_normal_cast_with_cost_envelope(program, input, None, None, state);
        }
        CastZoneEnvelopeKind::AdditionalCost(envelope) => {
            return begin_normal_cast_with_cost_envelope(
                program,
                input,
                Some(envelope),
                None,
                state,
            );
        }
        CastZoneEnvelopeKind::AlternativeCost(envelope) => {
            return begin_normal_cast_with_cost_envelope(
                program,
                input,
                None,
                Some(envelope),
                state,
            );
        }
        _ => return Err(CastZoneRuntimeError::WrongProgramKind),
    };
    if input.actor == 0 || !state.players.contains_key(&input.actor) {
        return Err(CastZoneRuntimeError::MissingPlayer(input.actor));
    }
    let source = state
        .objects
        .get(&input.source)
        .ok_or(CastZoneRuntimeError::MissingObject(input.source))?
        .clone();
    if source.reference != input.source {
        return Err(CastZoneRuntimeError::StaleObject(input.source));
    }
    if source.owner != input.actor {
        return Err(CastZoneRuntimeError::WrongOwner);
    }
    if !permission.from_zones.contains(&source.zone) {
        return Err(CastZoneRuntimeError::WrongZone {
            expected: permission.from_zones.clone(),
            actual: source.zone,
        });
    }
    validate_permission_object(permission, input, state)?;
    validate_timing(permission.timing, input.timing, &source, state.current_turn)?;
    if permission.other_costs_retained && !input.external_other_costs_satisfied {
        return Err(CastZoneRuntimeError::UnsatisfiedExternalCosts);
    }

    if permission.kind == PlayKind::Play && source.card_types.contains(&CardType::Land) {
        if input.base_mana.is_some()
            || input.alternative_cost.is_some()
            || input.additional_cost.is_some()
            || permission.without_paying_mana_cost
            || permission.alternative_cost.is_some()
            || permission.additional_cost.is_some()
        {
            return Err(CastZoneRuntimeError::UnexpectedCostPayment);
        }
        let player = state
            .players
            .get_mut(&input.actor)
            .ok_or(CastZoneRuntimeError::MissingPlayer(input.actor))?;
        if player.land_plays_remaining == 0 {
            return Err(CastZoneRuntimeError::IllegalLandPlay);
        }
        player.land_plays_remaining -= 1;
        let new_reference = move_object(state, input.source, Zone::Battlefield, Some(input.actor))?;
        for template in &permission.delayed {
            schedule_delayed(
                program.semantic_digest(),
                template,
                input.actor,
                Some(new_reference),
                None,
                state,
            )?;
        }
        return Ok(CastInitiationReceipt {
            program_digest: program.semantic_digest().to_owned(),
            cast_id: CastId(0),
            original_reference: input.source,
            stack_or_battlefield_reference: new_reference,
            kind: PlayKind::PlayLand,
            paid_costs: PaidCostLedger {
                base_mana_paid: false,
                without_paying_mana_cost: false,
                alternative_cost_paid: false,
                additional_cost_paid: false,
                paid_mana_costs: Vec::new(),
                life_paid: 0,
                chosen_x: None,
                paid_atoms: Vec::new(),
                externally_verified_paid_costs: input.externally_verified_paid_costs.clone(),
            },
            committed: true,
        });
    }

    let paid_costs = pay_permission_costs(permission, input, &source, state)?;
    create_cast_lifecycle(program, permission, input, paid_costs, state)
}

fn validate_cast_restriction(
    restriction: &CastRestrictionEnvelope,
    input: &CastExecutionInput,
    state: &CastZoneWorldState,
) -> Result<(), CastZoneRuntimeError> {
    let evidence = input
        .declare_attackers
        .as_ref()
        .ok_or(CastZoneRuntimeError::IncompleteAttackDeclarationEvidence)?;
    if !evidence.declaration_history_complete {
        return Err(CastZoneRuntimeError::IncompleteAttackDeclarationEvidence);
    }
    if restriction.during_declare_attackers_step && !evidence.currently_in_declare_attackers_step {
        return Err(CastZoneRuntimeError::IllegalTiming);
    }
    for declaration in &evidence.declarations {
        if declaration.attacking_player == declaration.attacked_player
            || declaration.attacking_player != state.active_player
            || !state.players.contains_key(&declaration.attacking_player)
            || !state.players.contains_key(&declaration.attacked_player)
        {
            return Err(CastZoneRuntimeError::StateInvariantViolation);
        }
    }
    if restriction.caster_was_attacked_this_step
        && !evidence
            .declarations
            .iter()
            .any(|declaration| declaration.attacked_player == input.actor)
    {
        return Err(CastZoneRuntimeError::IllegalTiming);
    }
    Ok(())
}

fn begin_normal_cast_with_cost_envelope(
    program: &CastZoneEnvelopeProgram,
    input: &CastExecutionInput,
    additional: Option<&AdditionalCostEnvelope>,
    alternative: Option<&AlternativeCostEnvelope>,
    state: &mut CastZoneWorldState,
) -> Result<CastInitiationReceipt, CastZoneRuntimeError> {
    require_execution_boundaries(state)?;
    if input.actor == 0 || !state.players.contains_key(&input.actor) {
        return Err(CastZoneRuntimeError::MissingPlayer(input.actor));
    }
    let source = state
        .objects
        .get(&input.source)
        .ok_or(CastZoneRuntimeError::MissingObject(input.source))?
        .clone();
    if source.owner != input.actor {
        return Err(CastZoneRuntimeError::WrongOwner);
    }
    if source.zone != Zone::Hand {
        return Err(CastZoneRuntimeError::WrongZone {
            expected: BTreeSet::from([Zone::Hand]),
            actual: source.zone,
        });
    }
    validate_timing(
        TimingWindow::Normal,
        input.timing,
        &source,
        state.current_turn,
    )?;
    if !input.external_other_costs_satisfied {
        return Err(CastZoneRuntimeError::UnsatisfiedExternalCosts);
    }
    let mut ledger = PaidCostLedger {
        base_mana_paid: false,
        without_paying_mana_cost: false,
        alternative_cost_paid: false,
        additional_cost_paid: false,
        paid_mana_costs: Vec::new(),
        life_paid: 0,
        chosen_x: input.chosen_x,
        paid_atoms: Vec::new(),
        externally_verified_paid_costs: input.externally_verified_paid_costs.clone(),
    };
    validate_nonmana_x_evidence(
        input.chosen_x,
        alternative.and_then(|envelope| envelope.cost.as_ref()),
        additional.map(|envelope| &envelope.cost),
    )?;
    if let Some(alternative) = alternative {
        if let Some(condition) = &alternative.condition
            && !input.conditions.satisfied.contains(condition)
        {
            return Err(CastZoneRuntimeError::UnsatisfiedCondition(
                condition.clone(),
            ));
        }
        if alternative.without_paying_mana_cost {
            if input.base_mana.is_some() || input.alternative_cost.is_some() {
                return Err(CastZoneRuntimeError::UnexpectedBaseManaPayment);
            }
            ledger.without_paying_mana_cost = true;
        } else {
            let expression = alternative
                .cost
                .as_ref()
                .ok_or(CastZoneRuntimeError::MissingCostPayment)?;
            let payment = input
                .alternative_cost
                .as_ref()
                .ok_or(CastZoneRuntimeError::MissingCostPayment)?;
            apply_cost_expression(
                expression,
                payment,
                input.actor,
                input.source,
                input.chosen_x,
                state,
                &mut ledger,
            )?;
            ledger.alternative_cost_paid = true;
        }
    } else {
        pay_printed_mana(&source, input, state, &mut ledger)?;
    }
    if let Some(additional) = additional {
        let should_pay = !additional.optional || input.additional_cost.is_some();
        if should_pay {
            let payment = input
                .additional_cost
                .as_ref()
                .ok_or(CastZoneRuntimeError::MissingCostPayment)?;
            apply_cost_expression(
                &additional.cost,
                payment,
                input.actor,
                input.source,
                input.chosen_x,
                state,
                &mut ledger,
            )?;
            ledger.additional_cost_paid = true;
        }
    } else if input.additional_cost.is_some() {
        return Err(CastZoneRuntimeError::UnexpectedCostPayment);
    }
    let permission = CastPermission {
        actor: PlayerOperand::You,
        kind: PlayKind::Cast,
        object: ObjectOperand::ThisCard,
        from_zones: BTreeSet::from([Zone::Hand]),
        timing: TimingWindow::Normal,
        without_paying_mana_cost: alternative
            .is_some_and(|envelope| envelope.without_paying_mana_cost),
        alternative_cost: alternative.and_then(|envelope| envelope.cost.clone()),
        additional_cost: additional.map(|envelope| envelope.cost.clone()),
        other_costs_retained: true,
        delayed: Vec::new(),
    };
    create_cast_lifecycle(program, &permission, input, ledger, state)
}

fn pay_permission_costs(
    permission: &CastPermission,
    input: &CastExecutionInput,
    source: &GameObject,
    state: &mut CastZoneWorldState,
) -> Result<PaidCostLedger, CastZoneRuntimeError> {
    let mut ledger = PaidCostLedger {
        base_mana_paid: false,
        without_paying_mana_cost: false,
        alternative_cost_paid: false,
        additional_cost_paid: false,
        paid_mana_costs: Vec::new(),
        life_paid: 0,
        chosen_x: input.chosen_x,
        paid_atoms: Vec::new(),
        externally_verified_paid_costs: input.externally_verified_paid_costs.clone(),
    };
    validate_nonmana_x_evidence(
        input.chosen_x,
        permission.alternative_cost.as_ref(),
        permission.additional_cost.as_ref(),
    )?;
    if permission.without_paying_mana_cost {
        if input.base_mana.is_some() || input.alternative_cost.is_some() {
            return Err(CastZoneRuntimeError::UnexpectedBaseManaPayment);
        }
        ledger.without_paying_mana_cost = true;
    } else if let Some(expression) = &permission.alternative_cost {
        let payment = input
            .alternative_cost
            .as_ref()
            .ok_or(CastZoneRuntimeError::MissingCostPayment)?;
        apply_cost_expression(
            expression,
            payment,
            input.actor,
            input.source,
            input.chosen_x,
            state,
            &mut ledger,
        )?;
        ledger.alternative_cost_paid = true;
    } else {
        pay_printed_mana(source, input, state, &mut ledger)?;
    }
    if let Some(expression) = &permission.additional_cost {
        let payment = input
            .additional_cost
            .as_ref()
            .ok_or(CastZoneRuntimeError::MissingCostPayment)?;
        apply_cost_expression(
            expression,
            payment,
            input.actor,
            input.source,
            input.chosen_x,
            state,
            &mut ledger,
        )?;
        ledger.additional_cost_paid = true;
    } else if input.additional_cost.is_some() {
        return Err(CastZoneRuntimeError::UnexpectedCostPayment);
    }
    Ok(ledger)
}

fn pay_printed_mana(
    source: &GameObject,
    input: &CastExecutionInput,
    state: &mut CastZoneWorldState,
    ledger: &mut PaidCostLedger,
) -> Result<(), CastZoneRuntimeError> {
    let printed = source
        .printed_mana_cost
        .as_ref()
        .ok_or(CastZoneRuntimeError::MissingPrintedManaCost)?;
    let payable = adjusted_mana_cost(source, printed, input, state)?;
    let payment = input
        .base_mana
        .as_ref()
        .ok_or(CastZoneRuntimeError::MissingBaseManaPayment)?;
    pay_mana_cost(input.actor, &payable, payment, state, ledger)?;
    ledger.base_mana_paid = true;
    Ok(())
}

fn create_cast_lifecycle(
    program: &CastZoneEnvelopeProgram,
    permission: &CastPermission,
    input: &CastExecutionInput,
    paid_costs: PaidCostLedger,
    state: &mut CastZoneWorldState,
) -> Result<CastInitiationReceipt, CastZoneRuntimeError> {
    let cast_id = CastId(state.next_cast_id);
    state.next_cast_id = state
        .next_cast_id
        .checked_add(1)
        .ok_or(CastZoneRuntimeError::CastIdOverflow)?;
    let origin_zone = state
        .objects
        .get(&input.source)
        .ok_or(CastZoneRuntimeError::MissingObject(input.source))?
        .zone;
    let stack_reference = move_object(state, input.source, Zone::Stack, Some(input.actor))?;
    state.stack.push(stack_reference);
    state.casts.insert(
        cast_id,
        CastLifecycle {
            cast_id,
            program_digest: program.semantic_digest().to_owned(),
            actor: input.actor,
            original_reference: input.source,
            stack_reference,
            resolved_reference: None,
            status: CastLifecycleStatus::OnStack,
            paid_costs: paid_costs.clone(),
            delayed_templates: permission.delayed.clone(),
            pending_entry_counters: Vec::new(),
            cast_turn: state.current_turn,
            origin_zone,
        },
    );
    Ok(CastInitiationReceipt {
        program_digest: program.semantic_digest().to_owned(),
        cast_id,
        original_reference: input.source,
        stack_or_battlefield_reference: stack_reference,
        kind: PlayKind::Cast,
        paid_costs,
        committed: true,
    })
}

fn validate_permission_object(
    permission: &CastPermission,
    input: &CastExecutionInput,
    state: &CastZoneWorldState,
) -> Result<(), CastZoneRuntimeError> {
    let selected = input.permission_object.unwrap_or(input.source);
    if selected != input.source {
        return Err(CastZoneRuntimeError::IllegalCostObject(selected));
    }
    let source = state
        .objects
        .get(&selected)
        .ok_or(CastZoneRuntimeError::MissingObject(selected))?;
    match permission.object {
        ObjectOperand::ThisCard | ObjectOperand::It | ObjectOperand::ThatCard => {}
        ObjectOperand::TopCardOfYourLibrary => {
            if !state.hidden_zone_evidence_complete {
                return Err(CastZoneRuntimeError::IncompleteHiddenZoneEvidence);
            }
            let top = state
                .libraries
                .get(&input.actor)
                .and_then(|library| library.last())
                .copied();
            if top != Some(selected) || source.zone != Zone::Library {
                return Err(CastZoneRuntimeError::IllegalCostObject(selected));
            }
        }
        ObjectOperand::CardsFromYourGraveyard => {
            if source.owner != input.actor || source.zone != Zone::Graveyard {
                return Err(CastZoneRuntimeError::IllegalCostObject(selected));
            }
        }
        ObjectOperand::CardsFromExileYouOwn | ObjectOperand::ExiledCardsWithSource => {
            if source.owner != input.actor || source.zone != Zone::Exile {
                return Err(CastZoneRuntimeError::IllegalCostObject(selected));
            }
        }
        _ => return Err(CastZoneRuntimeError::IllegalCostObject(selected)),
    }
    Ok(())
}

fn validate_timing(
    timing: TimingWindow,
    evidence: TimingExecutionEvidence,
    source: &GameObject,
    current_turn: TurnId,
) -> Result<(), CastZoneRuntimeError> {
    let normal_timing = source.card_types.contains(&CardType::Instant) || evidence.sorcery_timing;
    let legal = match timing {
        TimingWindow::Normal => normal_timing,
        TimingWindow::ThisTurn | TimingWindow::UntilEndOfTurn => {
            normal_timing
                && evidence.permission_window_open
                && evidence.permission_created_turn == Some(current_turn)
        }
        TimingWindow::UntilYourNextTurn => normal_timing && evidence.permission_window_open,
        TimingWindow::DuringYourTurn => evidence.during_actors_turn && normal_timing,
        TimingWindow::AsThoughFlash => true,
        TimingWindow::Sorcery => evidence.sorcery_timing,
        TimingWindow::LaterTurn => {
            normal_timing
                && evidence.permission_window_open
                && evidence.later_turn_than_permission_creation
                && evidence
                    .permission_created_turn
                    .is_some_and(|created| current_turn > created)
        }
    };
    legal
        .then_some(())
        .ok_or(CastZoneRuntimeError::IllegalTiming)
}

fn require_execution_boundaries(state: &CastZoneWorldState) -> Result<(), CastZoneRuntimeError> {
    if !state.no_applicable_replacement_effects {
        return Err(CastZoneRuntimeError::IncompleteReplacementEvidence);
    }
    if !state.timing_evidence_complete {
        return Err(CastZoneRuntimeError::IncompleteTimingEvidence);
    }
    Ok(())
}

pub fn install_cost_adjustment_transactionally<S: CastZoneStateAdapter>(
    program: &CastZoneEnvelopeProgram,
    input: &CostAdjustmentInstallationInput,
    state: &mut S,
) -> Result<CostAdjustmentReceipt, CastZoneRuntimeError> {
    let adjustment = match program.kind() {
        CastZoneEnvelopeKind::CostAdjustment(adjustment) => adjustment,
        _ => return Err(CastZoneRuntimeError::WrongProgramKind),
    };
    let mut staged = state.clone();
    validate_world(staged.cast_zone_world())?;
    require_execution_boundaries(staged.cast_zone_world())?;
    let world = staged.cast_zone_world_mut();
    if !world.players.contains_key(&input.controller) {
        return Err(CastZoneRuntimeError::MissingPlayer(input.controller));
    }
    let source = world
        .objects
        .get(&input.source)
        .ok_or(CastZoneRuntimeError::MissingObject(input.source))?;
    let source_is_valid = match adjustment.subject {
        CostAdjustmentSubject::ThisSpell => {
            source.owner == input.controller
                && matches!(
                    source.zone,
                    Zone::Hand | Zone::Graveyard | Zone::Exile | Zone::Command | Zone::Stack
                )
        }
        _ => source.zone == Zone::Battlefield && source.controller == input.controller,
    };
    if !source_is_valid {
        return Err(CastZoneRuntimeError::InactiveCostAdjustmentSource);
    }
    if world.active_cost_adjustments.iter().any(|active| {
        active.controller == input.controller
            && active.source == input.source
            && active.program_digest == program.semantic_digest()
    }) {
        return Err(CastZoneRuntimeError::DuplicateCostAdjustment);
    }
    world.active_cost_adjustments.push(ActiveCostAdjustment {
        controller: input.controller,
        source: input.source,
        program_digest: program.semantic_digest().to_owned(),
        adjustment: adjustment.clone(),
    });
    validate_world(staged.cast_zone_world())?;
    *state = staged;
    Ok(CostAdjustmentReceipt {
        program_digest: program.semantic_digest().to_owned(),
        controller: input.controller,
        source: input.source,
        installed: true,
        committed: true,
    })
}

fn adjusted_mana_cost(
    spell: &GameObject,
    printed: &ManaCost,
    input: &CastExecutionInput,
    state: &CastZoneWorldState,
) -> Result<ManaCost, CastZoneRuntimeError> {
    let applicable = state
        .active_cost_adjustments
        .iter()
        .filter(|active| {
            cost_adjustment_source_is_active(active, spell, state)
                && cost_adjustment_subject_applies(active, spell, input.actor)
        })
        .collect::<Vec<_>>();
    if applicable.is_empty() {
        return Ok(printed.clone());
    }
    if printed.symbols.contains(&ManaSymbol::VariableX) {
        // Applying generic modifiers to an X cost requires the declared X
        // choice to be part of the cost-construction transaction. This
        // standalone boundary does not guess that choice.
        return Err(CastZoneRuntimeError::UnsupportedAdjustmentState);
    }

    let mut increase = 0u32;
    let mut reduction = 0u32;
    let mut minimum_one = false;
    for active in applicable {
        let amount = evaluate_adjustment_amount(&active.adjustment, spell, input, state)?;
        match active.adjustment.direction {
            CostAdjustmentDirection::Increase => {
                increase = increase
                    .checked_add(amount)
                    .ok_or(CastZoneRuntimeError::ArithmeticOverflow)?;
            }
            CostAdjustmentDirection::Reduce => {
                reduction = reduction
                    .checked_add(amount)
                    .ok_or(CastZoneRuntimeError::ArithmeticOverflow)?;
                minimum_one |= active.adjustment.minimum_one;
            }
        }
    }

    let printed_generic = printed
        .symbols
        .iter()
        .filter_map(|symbol| match symbol {
            ManaSymbol::Generic(value) => Some(*value),
            _ => None,
        })
        .try_fold(0u32, |total, value| total.checked_add(value))
        .ok_or(CastZoneRuntimeError::ArithmeticOverflow)?;
    let non_generic_count = printed
        .symbols
        .iter()
        .filter(|symbol| !matches!(symbol, ManaSymbol::Generic(_)))
        .count() as u32;
    let mut adjusted_generic = printed_generic
        .checked_add(increase)
        .ok_or(CastZoneRuntimeError::ArithmeticOverflow)?
        .saturating_sub(reduction);
    if minimum_one && non_generic_count == 0 {
        adjusted_generic = adjusted_generic.max(1);
    }

    let mut symbols = Vec::new();
    if adjusted_generic > 0 {
        symbols.push(ManaSymbol::Generic(adjusted_generic));
    }
    symbols.extend(
        printed
            .symbols
            .iter()
            .copied()
            .filter(|symbol| !matches!(symbol, ManaSymbol::Generic(_))),
    );
    if symbols.is_empty() {
        // A fully reduced generic cost is a legal zero mana cost.
        symbols.push(ManaSymbol::Generic(0));
    }
    let exact = symbols
        .iter()
        .map(|symbol| match symbol {
            ManaSymbol::Generic(value) => format!("{{{value}}}"),
            ManaSymbol::White => "{W}".into(),
            ManaSymbol::Blue => "{U}".into(),
            ManaSymbol::Black => "{B}".into(),
            ManaSymbol::Red => "{R}".into(),
            ManaSymbol::Green => "{G}".into(),
            ManaSymbol::Colorless => "{C}".into(),
            ManaSymbol::Snow => "{S}".into(),
            ManaSymbol::Hybrid(first, second) => {
                format!(
                    "{{{}/{}}}",
                    first.stable_id().to_ascii_uppercase(),
                    second.stable_id().to_ascii_uppercase()
                )
            }
            ManaSymbol::Phyrexian(color) => {
                format!("{{{}/P}}", color.stable_id().to_ascii_uppercase())
            }
            ManaSymbol::VariableX => "{X}".into(),
        })
        .collect::<String>();
    Ok(ManaCost { exact, symbols })
}

fn cost_adjustment_source_is_active(
    active: &ActiveCostAdjustment,
    spell: &GameObject,
    state: &CastZoneWorldState,
) -> bool {
    let Some(source) = state.objects.get(&active.source) else {
        return false;
    };
    match active.adjustment.subject {
        CostAdjustmentSubject::ThisSpell => source.reference == spell.reference,
        _ => source.zone == Zone::Battlefield && source.controller == active.controller,
    }
}

fn cost_adjustment_subject_applies(
    active: &ActiveCostAdjustment,
    spell: &GameObject,
    actor: PlayerId,
) -> bool {
    if active.controller != actor {
        return false;
    }
    match active.adjustment.subject {
        CostAdjustmentSubject::ThisSpell => active.source == spell.reference,
        CostAdjustmentSubject::SpellsYouCast => true,
        CostAdjustmentSubject::CreatureSpellsYouCast => {
            spell.card_types.contains(&CardType::Creature)
        }
        CostAdjustmentSubject::ArtifactSpellsYouCast => {
            spell.card_types.contains(&CardType::Artifact)
        }
        CostAdjustmentSubject::CommanderSpellsYouCast => spell.is_commander,
    }
}

fn evaluate_adjustment_amount(
    adjustment: &CostAdjustment,
    spell: &GameObject,
    input: &CastExecutionInput,
    state: &CastZoneWorldState,
) -> Result<u32, CastZoneRuntimeError> {
    let base = match adjustment.amount {
        AmountOperand::Fixed(value) => value,
        AmountOperand::ManaValue => mana_value(spell.printed_mana_cost.as_ref()),
        AmountOperand::NumberOfCardsInYourHand => count_objects(state, |object| {
            object.owner == input.actor && object.zone == Zone::Hand
        })?,
        AmountOperand::NumberOfOpponents => count_opponents(state, input.actor)?,
        AmountOperand::X | AmountOperand::NumberOfObjectsMatching => {
            return Err(CastZoneRuntimeError::UnsupportedAdjustmentState);
        }
    };
    let multiplier = adjustment
        .qualifier
        .as_ref()
        .map(|qualifier| evaluate_qualifier_count(qualifier, spell, input, state))
        .transpose()?
        .unwrap_or(1);
    base.checked_mul(multiplier)
        .ok_or(CastZoneRuntimeError::ArithmeticOverflow)
}

fn evaluate_qualifier_count(
    qualifier: &AdjustmentQualifier,
    spell: &GameObject,
    input: &CastExecutionInput,
    state: &CastZoneWorldState,
) -> Result<u32, CastZoneRuntimeError> {
    let derived = match qualifier {
        AdjustmentQualifier::ForEachOpponent => count_opponents(state, input.actor)?,
        AdjustmentQualifier::ForEachCardInYourHand => count_objects(state, |object| {
            object.owner == input.actor && object.zone == Zone::Hand
        })?,
        AdjustmentQualifier::ForEachCardInYourGraveyard => count_objects(state, |object| {
            object.owner == input.actor && object.zone == Zone::Graveyard
        })?,
        AdjustmentQualifier::ForEachCreatureYouControl => count_objects(state, |object| {
            object.controller == input.actor
                && object.zone == Zone::Battlefield
                && object.card_types.contains(&CardType::Creature)
        })?,
        AdjustmentQualifier::ForEachArtifactYouControl => count_objects(state, |object| {
            object.controller == input.actor
                && object.zone == Zone::Battlefield
                && object.card_types.contains(&CardType::Artifact)
        })?,
        AdjustmentQualifier::ForEachColoredManaSymbolInItsManaCost => spell
            .printed_mana_cost
            .as_ref()
            .map(|cost| {
                cost.symbols
                    .iter()
                    .filter(|symbol| {
                        matches!(
                            symbol,
                            ManaSymbol::White
                                | ManaSymbol::Blue
                                | ManaSymbol::Black
                                | ManaSymbol::Red
                                | ManaSymbol::Green
                                | ManaSymbol::Hybrid(_, _)
                                | ManaSymbol::Phyrexian(_)
                        )
                    })
                    .count() as u32
            })
            .unwrap_or(0),
        AdjustmentQualifier::ForEachPreviousCommanderCast => state
            .casts
            .values()
            .filter(|cast| {
                cast.actor == input.actor
                    && cast.original_reference.object_id == spell.reference.object_id
                    && cast.origin_zone == Zone::Command
            })
            .count()
            .try_into()
            .map_err(|_| CastZoneRuntimeError::ArithmeticOverflow)?,
    };
    if let Some(asserted) = input.conditions.qualifier_counts.get(qualifier)
        && *asserted != derived
    {
        return Err(CastZoneRuntimeError::UnsupportedAdjustmentState);
    }
    Ok(derived)
}

fn count_opponents(
    state: &CastZoneWorldState,
    actor: PlayerId,
) -> Result<u32, CastZoneRuntimeError> {
    state
        .players
        .iter()
        .filter(|(player, state)| **player != actor && !state.lost_game)
        .count()
        .try_into()
        .map_err(|_| CastZoneRuntimeError::ArithmeticOverflow)
}

fn count_objects(
    state: &CastZoneWorldState,
    predicate: impl Fn(&GameObject) -> bool,
) -> Result<u32, CastZoneRuntimeError> {
    state
        .objects
        .values()
        .filter(|object| predicate(object))
        .count()
        .try_into()
        .map_err(|_| CastZoneRuntimeError::ArithmeticOverflow)
}

fn mana_value(cost: Option<&ManaCost>) -> u32 {
    cost.map_or(0, |cost| {
        cost.symbols
            .iter()
            .map(|symbol| match symbol {
                ManaSymbol::Generic(value) => *value,
                ManaSymbol::VariableX => 0,
                ManaSymbol::Hybrid(_, _)
                | ManaSymbol::Phyrexian(_)
                | ManaSymbol::White
                | ManaSymbol::Blue
                | ManaSymbol::Black
                | ManaSymbol::Red
                | ManaSymbol::Green
                | ManaSymbol::Colorless
                | ManaSymbol::Snow => 1,
            })
            .sum()
    })
}

fn validate_nonmana_x_evidence(
    chosen_x: Option<u32>,
    first: Option<&CostExpression>,
    second: Option<&CostExpression>,
) -> Result<(), CastZoneRuntimeError> {
    let consumes_x = first
        .into_iter()
        .chain(second)
        .flat_map(|expression| &expression.atoms)
        .any(|atom| matches!(atom, CostAtom::DiscardXCards));
    match (consumes_x, chosen_x) {
        (true, None) => Err(CastZoneRuntimeError::MissingChosenX),
        (false, Some(_)) => Err(CastZoneRuntimeError::UnexpectedX),
        _ => Ok(()),
    }
}

fn apply_cost_expression(
    expression: &CostExpression,
    payment: &CostPaymentInput,
    actor: PlayerId,
    source: ObjectRef,
    chosen_x: Option<u32>,
    state: &mut CastZoneWorldState,
    ledger: &mut PaidCostLedger,
) -> Result<(), CastZoneRuntimeError> {
    if payment.atoms.len() != expression.atoms.len() {
        return Err(CastZoneRuntimeError::WrongPaymentCardinality {
            atom: usize::MAX,
            expected: expression.atoms.len(),
            actual: payment.atoms.len(),
        });
    }
    for (index, atom) in expression.atoms.iter().enumerate() {
        let evidence = payment
            .atoms
            .get(&index)
            .ok_or(CastZoneRuntimeError::MissingCostPayment)?;
        match (atom, evidence) {
            (CostAtom::Mana(cost), CostAtomPaymentInput::Mana(mana)) => {
                pay_mana_cost(actor, cost, mana, state, ledger)?;
            }
            (CostAtom::PayLife(expected), CostAtomPaymentInput::Life(actual))
                if expected == actual =>
            {
                pay_life(actor, *actual, state)?;
                ledger.life_paid = ledger.life_paid.saturating_add(*actual);
            }
            (CostAtom::DiscardCard, CostAtomPaymentInput::Objects(objects)) => {
                require_exact_objects(index, objects, 1)?;
                validate_and_move_cost_object(
                    actor,
                    objects[0],
                    Zone::Hand,
                    None,
                    Zone::Graveyard,
                    state,
                )?;
            }
            (CostAtom::DiscardXCards, CostAtomPaymentInput::Objects(objects)) => {
                let amount = chosen_x.ok_or(CastZoneRuntimeError::MissingChosenX)?;
                let amount = usize::try_from(amount)
                    .map_err(|_| CastZoneRuntimeError::ArithmeticOverflow)?;
                require_exact_objects(index, objects, amount)?;
                for object in objects {
                    validate_and_move_cost_object(
                        actor,
                        *object,
                        Zone::Hand,
                        None,
                        Zone::Graveyard,
                        state,
                    )?;
                }
            }
            (CostAtom::DiscardLandCard, CostAtomPaymentInput::Objects(objects)) => {
                require_exact_objects(index, objects, 1)?;
                validate_and_move_cost_object(
                    actor,
                    objects[0],
                    Zone::Hand,
                    Some(CardType::Land),
                    Zone::Graveyard,
                    state,
                )?;
            }
            (CostAtom::SacrificeCreature, CostAtomPaymentInput::Objects(objects)) => {
                require_exact_objects(index, objects, 1)?;
                validate_and_move_cost_object(
                    actor,
                    objects[0],
                    Zone::Battlefield,
                    Some(CardType::Creature),
                    Zone::Graveyard,
                    state,
                )?;
            }
            (CostAtom::SacrificeArtifactOrCreature, CostAtomPaymentInput::Objects(objects)) => {
                require_exact_objects(index, objects, 1)?;
                let object = state
                    .objects
                    .get(&objects[0])
                    .ok_or(CastZoneRuntimeError::MissingObject(objects[0]))?;
                if object.controller != actor
                    || object.zone != Zone::Battlefield
                    || (!object.card_types.contains(&CardType::Artifact)
                        && !object.card_types.contains(&CardType::Creature))
                {
                    return Err(CastZoneRuntimeError::IllegalCostObject(objects[0]));
                }
                move_object(state, objects[0], Zone::Graveyard, None)?;
            }
            (CostAtom::SacrificePermanent, CostAtomPaymentInput::Objects(objects)) => {
                require_exact_objects(index, objects, 1)?;
                let object = state
                    .objects
                    .get(&objects[0])
                    .ok_or(CastZoneRuntimeError::MissingObject(objects[0]))?;
                if object.controller != actor || !object.is_permanent() {
                    return Err(CastZoneRuntimeError::IllegalCostObject(objects[0]));
                }
                move_object(state, objects[0], Zone::Graveyard, None)?;
            }
            (CostAtom::ExileCardFromYourGraveyard, CostAtomPaymentInput::Objects(objects)) => {
                require_exact_objects(index, objects, 1)?;
                validate_and_move_cost_object(
                    actor,
                    objects[0],
                    Zone::Graveyard,
                    None,
                    Zone::Exile,
                    state,
                )?;
            }
            (
                CostAtom::ExileCreatureCardFromYourGraveyard,
                CostAtomPaymentInput::Objects(objects),
            ) => {
                require_exact_objects(index, objects, 1)?;
                validate_and_move_cost_object(
                    actor,
                    objects[0],
                    Zone::Graveyard,
                    Some(CardType::Creature),
                    Zone::Exile,
                    state,
                )?;
            }
            (
                CostAtom::ExileCardsFromYourGraveyard(expected),
                CostAtomPaymentInput::Objects(objects),
            ) => {
                require_exact_objects(index, objects, *expected as usize)?;
                ensure_distinct(objects)?;
                for object in objects {
                    validate_and_move_cost_object(
                        actor,
                        *object,
                        Zone::Graveyard,
                        None,
                        Zone::Exile,
                        state,
                    )?;
                }
            }
            (
                CostAtom::RemoveCounter {
                    counter,
                    amount,
                    from,
                },
                CostAtomPaymentInput::Objects(objects),
            ) => {
                require_exact_objects(index, objects, 1)?;
                let expected = resolve_cost_object_operand(from, source);
                if expected != Some(objects[0]) {
                    return Err(CastZoneRuntimeError::IllegalCostObject(objects[0]));
                }
                let object = state
                    .objects
                    .get_mut(&objects[0])
                    .ok_or(CastZoneRuntimeError::MissingObject(objects[0]))?;
                let current = object.counters.get(counter).copied().unwrap_or(0);
                if current < *amount {
                    return Err(CastZoneRuntimeError::InsufficientCounters(objects[0]));
                }
                if current == *amount {
                    object.counters.remove(counter);
                } else {
                    object.counters.insert(counter.clone(), current - amount);
                }
            }
            (CostAtom::TapUntappedPermanentYouControl, CostAtomPaymentInput::Objects(objects)) => {
                require_exact_objects(index, objects, 1)?;
                let object = state
                    .objects
                    .get_mut(&objects[0])
                    .ok_or(CastZoneRuntimeError::MissingObject(objects[0]))?;
                if object.controller != actor
                    || object.zone != Zone::Battlefield
                    || !object.is_permanent()
                    || object.tapped
                {
                    return Err(CastZoneRuntimeError::IllegalCostObject(objects[0]));
                }
                object.tapped = true;
            }
            (CostAtom::RevealCardFromYourHand, CostAtomPaymentInput::Reveal(object)) => {
                let object_state = state
                    .objects
                    .get(object)
                    .ok_or(CastZoneRuntimeError::MissingObject(*object))?;
                if object_state.owner != actor || object_state.zone != Zone::Hand {
                    return Err(CastZoneRuntimeError::IllegalCostObject(*object));
                }
            }
            _ => return Err(CastZoneRuntimeError::WrongCostPaymentKind(index)),
        }
        ledger.paid_atoms.push(atom.clone());
    }
    Ok(())
}

fn resolve_cost_object_operand(operand: &ObjectOperand, source: ObjectRef) -> Option<ObjectRef> {
    match operand {
        ObjectOperand::ThisCard
        | ObjectOperand::ThisSpell
        | ObjectOperand::It
        | ObjectOperand::ThatCard
        | ObjectOperand::ThatSpell => Some(source),
        _ => None,
    }
}

fn require_exact_objects(
    atom: usize,
    objects: &[ObjectRef],
    expected: usize,
) -> Result<(), CastZoneRuntimeError> {
    if objects.len() != expected {
        return Err(CastZoneRuntimeError::WrongPaymentCardinality {
            atom,
            expected,
            actual: objects.len(),
        });
    }
    ensure_distinct(objects)
}

fn ensure_distinct(objects: &[ObjectRef]) -> Result<(), CastZoneRuntimeError> {
    let unique = objects.iter().copied().collect::<BTreeSet<_>>();
    if unique.len() != objects.len() {
        return Err(CastZoneRuntimeError::IllegalCostObject(
            objects.first().copied().unwrap_or(ObjectRef {
                object_id: 0,
                incarnation_id: IncarnationId(0),
            }),
        ));
    }
    Ok(())
}

fn validate_and_move_cost_object(
    actor: PlayerId,
    object_ref: ObjectRef,
    from: Zone,
    required_type: Option<CardType>,
    to: Zone,
    state: &mut CastZoneWorldState,
) -> Result<ObjectRef, CastZoneRuntimeError> {
    let object = state
        .objects
        .get(&object_ref)
        .ok_or(CastZoneRuntimeError::MissingObject(object_ref))?;
    let controlled_cost = from == Zone::Battlefield;
    if object.zone != from
        || (controlled_cost && object.controller != actor)
        || (!controlled_cost && object.owner != actor)
        || required_type.is_some_and(|card_type| !object.card_types.contains(&card_type))
    {
        return Err(CastZoneRuntimeError::IllegalCostObject(object_ref));
    }
    move_object(state, object_ref, to, None)
}

fn pay_life(
    actor: PlayerId,
    amount: u32,
    state: &mut CastZoneWorldState,
) -> Result<(), CastZoneRuntimeError> {
    let player = state
        .players
        .get_mut(&actor)
        .ok_or(CastZoneRuntimeError::MissingPlayer(actor))?;
    if amount == 0 || player.life <= i64::from(amount) {
        return Err(CastZoneRuntimeError::CannotPayLife);
    }
    player.life -= i64::from(amount);
    Ok(())
}

fn pay_mana_cost(
    actor: PlayerId,
    cost: &ManaCost,
    payment: &ManaPaymentInput,
    state: &mut CastZoneWorldState,
    ledger: &mut PaidCostLedger,
) -> Result<(), CastZoneRuntimeError> {
    let player = state
        .players
        .get(&actor)
        .ok_or(CastZoneRuntimeError::MissingPlayer(actor))?;
    let mut seen = BTreeSet::new();
    let units = payment
        .units
        .iter()
        .map(|id| {
            if !seen.insert(*id) {
                return Err(CastZoneRuntimeError::DuplicateManaUnit(*id));
            }
            player
                .mana_pool
                .get(id)
                .copied()
                .ok_or(CastZoneRuntimeError::MissingManaUnit(*id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let x_symbols = cost
        .symbols
        .iter()
        .filter(|symbol| **symbol == ManaSymbol::VariableX)
        .count();
    if x_symbols == 0 && payment.x_value != 0 {
        return Err(CastZoneRuntimeError::UnexpectedX);
    }

    let mut colored_requirements = Vec::new();
    let mut generic = 0usize;
    let mut phyrexian_life = 0u32;
    for (index, symbol) in cost.symbols.iter().copied().enumerate() {
        match symbol {
            ManaSymbol::Generic(amount) => {
                generic = generic.saturating_add(amount as usize);
            }
            ManaSymbol::VariableX => {
                generic = generic.saturating_add(payment.x_value as usize);
            }
            ManaSymbol::Phyrexian(color) if payment.phyrexian_life_symbols.contains(&index) => {
                phyrexian_life = phyrexian_life.saturating_add(2);
                let _ = color;
            }
            other => colored_requirements.push(other),
        }
    }
    if payment
        .phyrexian_life_symbols
        .iter()
        .any(|index| !matches!(cost.symbols.get(*index), Some(ManaSymbol::Phyrexian(_))))
    {
        return Err(CastZoneRuntimeError::ManaPaymentDoesNotMatch);
    }
    if units.len() != colored_requirements.len().saturating_add(generic)
        || !mana_assignment_exists(&colored_requirements, &units)
    {
        return Err(CastZoneRuntimeError::ManaPaymentDoesNotMatch);
    }
    pay_life_if_nonzero(actor, phyrexian_life, state)?;
    let player = state
        .players
        .get_mut(&actor)
        .ok_or(CastZoneRuntimeError::MissingPlayer(actor))?;
    for unit in units {
        player.mana_pool.remove(&unit.id);
    }
    ledger.life_paid = ledger.life_paid.saturating_add(phyrexian_life);
    ledger.paid_mana_costs.push(cost.clone());
    Ok(())
}

fn pay_life_if_nonzero(
    actor: PlayerId,
    amount: u32,
    state: &mut CastZoneWorldState,
) -> Result<(), CastZoneRuntimeError> {
    if amount == 0 {
        Ok(())
    } else {
        pay_life(actor, amount, state)
    }
}

fn mana_assignment_exists(requirements: &[ManaSymbol], units: &[ManaUnit]) -> bool {
    fn search(
        requirement: usize,
        requirements: &[ManaSymbol],
        units: &[ManaUnit],
        used: &mut BTreeSet<usize>,
    ) -> bool {
        if requirement == requirements.len() {
            return true;
        }
        for (index, unit) in units.iter().enumerate() {
            if used.contains(&index) || !mana_matches(requirements[requirement], *unit) {
                continue;
            }
            used.insert(index);
            if search(requirement + 1, requirements, units, used) {
                return true;
            }
            used.remove(&index);
        }
        false
    }
    search(0, requirements, units, &mut BTreeSet::new())
}

fn mana_matches(requirement: ManaSymbol, unit: ManaUnit) -> bool {
    match requirement {
        ManaSymbol::White => unit.color == ManaColor::White,
        ManaSymbol::Blue => unit.color == ManaColor::Blue,
        ManaSymbol::Black => unit.color == ManaColor::Black,
        ManaSymbol::Red => unit.color == ManaColor::Red,
        ManaSymbol::Green => unit.color == ManaColor::Green,
        ManaSymbol::Colorless => unit.color == ManaColor::Colorless,
        ManaSymbol::Snow => unit.snow,
        ManaSymbol::Hybrid(first, second) => unit.color == first || unit.color == second,
        ManaSymbol::Phyrexian(color) => unit.color == color,
        ManaSymbol::Generic(_) | ManaSymbol::VariableX => true,
    }
}

pub fn resolve_cast_transactionally<S: CastZoneStateAdapter>(
    cast_id: CastId,
    destination: Zone,
    state: &mut S,
) -> Result<CastResolutionReceipt, CastZoneRuntimeError> {
    let mut staged = state.clone();
    validate_world(staged.cast_zone_world())?;
    require_execution_boundaries(staged.cast_zone_world())?;
    let receipt = resolve_cast_in_world(cast_id, destination, staged.cast_zone_world_mut())?;
    validate_world(staged.cast_zone_world())?;
    *state = staged;
    Ok(receipt)
}

fn resolve_cast_in_world(
    cast_id: CastId,
    destination: Zone,
    state: &mut CastZoneWorldState,
) -> Result<CastResolutionReceipt, CastZoneRuntimeError> {
    let lifecycle = state
        .casts
        .get(&cast_id)
        .ok_or(CastZoneRuntimeError::MissingCast(cast_id))?
        .clone();
    if lifecycle.status != CastLifecycleStatus::OnStack {
        return Err(CastZoneRuntimeError::WrongCastStatus(lifecycle.status));
    }
    if !matches!(destination, Zone::Battlefield | Zone::Graveyard) {
        return Err(CastZoneRuntimeError::WrongZone {
            expected: BTreeSet::from([Zone::Battlefield, Zone::Graveyard]),
            actual: destination,
        });
    }
    let object = state.objects.get(&lifecycle.stack_reference).ok_or(
        CastZoneRuntimeError::MissingObject(lifecycle.stack_reference),
    )?;
    if object.zone != Zone::Stack {
        return Err(CastZoneRuntimeError::WrongZone {
            expected: BTreeSet::from([Zone::Stack]),
            actual: object.zone,
        });
    }
    if destination == Zone::Battlefield && !object.is_permanent() {
        return Err(CastZoneRuntimeError::WrongZone {
            expected: BTreeSet::from([Zone::Graveyard, Zone::Exile]),
            actual: destination,
        });
    }
    let new_reference = move_object(
        state,
        lifecycle.stack_reference,
        destination,
        (destination == Zone::Battlefield).then_some(lifecycle.actor),
    )?;
    if destination == Zone::Battlefield {
        let object = state
            .objects
            .get_mut(&new_reference)
            .ok_or(CastZoneRuntimeError::MissingObject(new_reference))?;
        for (counter, amount) in &lifecycle.pending_entry_counters {
            let current = object.counters.get(counter).copied().unwrap_or(0);
            let updated = current
                .checked_add(*amount)
                .ok_or(CastZoneRuntimeError::ArithmeticOverflow)?;
            object.counters.insert(counter.clone(), updated);
        }
    }
    state
        .stack
        .retain(|reference| *reference != lifecycle.stack_reference);
    let mut scheduled_delayed = Vec::new();
    for template in &lifecycle.delayed_templates {
        let id = schedule_delayed(
            &lifecycle.program_digest,
            template,
            lifecycle.actor,
            Some(new_reference),
            None,
            state,
        )?;
        scheduled_delayed.push(id);
    }
    let record = state
        .casts
        .get_mut(&cast_id)
        .ok_or(CastZoneRuntimeError::MissingCast(cast_id))?;
    record.resolved_reference = Some(new_reference);
    record.status = CastLifecycleStatus::Resolved;
    Ok(CastResolutionReceipt {
        program_digest: lifecycle.program_digest,
        cast_id,
        old_stack_reference: lifecycle.stack_reference,
        new_reference,
        destination,
        scheduled_delayed,
        committed: true,
    })
}

fn validate_world(state: &CastZoneWorldState) -> Result<(), CastZoneRuntimeError> {
    if state.active_player != 0 && !state.players.contains_key(&state.active_player) {
        return Err(CastZoneRuntimeError::StateInvariantViolation);
    }
    if state.players.keys().any(|player| *player == 0) {
        return Err(CastZoneRuntimeError::StateInvariantViolation);
    }

    let mut object_ids = BTreeSet::new();
    for (reference, object) in &state.objects {
        if reference != &object.reference
            || reference.object_id == 0
            || reference.incarnation_id.0 == 0
            || object.owner == 0
            || object.controller == 0
            || !state.players.contains_key(&object.owner)
            || !state.players.contains_key(&object.controller)
            || !object_ids.insert(reference.object_id)
            || object.counters.values().any(|amount| *amount == 0)
        {
            return Err(CastZoneRuntimeError::StateInvariantViolation);
        }
    }

    let mut indexed_library_objects = BTreeSet::new();
    for (owner, library) in &state.libraries {
        if !state.players.contains_key(owner) {
            return Err(CastZoneRuntimeError::StateInvariantViolation);
        }
        for reference in library {
            if !indexed_library_objects.insert(*reference) {
                return Err(CastZoneRuntimeError::StateInvariantViolation);
            }
            let object = state
                .objects
                .get(reference)
                .ok_or(CastZoneRuntimeError::StateInvariantViolation)?;
            if object.zone != Zone::Library || object.owner != *owner {
                return Err(CastZoneRuntimeError::StateInvariantViolation);
            }
        }
    }
    if state.objects.iter().any(|(reference, object)| {
        object.zone == Zone::Library && !indexed_library_objects.contains(reference)
    }) {
        return Err(CastZoneRuntimeError::StateInvariantViolation);
    }

    let stack_set = state.stack.iter().copied().collect::<BTreeSet<_>>();
    if stack_set.len() != state.stack.len() {
        return Err(CastZoneRuntimeError::StateInvariantViolation);
    }
    for reference in &state.stack {
        let object = state
            .objects
            .get(reference)
            .ok_or(CastZoneRuntimeError::StateInvariantViolation)?;
        if object.zone != Zone::Stack {
            return Err(CastZoneRuntimeError::StateInvariantViolation);
        }
    }
    if state
        .objects
        .iter()
        .any(|(reference, object)| object.zone == Zone::Stack && !stack_set.contains(reference))
    {
        return Err(CastZoneRuntimeError::StateInvariantViolation);
    }

    for (cast_id, cast) in &state.casts {
        if cast_id != &cast.cast_id
            || cast.cast_id.0 == 0
            || cast.actor == 0
            || !state.players.contains_key(&cast.actor)
            || cast
                .pending_entry_counters
                .iter()
                .any(|(counter, amount)| counter.is_empty() || *amount == 0)
        {
            return Err(CastZoneRuntimeError::StateInvariantViolation);
        }
        if cast.status == CastLifecycleStatus::OnStack {
            let object = state
                .objects
                .get(&cast.stack_reference)
                .ok_or(CastZoneRuntimeError::StateInvariantViolation)?;
            if object.zone != Zone::Stack || !stack_set.contains(&cast.stack_reference) {
                return Err(CastZoneRuntimeError::StateInvariantViolation);
            }
        }
    }

    for (pending_id, pending) in &state.pending_delayed {
        if pending_id != &pending.id
            || pending.id == 0
            || pending.controller == 0
            || !state.players.contains_key(&pending.controller)
            || pending
                .affected_player
                .is_some_and(|player| !state.players.contains_key(&player))
        {
            return Err(CastZoneRuntimeError::StateInvariantViolation);
        }
    }

    let mut turn_ids = BTreeSet::new();
    for turn in &state.extra_turns {
        if turn.turn_id == 0
            || turn.player == 0
            || !state.players.contains_key(&turn.player)
            || !turn_ids.insert(turn.turn_id)
        {
            return Err(CastZoneRuntimeError::StateInvariantViolation);
        }
    }
    let mut active_adjustment_keys = BTreeSet::new();
    for adjustment in &state.active_cost_adjustments {
        if adjustment.controller == 0
            || !state.players.contains_key(&adjustment.controller)
            || !state.objects.contains_key(&adjustment.source)
            || !active_adjustment_keys.insert((
                adjustment.controller,
                adjustment.source,
                adjustment.program_digest.as_str(),
            ))
        {
            return Err(CastZoneRuntimeError::StateInvariantViolation);
        }
    }
    if state
        .applied_linked_cost_actions
        .iter()
        .any(|(cast_id, digest)| !state.casts.contains_key(cast_id) || digest.is_empty())
    {
        return Err(CastZoneRuntimeError::StateInvariantViolation);
    }

    let largest_object_id = state
        .objects
        .keys()
        .map(|reference| reference.object_id)
        .max()
        .unwrap_or(0);
    let largest_cast_id = state.casts.keys().map(|cast| cast.0).max().unwrap_or(0);
    let largest_pending_id = state.pending_delayed.keys().copied().max().unwrap_or(0);
    let largest_turn_id = state
        .extra_turns
        .iter()
        .map(|turn| turn.turn_id)
        .chain(std::iter::once(state.current_turn))
        .max()
        .unwrap_or(0);
    if state.next_object_id <= largest_object_id
        || state.next_cast_id <= largest_cast_id
        || state.next_pending_id <= largest_pending_id
        || state.next_turn_id <= largest_turn_id
    {
        return Err(CastZoneRuntimeError::StateInvariantViolation);
    }
    Ok(())
}

fn move_object(
    state: &mut CastZoneWorldState,
    old_reference: ObjectRef,
    destination: Zone,
    battlefield_or_stack_controller: Option<PlayerId>,
) -> Result<ObjectRef, CastZoneRuntimeError> {
    let mut object = state
        .objects
        .remove(&old_reference)
        .ok_or(CastZoneRuntimeError::MissingObject(old_reference))?;
    if object.reference != old_reference {
        state.objects.insert(old_reference, object);
        return Err(CastZoneRuntimeError::StaleObject(old_reference));
    }
    if let Some(controller) = battlefield_or_stack_controller
        && !state.players.contains_key(&controller)
    {
        state.objects.insert(old_reference, object);
        return Err(CastZoneRuntimeError::MissingPlayer(controller));
    }

    for library in state.libraries.values_mut() {
        library.retain(|reference| *reference != old_reference);
    }
    state.stack.retain(|reference| *reference != old_reference);
    state
        .active_cost_adjustments
        .retain(|adjustment| adjustment.source != old_reference);

    let next_incarnation = old_reference
        .incarnation_id
        .0
        .checked_add(1)
        .ok_or(CastZoneRuntimeError::IncarnationOverflow(old_reference))?;
    let new_reference = ObjectRef {
        object_id: old_reference.object_id,
        incarnation_id: IncarnationId(next_incarnation),
    };
    if state.objects.contains_key(&new_reference) {
        state.objects.insert(old_reference, object);
        return Err(CastZoneRuntimeError::StateInvariantViolation);
    }
    object.reference = new_reference;
    object.zone = destination;
    object.controller = if matches!(destination, Zone::Battlefield | Zone::Stack) {
        battlefield_or_stack_controller.unwrap_or(object.owner)
    } else {
        object.owner
    };
    state.objects.insert(new_reference, object);

    if destination == Zone::Library {
        let owner = state
            .objects
            .get(&new_reference)
            .ok_or(CastZoneRuntimeError::StateInvariantViolation)?
            .owner;
        state
            .libraries
            .entry(owner)
            .or_default()
            .push(new_reference);
    }
    Ok(new_reference)
}

fn schedule_delayed(
    program_digest: &str,
    template: &DelayedInstruction,
    controller: PlayerId,
    expected_object: Option<ObjectRef>,
    affected_player: Option<PlayerId>,
    state: &mut CastZoneWorldState,
) -> Result<PendingInstructionId, CastZoneRuntimeError> {
    if !state.players.contains_key(&controller) {
        return Err(CastZoneRuntimeError::MissingPlayer(controller));
    }
    let object_action = !matches!(&template.action, DelayedAction::LoseGame(_));
    if object_action && expected_object.is_none() {
        return Err(CastZoneRuntimeError::MissingLinkedCostEvidence);
    }
    if let Some(reference) = expected_object
        && !state.objects.contains_key(&reference)
    {
        return Err(CastZoneRuntimeError::MissingObject(reference));
    }
    if matches!(&template.action, DelayedAction::LoseGame(_)) {
        let player = affected_player.ok_or(CastZoneRuntimeError::MissingLinkedCostEvidence)?;
        if !state.players.contains_key(&player) {
            return Err(CastZoneRuntimeError::MissingPlayer(player));
        }
    } else if affected_player.is_some() {
        return Err(CastZoneRuntimeError::StateInvariantViolation);
    }

    let id = state.next_pending_id;
    state.next_pending_id = state
        .next_pending_id
        .checked_add(1)
        .ok_or(CastZoneRuntimeError::PendingIdOverflow)?;
    state.pending_delayed.insert(
        id,
        PendingDelayedInstruction {
            id,
            program_digest: program_digest.to_owned(),
            moment: template.moment,
            action: template.action.clone(),
            controller,
            expected_object,
            affected_player,
            created_turn: state.current_turn,
            extra_turn: None,
        },
    );
    Ok(id)
}

pub fn copy_spell_transactionally<S: CastZoneStateAdapter>(
    program: &CastZoneEnvelopeProgram,
    input: &SpellCopyExecutionInput,
    state: &mut S,
) -> Result<SpellCopyReceipt, CastZoneRuntimeError> {
    let (target_operand, may_choose_new_targets) = match program.kind() {
        CastZoneEnvelopeKind::CopySpell {
            target,
            may_choose_new_targets,
        } => (target, *may_choose_new_targets),
        _ => return Err(CastZoneRuntimeError::WrongProgramKind),
    };
    let mut staged = state.clone();
    validate_world(staged.cast_zone_world())?;
    require_execution_boundaries(staged.cast_zone_world())?;
    if !staged.cast_zone_world().spell_target_evidence_complete {
        return Err(CastZoneRuntimeError::IncompleteSpellTargetEvidence);
    }
    let world = staged.cast_zone_world_mut();
    if !world.players.contains_key(&input.controller) {
        return Err(CastZoneRuntimeError::MissingPlayer(input.controller));
    }
    let target = world
        .objects
        .get(&input.target)
        .ok_or(CastZoneRuntimeError::MissingObject(input.target))?
        .clone();
    if target.zone != Zone::Stack || !world.stack.contains(&input.target) {
        return Err(CastZoneRuntimeError::WrongZone {
            expected: BTreeSet::from([Zone::Stack]),
            actual: target.zone,
        });
    }
    let is_instant_or_sorcery = target.card_types.contains(&CardType::Instant)
        || target.card_types.contains(&CardType::Sorcery);
    let target_is_legal = match target_operand {
        ObjectOperand::TargetSpell => true,
        ObjectOperand::TargetInstantOrSorcerySpell => is_instant_or_sorcery,
        ObjectOperand::TargetSpellYouControl => {
            is_instant_or_sorcery && target.controller == input.controller
        }
        // A freestanding "copy it" clause has no executable antecedent and is
        // intentionally not compiled. Keep the runtime closed if a program is
        // ever constructed outside the compiler.
        _ => false,
    };
    if !target_is_legal {
        return Err(CastZoneRuntimeError::TargetConstraintViolation);
    }

    let object_id = world.next_object_id;
    world.next_object_id = world
        .next_object_id
        .checked_add(1)
        .ok_or(CastZoneRuntimeError::ObjectIdOverflow)?;
    let copy_reference = ObjectRef {
        object_id,
        incarnation_id: IncarnationId(1),
    };
    if world.objects.contains_key(&copy_reference) {
        return Err(CastZoneRuntimeError::StateInvariantViolation);
    }
    world.objects.insert(
        copy_reference,
        GameObject {
            reference: copy_reference,
            owner: target.owner,
            controller: input.controller,
            zone: Zone::Stack,
            card_types: target.card_types.clone(),
            printed_mana_cost: target.printed_mana_cost.clone(),
            counters: BTreeMap::new(),
            tapped: false,
            is_commander: false,
            is_token: false,
            is_copy: true,
            copied_from: Some(target.reference),
        },
    );
    world.stack.push(copy_reference);
    validate_world(staged.cast_zone_world())?;
    *state = staged;
    Ok(SpellCopyReceipt {
        program_digest: program.semantic_digest().to_owned(),
        original_spell: input.target,
        copy: copy_reference,
        is_cast: false,
        may_choose_new_targets,
        committed: true,
    })
}

pub fn schedule_delayed_transactionally<S: CastZoneStateAdapter>(
    program: &CastZoneEnvelopeProgram,
    input: &DelayedScheduleInput,
    state: &mut S,
) -> Result<DelayedScheduleReceipt, CastZoneRuntimeError> {
    let instruction = match program.kind() {
        CastZoneEnvelopeKind::Delayed(instruction) => instruction,
        _ => return Err(CastZoneRuntimeError::WrongProgramKind),
    };
    let mut staged = state.clone();
    validate_world(staged.cast_zone_world())?;
    require_execution_boundaries(staged.cast_zone_world())?;
    let (expected_object, affected_player) =
        bind_delayed_operands(instruction, input, staged.cast_zone_world())?;
    let pending_id = schedule_delayed(
        program.semantic_digest(),
        instruction,
        input.controller,
        expected_object,
        affected_player,
        staged.cast_zone_world_mut(),
    )?;
    validate_world(staged.cast_zone_world())?;
    *state = staged;
    Ok(DelayedScheduleReceipt {
        program_digest: program.semantic_digest().to_owned(),
        pending_id,
        expected_object,
        affected_player,
        committed: true,
    })
}

fn bind_delayed_operands(
    instruction: &DelayedInstruction,
    input: &DelayedScheduleInput,
    state: &CastZoneWorldState,
) -> Result<(Option<ObjectRef>, Option<PlayerId>), CastZoneRuntimeError> {
    if !state.players.contains_key(&input.controller) {
        return Err(CastZoneRuntimeError::MissingPlayer(input.controller));
    }
    match &instruction.action {
        DelayedAction::LoseGame(player) => {
            let affected = resolve_player_operand(
                *player,
                input.controller,
                input.expected_object,
                input.affected_player,
                state,
            )?;
            Ok((None, Some(affected)))
        }
        DelayedAction::Exile(object)
        | DelayedAction::ReturnToHand(object)
        | DelayedAction::ReturnToBattlefield(object)
        | DelayedAction::Sacrifice(object) => {
            let reference = resolve_runtime_object_operand(object, input.expected_object, state)?;
            if input.affected_player.is_some() {
                return Err(CastZoneRuntimeError::TargetConstraintViolation);
            }
            Ok((Some(reference), None))
        }
    }
}

fn resolve_runtime_object_operand(
    operand: &ObjectOperand,
    supplied: Option<ObjectRef>,
    state: &CastZoneWorldState,
) -> Result<ObjectRef, CastZoneRuntimeError> {
    if !matches!(
        operand,
        ObjectOperand::ThisCard
            | ObjectOperand::ThisSpell
            | ObjectOperand::It
            | ObjectOperand::ThatCard
            | ObjectOperand::ThatSpell
    ) {
        return Err(CastZoneRuntimeError::TargetConstraintViolation);
    }
    let reference = supplied.ok_or(CastZoneRuntimeError::MissingLinkedCostEvidence)?;
    state
        .objects
        .contains_key(&reference)
        .then_some(reference)
        .ok_or(CastZoneRuntimeError::MissingObject(reference))
}

fn resolve_player_operand(
    operand: PlayerOperand,
    controller: PlayerId,
    expected_object: Option<ObjectRef>,
    supplied: Option<PlayerId>,
    state: &CastZoneWorldState,
) -> Result<PlayerId, CastZoneRuntimeError> {
    let player = match operand {
        PlayerOperand::You => controller,
        PlayerOperand::ItsController => {
            let reference =
                expected_object.ok_or(CastZoneRuntimeError::MissingLinkedCostEvidence)?;
            state
                .objects
                .get(&reference)
                .ok_or(CastZoneRuntimeError::MissingObject(reference))?
                .controller
        }
        PlayerOperand::ItsOwner => {
            let reference =
                expected_object.ok_or(CastZoneRuntimeError::MissingLinkedCostEvidence)?;
            state
                .objects
                .get(&reference)
                .ok_or(CastZoneRuntimeError::MissingObject(reference))?
                .owner
        }
        PlayerOperand::ThatPlayer | PlayerOperand::TargetPlayer | PlayerOperand::TargetOpponent => {
            supplied.ok_or(CastZoneRuntimeError::MissingLinkedCostEvidence)?
        }
        PlayerOperand::EachPlayer => {
            return Err(CastZoneRuntimeError::TargetConstraintViolation);
        }
    };
    if !state.players.contains_key(&player) {
        return Err(CastZoneRuntimeError::MissingPlayer(player));
    }
    if operand == PlayerOperand::TargetOpponent && player == controller {
        return Err(CastZoneRuntimeError::TargetConstraintViolation);
    }
    Ok(player)
}

pub fn resolve_delayed_transactionally<S: CastZoneStateAdapter>(
    pending_id: PendingInstructionId,
    moment: DelayedMoment,
    state: &mut S,
) -> Result<DelayedResolutionReceipt, CastZoneRuntimeError> {
    let mut staged = state.clone();
    validate_world(staged.cast_zone_world())?;
    require_execution_boundaries(staged.cast_zone_world())?;
    let receipt = resolve_delayed_in_world(pending_id, moment, staged.cast_zone_world_mut())?;
    validate_world(staged.cast_zone_world())?;
    *state = staged;
    Ok(receipt)
}

fn resolve_delayed_in_world(
    pending_id: PendingInstructionId,
    moment: DelayedMoment,
    state: &mut CastZoneWorldState,
) -> Result<DelayedResolutionReceipt, CastZoneRuntimeError> {
    let pending = state
        .pending_delayed
        .get(&pending_id)
        .ok_or(CastZoneRuntimeError::MissingPendingInstruction(pending_id))?
        .clone();
    if pending.moment != moment {
        return Err(CastZoneRuntimeError::WrongDelayedMoment);
    }
    validate_delayed_event_window(&pending, state)?;

    let object_before = pending.expected_object;
    let object_after;
    let mut affected_player = pending.affected_player;
    let mut applied = false;
    match &pending.action {
        DelayedAction::LoseGame(_) => {
            let player = pending
                .affected_player
                .ok_or(CastZoneRuntimeError::MissingLinkedCostEvidence)?;
            let player_state = state
                .players
                .get_mut(&player)
                .ok_or(CastZoneRuntimeError::MissingPlayer(player))?;
            player_state.lost_game = true;
            object_after = None;
            affected_player = Some(player);
            applied = true;
        }
        DelayedAction::Exile(_) => {
            if let Some(reference) = pending.expected_object
                && state.objects.contains_key(&reference)
            {
                let moved = move_object(state, reference, Zone::Exile, None)?;
                object_after = Some(moved);
                applied = true;
            } else {
                object_after = None;
            }
        }
        DelayedAction::ReturnToHand(_) => {
            if let Some(reference) = pending.expected_object
                && state.objects.contains_key(&reference)
            {
                let moved = move_object(state, reference, Zone::Hand, None)?;
                object_after = Some(moved);
                applied = true;
            } else {
                object_after = None;
            }
        }
        DelayedAction::ReturnToBattlefield(_) => {
            if let Some(reference) = pending.expected_object
                && let Some(object) = state.objects.get(&reference)
            {
                if !object.is_permanent() || object.zone == Zone::Battlefield {
                    return Err(CastZoneRuntimeError::TargetConstraintViolation);
                }
                let owner = object.owner;
                let moved = move_object(state, reference, Zone::Battlefield, Some(owner))?;
                object_after = Some(moved);
                applied = true;
            } else {
                object_after = None;
            }
        }
        DelayedAction::Sacrifice(_) => {
            if let Some(reference) = pending.expected_object
                && let Some(object) = state.objects.get(&reference)
            {
                if object.zone != Zone::Battlefield
                    || object.controller != pending.controller
                    || !object.is_permanent()
                {
                    return Err(CastZoneRuntimeError::TargetConstraintViolation);
                }
                let moved = move_object(state, reference, Zone::Graveyard, None)?;
                object_after = Some(moved);
                applied = true;
            } else {
                object_after = None;
            }
        }
    }
    state.pending_delayed.remove(&pending_id);
    if let (Some(before), Some(after)) = (object_before, object_after)
        && before != after
    {
        update_resolved_lifecycle_reference(before, after, state);
    }
    Ok(DelayedResolutionReceipt {
        pending_id,
        program_digest: pending.program_digest,
        object_before,
        object_after,
        affected_player,
        applied,
        committed: true,
    })
}

fn validate_delayed_event_window(
    pending: &PendingDelayedInstruction,
    state: &CastZoneWorldState,
) -> Result<(), CastZoneRuntimeError> {
    let valid = match pending.moment {
        DelayedMoment::BeginningOfYourNextEndStep | DelayedMoment::BeginningOfYourNextUpkeep => {
            state.active_player == pending.controller
        }
        DelayedMoment::ThatTurnsEndStep => pending.extra_turn == Some(state.current_turn),
        DelayedMoment::BeginningOfNextEndStep
        | DelayedMoment::BeginningOfNextUpkeep
        | DelayedMoment::EndOfTurn
        | DelayedMoment::EndOfCombat => true,
    };
    valid.then_some(()).ok_or(CastZoneRuntimeError::WrongTurn)
}

pub fn create_extra_turn_transactionally<S: CastZoneStateAdapter>(
    program: &CastZoneEnvelopeProgram,
    input: &ExtraTurnExecutionInput,
    state: &mut S,
) -> Result<ExtraTurnReceipt, CastZoneRuntimeError> {
    let (player_operand, delayed_loss) = match program.kind() {
        CastZoneEnvelopeKind::ExtraTurnWithDelayedLoss {
            player,
            delayed_loss,
        } => (*player, delayed_loss),
        _ => return Err(CastZoneRuntimeError::WrongProgramKind),
    };
    let mut staged = state.clone();
    validate_world(staged.cast_zone_world())?;
    require_execution_boundaries(staged.cast_zone_world())?;
    let player = resolve_player_operand(
        player_operand,
        input.controller,
        None,
        input.affected_player,
        staged.cast_zone_world(),
    )?;
    let world = staged.cast_zone_world_mut();
    let turn_id = world.next_turn_id;
    world.next_turn_id = world
        .next_turn_id
        .checked_add(1)
        .ok_or(CastZoneRuntimeError::TurnIdOverflow)?;
    world.extra_turns.push(ExtraTurn {
        turn_id,
        player,
        after_turn: world.current_turn,
        source_program_digest: program.semantic_digest().to_owned(),
    });
    let delayed_loss_id = schedule_delayed(
        program.semantic_digest(),
        delayed_loss,
        input.controller,
        None,
        Some(player),
        world,
    )?;
    world
        .pending_delayed
        .get_mut(&delayed_loss_id)
        .ok_or(CastZoneRuntimeError::StateInvariantViolation)?
        .extra_turn = Some(turn_id);
    validate_world(staged.cast_zone_world())?;
    *state = staged;
    Ok(ExtraTurnReceipt {
        program_digest: program.semantic_digest().to_owned(),
        turn_id,
        player,
        delayed_loss_id,
        committed: true,
    })
}

pub fn execute_linked_cost_paid_transactionally<S: CastZoneStateAdapter>(
    program: &CastZoneEnvelopeProgram,
    cast_id: CastId,
    state: &mut S,
) -> Result<LinkedCostReceipt, CastZoneRuntimeError> {
    let envelope = match program.kind() {
        CastZoneEnvelopeKind::LinkedCostPaid(envelope) => envelope,
        _ => return Err(CastZoneRuntimeError::WrongProgramKind),
    };
    let mut staged = state.clone();
    validate_world(staged.cast_zone_world())?;
    require_execution_boundaries(staged.cast_zone_world())?;
    let receipt = execute_linked_cost_paid_in_world(
        program,
        envelope,
        cast_id,
        staged.cast_zone_world_mut(),
    )?;
    validate_world(staged.cast_zone_world())?;
    *state = staged;
    Ok(receipt)
}

fn execute_linked_cost_paid_in_world(
    program: &CastZoneEnvelopeProgram,
    envelope: &LinkedCostPaidEnvelope,
    cast_id: CastId,
    state: &mut CastZoneWorldState,
) -> Result<LinkedCostReceipt, CastZoneRuntimeError> {
    let lifecycle = state
        .casts
        .get(&cast_id)
        .ok_or(CastZoneRuntimeError::MissingCast(cast_id))?
        .clone();
    let required_status = if matches!(
        &envelope.action,
        LinkedResolutionAction::EnterWithCounter { .. }
    ) {
        CastLifecycleStatus::OnStack
    } else {
        CastLifecycleStatus::Resolved
    };
    if lifecycle.status != required_status {
        return Err(CastZoneRuntimeError::WrongCastStatus(lifecycle.status));
    }
    if !paid_cost_reference_satisfied(&envelope.cost_reference, &lifecycle.paid_costs) {
        return Err(CastZoneRuntimeError::MissingLinkedCostEvidence);
    }
    let application_key = (cast_id, program.semantic_digest().to_owned());
    if state.applied_linked_cost_actions.contains(&application_key) {
        return Err(CastZoneRuntimeError::LinkedCostActionAlreadyApplied);
    }

    let object_after = match &envelope.action {
        LinkedResolutionAction::DrawCard => {
            if !state.hidden_zone_evidence_complete {
                return Err(CastZoneRuntimeError::IncompleteHiddenZoneEvidence);
            }
            Some(draw_top_card(lifecycle.actor, state)?)
        }
        LinkedResolutionAction::CreateTokenCopy => {
            let source_ref = lifecycle
                .resolved_reference
                .ok_or(CastZoneRuntimeError::UnsupportedLinkedActionState)?;
            Some(create_token_copy(source_ref, lifecycle.actor, state)?)
        }
        LinkedResolutionAction::EnterWithCounter { counter, amount } => {
            if *amount == 0 {
                return Err(CastZoneRuntimeError::UnsupportedLinkedActionState);
            }
            let cast = state
                .casts
                .get_mut(&cast_id)
                .ok_or(CastZoneRuntimeError::MissingCast(cast_id))?;
            cast.pending_entry_counters.push((counter.clone(), *amount));
            Some(lifecycle.stack_reference)
        }
        LinkedResolutionAction::ReturnToOwnersHand(object) => {
            let reference = linked_object_reference(object, &lifecycle)?;
            let moved = move_object(state, reference, Zone::Hand, None)?;
            update_resolved_lifecycle_reference(reference, moved, state);
            mark_lifecycle_left_expected_zone(cast_id, state)?;
            Some(moved)
        }
        LinkedResolutionAction::Exile(object) => {
            let reference = linked_object_reference(object, &lifecycle)?;
            let moved = move_object(state, reference, Zone::Exile, None)?;
            update_resolved_lifecycle_reference(reference, moved, state);
            mark_lifecycle_left_expected_zone(cast_id, state)?;
            Some(moved)
        }
        LinkedResolutionAction::Sacrifice(object) => {
            let reference = linked_object_reference(object, &lifecycle)?;
            let object_state = state
                .objects
                .get(&reference)
                .ok_or(CastZoneRuntimeError::MissingObject(reference))?;
            if object_state.zone != Zone::Battlefield
                || object_state.controller != lifecycle.actor
                || !object_state.is_permanent()
            {
                return Err(CastZoneRuntimeError::UnsupportedLinkedActionState);
            }
            let moved = move_object(state, reference, Zone::Graveyard, None)?;
            update_resolved_lifecycle_reference(reference, moved, state);
            mark_lifecycle_left_expected_zone(cast_id, state)?;
            Some(moved)
        }
    };
    state.applied_linked_cost_actions.insert(application_key);
    Ok(LinkedCostReceipt {
        program_digest: program.semantic_digest().to_owned(),
        cast_id,
        action: envelope.action.clone(),
        object_after,
        committed: true,
    })
}

fn paid_cost_reference_satisfied(reference: &PaidCostReference, ledger: &PaidCostLedger) -> bool {
    if ledger.externally_verified_paid_costs.contains(reference) {
        return true;
    }
    match reference {
        PaidCostReference::Kicker => false,
        PaidCostReference::AdditionalCost => ledger.additional_cost_paid,
        PaidCostReference::AlternativeCost => ledger.alternative_cost_paid,
        PaidCostReference::Mana(cost) => ledger
            .paid_atoms
            .iter()
            .any(|atom| matches!(atom, CostAtom::Mana(paid) if paid == cost)),
        PaidCostReference::Life(amount) => ledger
            .paid_atoms
            .iter()
            .any(|atom| matches!(atom, CostAtom::PayLife(paid) if paid == amount)),
    }
}

fn linked_object_reference(
    operand: &ObjectOperand,
    lifecycle: &CastLifecycle,
) -> Result<ObjectRef, CastZoneRuntimeError> {
    if !matches!(
        operand,
        ObjectOperand::It
            | ObjectOperand::ThisCard
            | ObjectOperand::ThisSpell
            | ObjectOperand::ThatCard
            | ObjectOperand::ThatSpell
    ) {
        return Err(CastZoneRuntimeError::UnsupportedLinkedActionState);
    }
    lifecycle
        .resolved_reference
        .ok_or(CastZoneRuntimeError::UnsupportedLinkedActionState)
}

fn draw_top_card(
    player: PlayerId,
    state: &mut CastZoneWorldState,
) -> Result<ObjectRef, CastZoneRuntimeError> {
    let reference = state
        .libraries
        .get_mut(&player)
        .and_then(Vec::pop)
        .ok_or(CastZoneRuntimeError::MissingLibraryCard)?;
    move_object(state, reference, Zone::Hand, None)
}

fn create_token_copy(
    source_ref: ObjectRef,
    controller: PlayerId,
    state: &mut CastZoneWorldState,
) -> Result<ObjectRef, CastZoneRuntimeError> {
    let source = state
        .objects
        .get(&source_ref)
        .ok_or(CastZoneRuntimeError::MissingObject(source_ref))?
        .clone();
    if source.zone != Zone::Battlefield || !source.is_permanent() {
        return Err(CastZoneRuntimeError::UnsupportedLinkedActionState);
    }
    let object_id = state.next_object_id;
    state.next_object_id = state
        .next_object_id
        .checked_add(1)
        .ok_or(CastZoneRuntimeError::ObjectIdOverflow)?;
    let reference = ObjectRef {
        object_id,
        incarnation_id: IncarnationId(1),
    };
    state.objects.insert(
        reference,
        GameObject {
            reference,
            owner: controller,
            controller,
            zone: Zone::Battlefield,
            card_types: source.card_types,
            printed_mana_cost: source.printed_mana_cost,
            counters: BTreeMap::new(),
            tapped: false,
            is_commander: false,
            is_token: true,
            is_copy: true,
            copied_from: Some(source_ref),
        },
    );
    Ok(reference)
}

fn update_resolved_lifecycle_reference(
    before: ObjectRef,
    after: ObjectRef,
    state: &mut CastZoneWorldState,
) {
    for lifecycle in state.casts.values_mut() {
        if lifecycle.resolved_reference == Some(before) {
            lifecycle.resolved_reference = Some(after);
        }
    }
}

fn mark_lifecycle_left_expected_zone(
    cast_id: CastId,
    state: &mut CastZoneWorldState,
) -> Result<(), CastZoneRuntimeError> {
    state
        .casts
        .get_mut(&cast_id)
        .ok_or(CastZoneRuntimeError::MissingCast(cast_id))?
        .status = CastLifecycleStatus::LeftExpectedZone;
    Ok(())
}
