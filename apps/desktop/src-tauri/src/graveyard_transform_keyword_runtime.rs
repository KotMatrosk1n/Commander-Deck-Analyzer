//! Content keyed runtime contracts for Disturb, Soulshift, and Craft.
//!
//! Recognition and execution are intentionally separate from production
//! coverage. The programs in this module preserve the game objects, costs,
//! targets, choices, zone changes, and linked exile state needed by these
//! mechanics. The production adapter stays disconnected until the main
//! simulation can provide the same complete evidence.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use sha2::{Digest, Sha256};

pub const GRAVEYARD_TRANSFORM_KEYWORD_COMPILER_VERSION: &str =
    "graveyard-transform-keyword-compiler-0.1";
pub const GRAVEYARD_TRANSFORM_KEYWORD_RUNTIME_VERSION: &str =
    "graveyard-transform-keyword-runtime-0.1";
pub const GRAVEYARD_TRANSFORM_KEYWORD_RULES_CONTEXT_VERSION: &str =
    "magic-comprehensive-rules-2026-06-19:118,400.7,601.2,603,608.2,614,702.46,702.146,702.167,712";

pub const fn graveyard_transform_keyword_production_adapter_connected() -> bool {
    false
}

/// Canonicalize only self references needed by the graveyard/transform
/// lifecycle compiler. Snapshot coordinates, hashes, timestamps, and database
/// identities are deliberately absent.
///
/// The face name is used only to prove that a printed proper-name reference is
/// a self reference. It is replaced before the text enters semantic identity.
pub fn normalize_face_oracle_text_for_semantics(oracle_text: &str, face_name: &str) -> String {
    oracle_text
        .lines()
        .map(|line| normalize_face_oracle_line_for_semantics(line, face_name))
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_face_oracle_line_for_semantics(source: &str, face_name: &str) -> String {
    let trimmed = source.trim();
    let suffix = " would be put into a graveyard from anywhere, exile it instead.";
    if let Some(subject) = trimmed
        .strip_prefix("If ")
        .and_then(|rest| rest.strip_suffix(suffix))
    {
        let unprefixed = face_name.strip_prefix("A-").unwrap_or(face_name);
        let self_subject = subject == face_name
            || subject == unprefixed
            || subject.starts_with("this ")
            || face_name
                .strip_prefix(subject)
                .is_some_and(|remainder| remainder.starts_with(','));
        if self_subject {
            return "If this object would be put into a graveyard from anywhere, exile it instead."
                .into();
        }
    }
    let mut normalized = trimmed.replace(face_name, "this object");
    if let Some(unprefixed) = face_name.strip_prefix("A-") {
        normalized = normalized.replace(unprefixed, "this object");
    }
    normalized
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncarnationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectRef {
    pub object_id: ObjectId,
    pub incarnation_id: IncarnationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaUnitId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingSoulshiftTriggerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PendingCraftAbilityId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
    Stack,
    Command,
    OutsideGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FaceId {
    Front,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CardLayout {
    Normal,
    Transform,
}

impl CardLayout {
    fn stable_id(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Transform => "transform",
        }
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
    fn stable_id(self) -> &'static str {
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
}

impl ManaSymbol {
    fn stable_id(self) -> String {
        match self {
            Self::Generic(amount) => format!("generic/{amount}"),
            Self::White => "white".into(),
            Self::Blue => "blue".into(),
            Self::Black => "black".into(),
            Self::Red => "red".into(),
            Self::Green => "green".into(),
            Self::Colorless => "colorless".into(),
            Self::Snow => "snow".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaCost {
    pub exact: String,
    pub symbols: Vec<ManaSymbol>,
}

impl ManaCost {
    fn stable_id(&self) -> String {
        self.symbols
            .iter()
            .map(|symbol| symbol.stable_id())
            .collect::<Vec<_>>()
            .join(",")
    }

    fn required_units(&self) -> Option<usize> {
        self.symbols.iter().try_fold(0usize, |total, symbol| {
            let amount = match symbol {
                ManaSymbol::Generic(amount) => usize::try_from(*amount).ok()?,
                _ => 1,
            };
            total.checked_add(amount)
        })
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
    fn stable_id(self) -> &'static str {
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
pub struct FaceSemanticContext {
    /// This text has source names replaced by a stable self reference.
    pub normalized_oracle_text: String,
    pub type_line: String,
    pub mana_cost: String,
    pub colors: BTreeSet<ManaColor>,
    pub color_indicator: BTreeSet<ManaColor>,
    pub root_mana_value: u32,
    pub power: Option<String>,
    pub toughness: Option<String>,
    pub loyalty: Option<String>,
    pub defense: Option<String>,
}

impl FaceSemanticContext {
    fn stable_id(&self) -> String {
        let colors = self
            .colors
            .iter()
            .map(|color| color.stable_id())
            .collect::<Vec<_>>()
            .join(",");
        let color_indicator = self
            .color_indicator
            .iter()
            .map(|color| color.stable_id())
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "oracle={};type={};mana={};colors={};indicator={};root-mv={};power={:?};toughness={:?};loyalty={:?};defense={:?}",
            self.normalized_oracle_text,
            canonical_space(&self.type_line),
            self.mana_cost,
            colors,
            color_indicator,
            self.root_mana_value,
            self.power,
            self.toughness,
            self.loyalty,
            self.defense
        )
    }

    fn card_types(&self) -> BTreeSet<CardType> {
        card_types_from_type_line(&self.type_line)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformSemanticContext {
    pub layout: CardLayout,
    pub keyword_face: FaceId,
    pub front: FaceSemanticContext,
    pub back: FaceSemanticContext,
}

impl TransformSemanticContext {
    fn stable_id(&self) -> String {
        format!(
            "layout={};keyword-face={:?};front=[{}];back=[{}]",
            self.layout.stable_id(),
            self.keyword_face,
            self.front.stable_id(),
            self.back.stable_id()
        )
    }

    fn is_complete_front_keyword_transform(&self) -> bool {
        self.layout == CardLayout::Transform
            && self.keyword_face == FaceId::Front
            && !self.front.type_line.trim().is_empty()
            && !self.back.type_line.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSemanticContext {
    Transform(TransformSemanticContext),
    SingleFace {
        layout: CardLayout,
        type_line: String,
        normalized_oracle_text: String,
    },
}

impl SourceSemanticContext {
    fn stable_id(&self) -> String {
        match self {
            Self::Transform(context) => format!("transform/{}", context.stable_id()),
            Self::SingleFace {
                layout,
                type_line,
                normalized_oracle_text,
            } => format!(
                "single/layout={};type={};oracle={}",
                layout.stable_id(),
                canonical_space(type_line),
                normalized_oracle_text
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialLocationScope {
    ControlledBattlefieldOrYourGraveyard,
    YourGraveyardOnly,
}

impl MaterialLocationScope {
    fn stable_id(self) -> &'static str {
        match self {
            Self::ControlledBattlefieldOrYourGraveyard => "battlefield-or-graveyard",
            Self::YourGraveyardOnly => "graveyard-only",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftMaterialRequirement {
    Exact {
        count: usize,
        filter: MaterialFilter,
        scope: MaterialLocationScope,
    },
    AtLeast {
        count: usize,
        filter: MaterialFilter,
        scope: MaterialLocationScope,
    },
    TwoSharingCardType,
    FourDistinctCreatureSubtypes {
        required_subtypes: BTreeSet<String>,
    },
}

impl CraftMaterialRequirement {
    fn stable_id(&self) -> String {
        match self {
            Self::Exact {
                count,
                filter,
                scope,
            } => format!("exact/{count}/{}/{}", filter.stable_id(), scope.stable_id()),
            Self::AtLeast {
                count,
                filter,
                scope,
            } => format!(
                "at-least/{count}/{}/{}",
                filter.stable_id(),
                scope.stable_id()
            ),
            Self::TwoSharingCardType => "two-sharing-card-type".into(),
            Self::FourDistinctCreatureSubtypes { required_subtypes } => format!(
                "four-distinct-subtypes/{}",
                required_subtypes
                    .iter()
                    .map(|value| canonical_word(value))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialFilter {
    AnyCard,
    CardType(CardType),
    Subtype(String),
    NonlandWithActivatedAbility,
    RedInstantOrSorcery,
}

impl MaterialFilter {
    fn stable_id(&self) -> String {
        match self {
            Self::AnyCard => "any-card".into(),
            Self::CardType(card_type) => format!("card-type/{}", card_type.stable_id()),
            Self::Subtype(subtype) => format!("subtype/{}", canonical_word(subtype)),
            Self::NonlandWithActivatedAbility => "nonland-with-activated-ability".into(),
            Self::RedInstantOrSorcery => "red-instant-or-sorcery".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisturbProgram {
    pub alternative_cost: ManaCost,
    pub cast_only_from_owners_graveyard: bool,
    pub put_on_stack_back_face_up: bool,
    pub resolving_spell_enters_back_face_up: bool,
    pub back_face_graveyard_move_is_replaced_with_exile: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoulshiftProgram {
    pub maximum_mana_value: u32,
    pub uses_last_known_battlefield_ability: bool,
    pub targets_spirit_card_in_ability_controllers_graveyard: bool,
    pub each_instance_triggers_independently: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftProgram {
    pub activation_cost: ManaCost,
    pub material_requirement: CraftMaterialRequirement,
    pub exile_source_as_cost: bool,
    pub sorcery_timing_only: bool,
    pub return_transformed_under_owner_control: bool,
    pub links_material_cards_remaining_in_exile: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraveyardTransformKeywordKind {
    Disturb(DisturbProgram),
    Soulshift(SoulshiftProgram),
    Craft(CraftProgram),
}

impl GraveyardTransformKeywordKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disturb(_) => "Disturb",
            Self::Soulshift(_) => "Soulshift",
            Self::Craft(_) => "Craft",
        }
    }

    fn stable_id(&self) -> String {
        match self {
            Self::Disturb(program) => format!(
                "disturb;cost={};owner-graveyard={};stack-back={};resolve-back={};grave-exile={}",
                program.alternative_cost.stable_id(),
                program.cast_only_from_owners_graveyard,
                program.put_on_stack_back_face_up,
                program.resolving_spell_enters_back_face_up,
                program.back_face_graveyard_move_is_replaced_with_exile
            ),
            Self::Soulshift(program) => format!(
                "soulshift;maximum={};lki={};controller-graveyard={};independent={}",
                program.maximum_mana_value,
                program.uses_last_known_battlefield_ability,
                program.targets_spirit_card_in_ability_controllers_graveyard,
                program.each_instance_triggers_independently
            ),
            Self::Craft(program) => format!(
                "craft;cost={};materials={};exile-source={};sorcery={};owner-control={};linked-exile={}",
                program.activation_cost.stable_id(),
                program.material_requirement.stable_id(),
                program.exile_source_as_cost,
                program.sorcery_timing_only,
                program.return_transformed_under_owner_control,
                program.links_material_cards_remaining_in_exile
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraveyardTransformKeywordProgram {
    exact_source: String,
    source_context: SourceSemanticContext,
    semantic_digest: String,
    kind: GraveyardTransformKeywordKind,
}

impl GraveyardTransformKeywordProgram {
    pub fn exact_source(&self) -> &str {
        &self.exact_source
    }

    pub fn source_context(&self) -> &SourceSemanticContext {
        &self.source_context
    }

    pub fn semantic_digest(&self) -> &str {
        &self.semantic_digest
    }

    pub fn kind(&self) -> &GraveyardTransformKeywordKind {
        &self.kind
    }

    pub const fn production_adapter_connected(&self) -> bool {
        graveyard_transform_keyword_production_adapter_connected()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotCandidateClass {
    SupportedFamily,
    ReminderlessOracleBoundary,
    CompoundOrUnsupportedContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EarlierOwnedAssociatedClause {
    ObjectStateSelfGraveyardReplacement,
}

pub fn classify_earlier_owned_associated_clause(
    normalized_source: &str,
) -> Option<EarlierOwnedAssociatedClause> {
    back_has_complete_graveyard_exile_replacement(normalized_source)
        .then_some(EarlierOwnedAssociatedClause::ObjectStateSelfGraveyardReplacement)
}

pub fn classify_snapshot_candidate(
    exact_source: &str,
    source_context: &SourceSemanticContext,
) -> Option<SnapshotCandidateClass> {
    if !["Disturb", "Soulshift", "Craft"]
        .iter()
        .any(|keyword| exact_source.starts_with(keyword))
    {
        return None;
    }
    if compile_graveyard_transform_keyword_program(exact_source, source_context).is_some() {
        return Some(SnapshotCandidateClass::SupportedFamily);
    }
    if split_trailing_parenthetical(exact_source)
        .and_then(|(_, reminder)| reminder)
        .is_none()
    {
        return Some(SnapshotCandidateClass::ReminderlessOracleBoundary);
    }
    Some(SnapshotCandidateClass::CompoundOrUnsupportedContext)
}

/// Returns a complete program or `None`.
///
/// Snapshot coordinates, card names, Oracle IDs, snapshot hashes, and update
/// timestamps are intentionally not inputs.
pub fn compile_graveyard_transform_keyword_program(
    exact_source: &str,
    source_context: &SourceSemanticContext,
) -> Option<GraveyardTransformKeywordProgram> {
    if !complete_single_line(exact_source) {
        return None;
    }
    let kind = parse_disturb(exact_source, source_context)
        .or_else(|| parse_soulshift(exact_source, source_context))
        .or_else(|| parse_craft(exact_source, source_context))?;
    let semantic_digest = semantic_digest(exact_source, source_context, &kind);
    Some(GraveyardTransformKeywordProgram {
        exact_source: exact_source.to_owned(),
        source_context: source_context.clone(),
        semantic_digest,
        kind,
    })
}

fn parse_disturb(
    exact_source: &str,
    source_context: &SourceSemanticContext,
) -> Option<GraveyardTransformKeywordKind> {
    let SourceSemanticContext::Transform(context) = source_context else {
        return None;
    };
    if !context.is_complete_front_keyword_transform()
        || !back_has_complete_graveyard_exile_replacement(&context.back.normalized_oracle_text)
    {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    if reminder? != "You may cast this card from your graveyard transformed for its disturb cost." {
        return None;
    }
    let cost = parse_mana_cost(core.strip_prefix("Disturb ")?)?;
    Some(GraveyardTransformKeywordKind::Disturb(DisturbProgram {
        alternative_cost: cost,
        cast_only_from_owners_graveyard: true,
        put_on_stack_back_face_up: true,
        resolving_spell_enters_back_face_up: true,
        back_face_graveyard_move_is_replaced_with_exile: true,
    }))
}

fn parse_soulshift(
    exact_source: &str,
    source_context: &SourceSemanticContext,
) -> Option<GraveyardTransformKeywordKind> {
    let SourceSemanticContext::SingleFace {
        type_line,
        normalized_oracle_text,
        ..
    } = source_context
    else {
        return None;
    };
    if normalized_oracle_text.trim().is_empty()
        || !card_types_from_type_line(type_line).contains(&CardType::Creature)
    {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    let maximum_mana_value = core.strip_prefix("Soulshift ")?.parse::<u32>().ok()?;
    let expected = format!(
        "When this creature dies, you may return target Spirit card with mana value {maximum_mana_value} or less from your graveyard to your hand."
    );
    if reminder? != expected {
        return None;
    }
    Some(GraveyardTransformKeywordKind::Soulshift(SoulshiftProgram {
        maximum_mana_value,
        uses_last_known_battlefield_ability: true,
        targets_spirit_card_in_ability_controllers_graveyard: true,
        each_instance_triggers_independently: true,
    }))
}

fn parse_craft(
    exact_source: &str,
    source_context: &SourceSemanticContext,
) -> Option<GraveyardTransformKeywordKind> {
    let SourceSemanticContext::Transform(context) = source_context else {
        return None;
    };
    if !context.is_complete_front_keyword_transform()
        || !context.front.card_types().contains(&CardType::Artifact)
    {
        return None;
    }
    let (core, reminder) = split_trailing_parenthetical(exact_source)?;
    let reminder = reminder?;
    let core = core.strip_prefix("Craft with ")?;
    let cost_start = core.rfind(" {")? + 1;
    let material_description = core[..cost_start - 1].trim();
    let cost = parse_mana_cost(&core[cost_start..])?;
    let material_requirement = parse_craft_material_requirement(material_description)?;
    let expected_instruction = craft_reminder_instruction(material_description);
    let expected = format!(
        "{}, Exile this artifact, {}: Return this card transformed under its owner's control. Craft only as a sorcery.",
        cost.exact, expected_instruction
    );
    if reminder != expected {
        return None;
    }
    Some(GraveyardTransformKeywordKind::Craft(CraftProgram {
        activation_cost: cost,
        material_requirement,
        exile_source_as_cost: true,
        sorcery_timing_only: true,
        return_transformed_under_owner_control: true,
        links_material_cards_remaining_in_exile: true,
    }))
}

fn parse_craft_material_requirement(source: &str) -> Option<CraftMaterialRequirement> {
    let battlefield_or_graveyard = MaterialLocationScope::ControlledBattlefieldOrYourGraveyard;
    let graveyard_only = MaterialLocationScope::YourGraveyardOnly;
    match source {
        "two that share a card type" => Some(CraftMaterialRequirement::TwoSharingCardType),
        "Cave" => Some(CraftMaterialRequirement::Exact {
            count: 1,
            filter: MaterialFilter::Subtype("Cave".into()),
            scope: battlefield_or_graveyard,
        }),
        "artifact" => Some(CraftMaterialRequirement::Exact {
            count: 1,
            filter: MaterialFilter::CardType(CardType::Artifact),
            scope: battlefield_or_graveyard,
        }),
        "Island" => Some(CraftMaterialRequirement::Exact {
            count: 1,
            filter: MaterialFilter::Subtype("Island".into()),
            scope: battlefield_or_graveyard,
        }),
        "two creatures" => Some(CraftMaterialRequirement::Exact {
            count: 2,
            filter: MaterialFilter::CardType(CardType::Creature),
            scope: battlefield_or_graveyard,
        }),
        "creature" => Some(CraftMaterialRequirement::Exact {
            count: 1,
            filter: MaterialFilter::CardType(CardType::Creature),
            scope: battlefield_or_graveyard,
        }),
        "one or more creatures" => Some(CraftMaterialRequirement::AtLeast {
            count: 1,
            filter: MaterialFilter::CardType(CardType::Creature),
            scope: battlefield_or_graveyard,
        }),
        "a Dinosaur, a Merfolk, a Pirate, and a Vampire" => {
            Some(CraftMaterialRequirement::FourDistinctCreatureSubtypes {
                required_subtypes: ["Dinosaur", "Merfolk", "Pirate", "Vampire"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
            })
        }
        "four or more nonlands with activated abilities" => {
            Some(CraftMaterialRequirement::AtLeast {
                count: 4,
                filter: MaterialFilter::NonlandWithActivatedAbility,
                scope: battlefield_or_graveyard,
            })
        }
        "one or more" => Some(CraftMaterialRequirement::AtLeast {
            count: 1,
            filter: MaterialFilter::AnyCard,
            scope: battlefield_or_graveyard,
        }),
        "six artifacts" => Some(CraftMaterialRequirement::Exact {
            count: 6,
            filter: MaterialFilter::CardType(CardType::Artifact),
            scope: battlefield_or_graveyard,
        }),
        "four or more red instant and/or sorcery cards" => {
            Some(CraftMaterialRequirement::AtLeast {
                count: 4,
                filter: MaterialFilter::RedInstantOrSorcery,
                scope: graveyard_only,
            })
        }
        "one or more Dinosaurs" => Some(CraftMaterialRequirement::AtLeast {
            count: 1,
            filter: MaterialFilter::Subtype("Dinosaur".into()),
            scope: battlefield_or_graveyard,
        }),
        _ => None,
    }
}

fn craft_reminder_instruction(material_description: &str) -> String {
    match material_description {
        "two that share a card type" => {
            "Exile the two from among other permanents you control and/or cards from your graveyard"
        }
        "Cave" => "Exile a Cave you control or a Cave card from your graveyard",
        "artifact" => {
            "Exile another artifact you control or an artifact card from your graveyard"
        }
        "Island" => "Exile an Island you control or an Island card from your graveyard",
        "two creatures" => {
            "Exile the two from among creatures you control and/or creature cards in your graveyard"
        }
        "creature" => "Exile a creature you control or a creature card from your graveyard",
        "one or more creatures" => {
            "Exile one or more creatures you control and/or creature cards from your graveyard"
        }
        "a Dinosaur, a Merfolk, a Pirate, and a Vampire" => {
            "Exile the four from among permanents you control and/or cards in your graveyard"
        }
        "four or more nonlands with activated abilities" => {
            "Exile the four or more from among other permanents you control and/or cards in your graveyard"
        }
        "one or more" => {
            "Exile one or more other permanents you control and/or cards from your graveyard"
        }
        "six artifacts" => {
            "Exile the six from among other permanents you control and/or cards from your graveyard"
        }
        "four or more red instant and/or sorcery cards" => {
            "Exile the four or more from your graveyard"
        }
        "one or more Dinosaurs" => {
            "Exile one or more Dinosaurs you control and/or Dinosaur cards from your graveyard"
        }
        _ => "",
    }
    .into()
}

fn semantic_digest(
    exact_source: &str,
    source_context: &SourceSemanticContext,
    kind: &GraveyardTransformKeywordKind,
) -> String {
    semantic_digest_with_versions(
        exact_source,
        source_context,
        kind,
        GRAVEYARD_TRANSFORM_KEYWORD_COMPILER_VERSION,
        GRAVEYARD_TRANSFORM_KEYWORD_RUNTIME_VERSION,
        GRAVEYARD_TRANSFORM_KEYWORD_RULES_CONTEXT_VERSION,
    )
}

fn semantic_digest_with_versions(
    exact_source: &str,
    source_context: &SourceSemanticContext,
    kind: &GraveyardTransformKeywordKind,
    compiler_version: &str,
    runtime_version: &str,
    rules_context_version: &str,
) -> String {
    let mut hasher = Sha256::new();
    for component in [
        "graveyard-transform-keyword-content/v1".to_owned(),
        compiler_version.to_owned(),
        runtime_version.to_owned(),
        rules_context_version.to_owned(),
        exact_source.to_owned(),
        source_context.stable_id(),
        kind.stable_id(),
    ] {
        hasher.update((component.len() as u64).to_le_bytes());
        hasher.update(component.as_bytes());
    }
    format!("{:X}", hasher.finalize())
}

fn complete_single_line(source: &str) -> bool {
    !source.is_empty()
        && source == source.trim()
        && !source.contains('\n')
        && !source.contains('\r')
}

fn split_trailing_parenthetical(source: &str) -> Option<(&str, Option<&str>)> {
    if !source.ends_with(')') {
        return Some((source, None));
    }
    let opening = source.rfind(" (")?;
    let core = &source[..opening];
    let reminder = &source[opening + 2..source.len() - 1];
    if core.is_empty() || reminder.is_empty() || reminder.contains('(') || reminder.contains(')') {
        return None;
    }
    Some((core, Some(reminder)))
}

fn back_has_complete_graveyard_exile_replacement(normalized_oracle_text: &str) -> bool {
    normalized_oracle_text.lines().map(str::trim).any(|line| {
        line == "If this object would be put into a graveyard from anywhere, exile it instead."
            || line
                == "if this object would be put into a graveyard from anywhere, exile it instead."
    })
}

fn parse_mana_cost(source: &str) -> Option<ManaCost> {
    if source.is_empty() || source != source.trim() {
        return None;
    }
    let mut rest = source;
    let mut symbols = Vec::new();
    while !rest.is_empty() {
        let after_open = rest.strip_prefix('{')?;
        let close = after_open.find('}')?;
        let token = &after_open[..close];
        if token.is_empty() || token.contains('/') {
            return None;
        }
        let symbol = match token {
            "W" => ManaSymbol::White,
            "U" => ManaSymbol::Blue,
            "B" => ManaSymbol::Black,
            "R" => ManaSymbol::Red,
            "G" => ManaSymbol::Green,
            "C" => ManaSymbol::Colorless,
            "S" => ManaSymbol::Snow,
            _ => ManaSymbol::Generic(token.parse::<u32>().ok()?),
        };
        symbols.push(symbol);
        rest = &after_open[close + 1..];
    }
    if symbols.is_empty() {
        return None;
    }
    Some(ManaCost {
        exact: source.to_owned(),
        symbols,
    })
}

fn canonical_space(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn canonical_word(source: &str) -> String {
    canonical_space(source).to_ascii_lowercase()
}

fn card_types_from_type_line(type_line: &str) -> BTreeSet<CardType> {
    let before_dash = type_line
        .split_once('\u{2014}')
        .map(|(left, _)| left)
        .or_else(|| type_line.split_once(" - ").map(|(left, _)| left))
        .unwrap_or(type_line);
    before_dash
        .split_whitespace()
        .filter_map(
            |word| match word.trim_matches(|character: char| !character.is_alphabetic()) {
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
            },
        )
        .collect()
}

fn subtypes_from_type_line(type_line: &str) -> BTreeSet<String> {
    let after_dash = type_line
        .split_once('\u{2014}')
        .map(|(_, right)| right)
        .or_else(|| type_line.split_once(" - ").map(|(_, right)| right));
    after_dash
        .unwrap_or_default()
        .split_whitespace()
        .map(|value| value.trim_matches(|character: char| !character.is_alphanumeric()))
        .filter(|value| !value.is_empty())
        .map(canonical_word)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceCharacteristics {
    /// Display metadata is retained on the physical object but is not part of
    /// the program identity.
    pub display_name: String,
    pub semantic: FaceSemanticContext,
    /// `None` means the ability inventory is incomplete.
    pub has_activated_ability: Option<bool>,
}

impl FaceCharacteristics {
    fn card_types(&self) -> BTreeSet<CardType> {
        self.semantic.card_types()
    }

    fn subtypes(&self) -> BTreeSet<String> {
        subtypes_from_type_line(&self.semantic.type_line)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalCardDefinition {
    pub layout: CardLayout,
    pub front: FaceCharacteristics,
    pub back: Option<FaceCharacteristics>,
}

impl PhysicalCardDefinition {
    fn face(&self, face: FaceId) -> Option<&FaceCharacteristics> {
        match face {
            FaceId::Front => Some(&self.front),
            FaceId::Back => self.back.as_ref(),
        }
    }

    fn source_context_for(&self, keyword_face: FaceId) -> Option<SourceSemanticContext> {
        match self.layout {
            CardLayout::Normal => {
                if keyword_face != FaceId::Front || self.back.is_some() {
                    return None;
                }
                Some(SourceSemanticContext::SingleFace {
                    layout: CardLayout::Normal,
                    type_line: self.front.semantic.type_line.clone(),
                    normalized_oracle_text: self.front.semantic.normalized_oracle_text.clone(),
                })
            }
            CardLayout::Transform => {
                Some(SourceSemanticContext::Transform(TransformSemanticContext {
                    layout: CardLayout::Transform,
                    keyword_face,
                    front: self.front.semantic.clone(),
                    back: self.back.as_ref()?.semantic.clone(),
                }))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftLinkedExileState {
    pub craft_ability_id: PendingCraftAbilityId,
    pub craft_program_digest: String,
    /// Only cards still represented by the exact exile incarnations remain
    /// linked under rule 702.167c.
    pub material_cards_in_exile: BTreeSet<ObjectRef>,
    /// This audit list records every card exiled to pay the cost.
    pub all_material_exile_incarnations: BTreeSet<ObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CastMode {
    Ordinary,
    Disturb {
        semantic_digest: String,
        caster: PlayerId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackedObject {
    pub object_ref: ObjectRef,
    pub owner: PlayerId,
    pub controller: Option<PlayerId>,
    pub zone: Zone,
    pub active_face: FaceId,
    pub definition: PhysicalCardDefinition,
    pub cast_mode: Option<CastMode>,
    pub craft_link: Option<CraftLinkedExileState>,
}

impl TrackedObject {
    pub fn card(
        object_ref: ObjectRef,
        owner: PlayerId,
        zone: Zone,
        definition: PhysicalCardDefinition,
    ) -> Self {
        Self {
            object_ref,
            owner,
            controller: None,
            zone,
            active_face: FaceId::Front,
            definition,
            cast_mode: None,
            craft_link: None,
        }
    }

    pub fn permanent(
        object_ref: ObjectRef,
        owner: PlayerId,
        controller: PlayerId,
        active_face: FaceId,
        definition: PhysicalCardDefinition,
    ) -> Self {
        Self {
            object_ref,
            owner,
            controller: Some(controller),
            zone: Zone::Battlefield,
            active_face,
            definition,
            cast_mode: None,
            craft_link: None,
        }
    }

    pub fn current_face(&self) -> Option<&FaceCharacteristics> {
        self.definition.face(self.active_face)
    }

    pub fn effective_mana_value(&self) -> Option<u32> {
        match self.definition.layout {
            CardLayout::Normal | CardLayout::Transform => {
                Some(self.definition.front.semantic.root_mana_value)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaUnit {
    pub id: ManaUnitId,
    pub color: ManaColor,
    pub produced_by_snow_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerState {
    pub player: PlayerId,
    pub mana_pool: BTreeMap<ManaUnitId, ManaUnit>,
}

impl PlayerState {
    pub fn new(player: PlayerId) -> Self {
        Self {
            player,
            mana_pool: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Beginning,
    PrecombatMain,
    Combat,
    PostcombatMain,
    Ending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorityWindow {
    pub actor: PlayerId,
    pub active_player: PlayerId,
    pub phase: Phase,
    pub stack_empty: bool,
    pub has_priority: bool,
}

impl PriorityWindow {
    fn sorcery_timing_for(self, player: PlayerId) -> bool {
        self.actor == player
            && self.active_player == player
            && self.has_priority
            && self.stack_empty
            && matches!(self.phase, Phase::PrecombatMain | Phase::PostcombatMain)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaPaymentEvidence {
    pub player: PlayerId,
    pub mana_units: Vec<ManaUnitId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OtherCastCostEvidence {
    pub cost_inventory_complete: bool,
    pub all_required_costs_paid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisturbCastPermissionEvidence {
    pub timing_and_permission_inventory_complete: bool,
    pub casting_back_face_is_legal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisturbCastEvidence {
    pub caster: PlayerId,
    pub priority: PriorityWindow,
    pub permission: DisturbCastPermissionEvidence,
    pub mana_payment: ManaPaymentEvidence,
    pub other_costs: OtherCastCostEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct KeywordBindingId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordBinding {
    pub id: KeywordBindingId,
    pub physical_object_id: ObjectId,
    pub program: GraveyardTransformKeywordProgram,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisturbCastReceipt {
    pub physical_object_id: ObjectId,
    pub previous_graveyard_object: ObjectRef,
    pub stack_object: ObjectRef,
    pub caster: PlayerId,
    pub active_face: FaceId,
    pub cast_mode: CastMode,
    pub semantic_digest: String,
    pub mana_units_spent: Vec<ManaUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DisturbedSpellState {
    stack_object: ObjectRef,
    caster: PlayerId,
    program_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisturbResolutionReceipt {
    pub stack_object: ObjectRef,
    pub battlefield_object: ObjectRef,
    pub controller: PlayerId,
    pub active_face: FaceId,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReplacementEffectIdentity {
    DisturbBackFace { semantic_digest: String },
    External(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementOutcome {
    MoveTo(Zone),
    PreventZoneChange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementStepEvidence {
    pub chooser: PlayerId,
    pub applicable_effects_complete: bool,
    pub applicable: BTreeSet<ReplacementEffectIdentity>,
    pub chosen: ReplacementEffectIdentity,
    /// This must be `None` for the intrinsic Disturb replacement.
    pub external_outcome: Option<ReplacementOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneChangeReplacementEvidence {
    pub affected_player: PlayerId,
    pub steps: Vec<ReplacementStepEvidence>,
    pub final_applicable_effects_complete: bool,
    pub final_applicable: BTreeSet<ReplacementEffectIdentity>,
}

impl ZoneChangeReplacementEvidence {
    pub fn no_effects(affected_player: PlayerId) -> Self {
        Self {
            affected_player,
            steps: Vec::new(),
            final_applicable_effects_complete: true,
            final_applicable: BTreeSet::new(),
        }
    }

    pub fn intrinsic_disturb_only(
        affected_player: PlayerId,
        semantic_digest: impl Into<String>,
    ) -> Self {
        let identity = ReplacementEffectIdentity::DisturbBackFace {
            semantic_digest: semantic_digest.into(),
        };
        Self {
            affected_player,
            steps: vec![ReplacementStepEvidence {
                chooser: affected_player,
                applicable_effects_complete: true,
                applicable: BTreeSet::from([identity.clone()]),
                chosen: identity,
                external_outcome: None,
            }],
            final_applicable_effects_complete: true,
            final_applicable: BTreeSet::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneChangeReceipt {
    pub old_object: ObjectRef,
    pub new_object: Option<ObjectRef>,
    pub origin: Zone,
    pub requested_destination: Zone,
    pub actual_destination: Option<Zone>,
    pub graveyard_replaced_with_exile: bool,
    pub applied_replacements: Vec<ReplacementEffectIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoulshiftDeathReceipt {
    pub battlefield_object: ObjectRef,
    pub destination_object: Option<ObjectRef>,
    pub destination: Option<Zone>,
    pub triggered: Vec<PendingSoulshiftTriggerId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoulshiftTargetDeclaration {
    Target(ObjectRef),
    NoLegalTarget { graveyard_inventory_complete: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SoulshiftTargetState {
    Undeclared,
    Target(ObjectRef),
    NoLegalTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSoulshiftTrigger {
    pub id: PendingSoulshiftTriggerId,
    pub binding_id: KeywordBindingId,
    pub ability_controller: PlayerId,
    pub source_last_known_object: ObjectRef,
    pub maximum_mana_value: u32,
    pub semantic_digest: String,
    target: SoulshiftTargetState,
}

impl PendingSoulshiftTrigger {
    pub fn declared_target(&self) -> Option<ObjectRef> {
        match self.target {
            SoulshiftTargetState::Target(target) => Some(target),
            SoulshiftTargetState::Undeclared | SoulshiftTargetState::NoLegalTarget => None,
        }
    }

    pub fn has_no_legal_target(&self) -> bool {
        self.target == SoulshiftTargetState::NoLegalTarget
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulshiftResolutionChoice {
    ReturnTarget,
    Decline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoulshiftResolution {
    NoLegalTarget,
    Declined {
        target: ObjectRef,
    },
    CounteredForIllegalTarget {
        target: ObjectRef,
    },
    ReturnedToHand {
        old_target: ObjectRef,
        hand_object: ObjectRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoulshiftResolutionReceipt {
    pub trigger_id: PendingSoulshiftTriggerId,
    pub semantic_digest: String,
    pub resolution: SoulshiftResolution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftActivationEvidence {
    pub activator: PlayerId,
    pub priority: PriorityWindow,
    pub mana_payment: ManaPaymentEvidence,
    pub cost_choice_inventory_complete: bool,
    /// There must be one complete entry for the source and each material.
    pub zone_change_replacements: BTreeMap<ObjectRef, ZoneChangeReplacementEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftExileEvent {
    pub old_object: ObjectRef,
    pub exile_object: ObjectRef,
    pub origin: Zone,
    pub is_source: bool,
    pub during_craft_activation: bool,
    pub applied_replacements: Vec<ReplacementEffectIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingCraftAbility {
    pub id: PendingCraftAbilityId,
    pub activator: PlayerId,
    pub source_owner: PlayerId,
    pub source_exile_object: ObjectRef,
    pub material_exile_objects: BTreeSet<ObjectRef>,
    pub shared_card_types: BTreeSet<CardType>,
    pub subtype_role_assignment: BTreeMap<String, ObjectRef>,
    pub semantic_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftActivationReceipt {
    pub ability: PendingCraftAbility,
    pub exile_events: Vec<CraftExileEvent>,
    pub mana_units_spent: Vec<ManaUnitId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftResolution {
    SourceNoLongerInCraftExile {
        expected_source: ObjectRef,
    },
    ReturnedTransformed {
        exile_source: ObjectRef,
        battlefield_object: ObjectRef,
        controller: PlayerId,
        linked_materials: BTreeSet<ObjectRef>,
        materials_no_longer_in_exile: BTreeSet<ObjectRef>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraftResolutionReceipt {
    pub ability_id: PendingCraftAbilityId,
    pub semantic_digest: String,
    pub resolution: CraftResolution,
}

#[derive(Debug, Default)]
pub struct GraveyardTransformKeywordRuntime {
    players: BTreeMap<PlayerId, PlayerState>,
    objects: BTreeMap<ObjectId, TrackedObject>,
    bindings: BTreeMap<ObjectId, Vec<KeywordBinding>>,
    disturbed_spells: BTreeMap<ObjectRef, DisturbedSpellState>,
    pending_soulshift: BTreeMap<PendingSoulshiftTriggerId, PendingSoulshiftTrigger>,
    resolved_soulshift: BTreeMap<PendingSoulshiftTriggerId, SoulshiftResolutionReceipt>,
    pending_craft: BTreeMap<PendingCraftAbilityId, PendingCraftAbility>,
    resolved_craft: BTreeMap<PendingCraftAbilityId, CraftResolutionReceipt>,
    disturb_cast_ledger: Vec<DisturbCastReceipt>,
    next_binding_id: u64,
    next_soulshift_trigger_id: u64,
    next_craft_ability_id: u64,
}

impl GraveyardTransformKeywordRuntime {
    pub fn new() -> Self {
        Self {
            next_binding_id: 1,
            next_soulshift_trigger_id: 1,
            next_craft_ability_id: 1,
            ..Self::default()
        }
    }

    pub fn insert_player(
        &mut self,
        player: PlayerState,
    ) -> Result<(), GraveyardTransformRuntimeError> {
        if self.players.contains_key(&player.player) {
            return Err(GraveyardTransformRuntimeError::DuplicatePlayer(
                player.player,
            ));
        }
        self.players.insert(player.player, player);
        Ok(())
    }

    pub fn insert_object(
        &mut self,
        object: TrackedObject,
    ) -> Result<(), GraveyardTransformRuntimeError> {
        validate_tracked_object(&object)?;
        if self.objects.contains_key(&object.object_ref.object_id) {
            return Err(GraveyardTransformRuntimeError::DuplicateObject(
                object.object_ref.object_id,
            ));
        }
        self.objects.insert(object.object_ref.object_id, object);
        Ok(())
    }

    pub fn object(&self, object_id: ObjectId) -> Option<&TrackedObject> {
        self.objects.get(&object_id)
    }

    pub fn pending_soulshift_trigger(
        &self,
        trigger_id: PendingSoulshiftTriggerId,
    ) -> Option<&PendingSoulshiftTrigger> {
        self.pending_soulshift.get(&trigger_id)
    }

    pub fn resolved_soulshift_trigger(
        &self,
        trigger_id: PendingSoulshiftTriggerId,
    ) -> Option<&SoulshiftResolutionReceipt> {
        self.resolved_soulshift.get(&trigger_id)
    }

    pub fn pending_craft_ability(
        &self,
        ability_id: PendingCraftAbilityId,
    ) -> Option<&PendingCraftAbility> {
        self.pending_craft.get(&ability_id)
    }

    pub fn resolved_craft_ability(
        &self,
        ability_id: PendingCraftAbilityId,
    ) -> Option<&CraftResolutionReceipt> {
        self.resolved_craft.get(&ability_id)
    }

    pub fn disturb_cast_ledger(&self) -> &[DisturbCastReceipt] {
        &self.disturb_cast_ledger
    }

    pub fn install_program(
        &mut self,
        source: ObjectRef,
        program: GraveyardTransformKeywordProgram,
    ) -> Result<KeywordBindingId, GraveyardTransformRuntimeError> {
        let object = self.exact_object(source)?;
        let expected_context = match program.kind() {
            GraveyardTransformKeywordKind::Soulshift(_) => {
                object.definition.source_context_for(FaceId::Front)
            }
            GraveyardTransformKeywordKind::Disturb(_) | GraveyardTransformKeywordKind::Craft(_) => {
                object.definition.source_context_for(FaceId::Front)
            }
        }
        .ok_or(GraveyardTransformRuntimeError::IncompleteFaceContext)?;
        if expected_context != *program.source_context() {
            return Err(GraveyardTransformRuntimeError::ProgramContextMismatch);
        }
        if !matches!(program.kind(), GraveyardTransformKeywordKind::Soulshift(_))
            && self
                .bindings
                .get(&source.object_id)
                .is_some_and(|bindings| {
                    bindings
                        .iter()
                        .any(|binding| binding.program.kind().label() == program.kind().label())
                })
        {
            return Err(GraveyardTransformRuntimeError::DuplicateBinding);
        }
        let id = KeywordBindingId(self.next_binding_id);
        self.next_binding_id = self
            .next_binding_id
            .checked_add(1)
            .ok_or(GraveyardTransformRuntimeError::IdentifierOverflow)?;
        self.bindings
            .entry(source.object_id)
            .or_default()
            .push(KeywordBinding {
                id,
                physical_object_id: source.object_id,
                program,
            });
        Ok(id)
    }

    pub fn cast_with_disturb(
        &mut self,
        source: ObjectRef,
        semantic_digest: &str,
        evidence: DisturbCastEvidence,
    ) -> Result<DisturbCastReceipt, GraveyardTransformRuntimeError> {
        let program = self
            .find_program(source.object_id, semantic_digest, "Disturb")?
            .clone();
        let GraveyardTransformKeywordKind::Disturb(disturb) = program.kind() else {
            return Err(GraveyardTransformRuntimeError::WrongProgramKind);
        };
        let object = self.exact_object(source)?;
        if object.zone != Zone::Graveyard
            || object.owner != evidence.caster
            || object.active_face != FaceId::Front
        {
            return Err(GraveyardTransformRuntimeError::IllegalDisturbSource);
        }
        if evidence.priority.actor != evidence.caster || !evidence.priority.has_priority {
            return Err(GraveyardTransformRuntimeError::NoPriority);
        }
        if !evidence.permission.timing_and_permission_inventory_complete {
            return Err(GraveyardTransformRuntimeError::IncompleteCastPermissionEvidence);
        }
        if !evidence.permission.casting_back_face_is_legal {
            return Err(GraveyardTransformRuntimeError::IllegalCastTiming);
        }
        if !evidence.other_costs.cost_inventory_complete {
            return Err(GraveyardTransformRuntimeError::IncompleteOtherCostEvidence);
        }
        if !evidence.other_costs.all_required_costs_paid {
            return Err(GraveyardTransformRuntimeError::UnpaidOtherCastCost);
        }
        self.validate_mana_payment(
            evidence.caster,
            &disturb.alternative_cost,
            &evidence.mana_payment,
        )?;

        let mana_units_spent = evidence.mana_payment.mana_units.clone();
        self.spend_mana(evidence.caster, &mana_units_spent)?;
        let stack_object = self.move_exact_object_internal(
            source,
            Zone::Stack,
            Some(evidence.caster),
            FaceId::Back,
            Some(CastMode::Disturb {
                semantic_digest: semantic_digest.to_owned(),
                caster: evidence.caster,
            }),
            None,
        )?;
        self.disturbed_spells.insert(
            stack_object,
            DisturbedSpellState {
                stack_object,
                caster: evidence.caster,
                program_digest: semantic_digest.to_owned(),
            },
        );
        let receipt = DisturbCastReceipt {
            physical_object_id: source.object_id,
            previous_graveyard_object: source,
            stack_object,
            caster: evidence.caster,
            active_face: FaceId::Back,
            cast_mode: CastMode::Disturb {
                semantic_digest: semantic_digest.to_owned(),
                caster: evidence.caster,
            },
            semantic_digest: semantic_digest.to_owned(),
            mana_units_spent,
        };
        self.disturb_cast_ledger.push(receipt.clone());
        Ok(receipt)
    }

    pub fn resolve_disturbed_spell(
        &mut self,
        stack_object: ObjectRef,
    ) -> Result<DisturbResolutionReceipt, GraveyardTransformRuntimeError> {
        let state = self.disturbed_spells.get(&stack_object).cloned().ok_or(
            GraveyardTransformRuntimeError::NotDisturbedSpell(stack_object),
        )?;
        let object = self.exact_object(stack_object)?;
        if object.zone != Zone::Stack
            || object.active_face != FaceId::Back
            || !matches!(
                object.cast_mode,
                Some(CastMode::Disturb { ref semantic_digest, caster })
                    if semantic_digest == &state.program_digest && caster == state.caster
            )
        {
            return Err(GraveyardTransformRuntimeError::DisturbedSpellStateChanged);
        }
        self.disturbed_spells.remove(&stack_object);
        let battlefield_object = self.move_exact_object_internal(
            stack_object,
            Zone::Battlefield,
            Some(state.caster),
            FaceId::Back,
            None,
            None,
        )?;
        Ok(DisturbResolutionReceipt {
            stack_object,
            battlefield_object,
            controller: state.caster,
            active_face: FaceId::Back,
            semantic_digest: state.program_digest,
        })
    }

    pub fn move_object(
        &mut self,
        source: ObjectRef,
        requested_destination: Zone,
        destination_controller: Option<PlayerId>,
        replacement_evidence: ZoneChangeReplacementEvidence,
    ) -> Result<ZoneChangeReceipt, GraveyardTransformRuntimeError> {
        let object = self.exact_object(source)?;
        if object.zone == Zone::Battlefield
            && requested_destination == Zone::Graveyard
            && self.has_program_kind(source.object_id, "Soulshift")
        {
            return Err(GraveyardTransformRuntimeError::SoulshiftDeathEvidenceRequired);
        }
        self.move_object_with_replacements(
            source,
            requested_destination,
            destination_controller,
            replacement_evidence,
        )
    }

    pub fn move_battlefield_object_with_soulshift_evidence(
        &mut self,
        source: ObjectRef,
        requested_destination: Zone,
        replacement_evidence: ZoneChangeReplacementEvidence,
    ) -> Result<SoulshiftDeathReceipt, GraveyardTransformRuntimeError> {
        let object = self.exact_object(source)?;
        if object.zone != Zone::Battlefield {
            return Err(GraveyardTransformRuntimeError::NotBattlefieldObject(source));
        }
        let ability_controller = object
            .controller
            .ok_or(GraveyardTransformRuntimeError::MissingController)?;
        let soulshift_bindings = self
            .bindings
            .get(&source.object_id)
            .into_iter()
            .flatten()
            .filter_map(|binding| match binding.program.kind() {
                GraveyardTransformKeywordKind::Soulshift(program) => {
                    Some((binding.clone(), program.clone()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let zone_change = self.move_object_with_replacements(
            source,
            requested_destination,
            None,
            replacement_evidence,
        )?;
        let mut triggered = Vec::new();
        if zone_change.actual_destination == Some(Zone::Graveyard) {
            for (binding, soulshift) in soulshift_bindings {
                let id = PendingSoulshiftTriggerId(self.next_soulshift_trigger_id);
                self.next_soulshift_trigger_id = self
                    .next_soulshift_trigger_id
                    .checked_add(1)
                    .ok_or(GraveyardTransformRuntimeError::IdentifierOverflow)?;
                self.pending_soulshift.insert(
                    id,
                    PendingSoulshiftTrigger {
                        id,
                        binding_id: binding.id,
                        ability_controller,
                        source_last_known_object: source,
                        maximum_mana_value: soulshift.maximum_mana_value,
                        semantic_digest: binding.program.semantic_digest().to_owned(),
                        target: SoulshiftTargetState::Undeclared,
                    },
                );
                triggered.push(id);
            }
        }
        Ok(SoulshiftDeathReceipt {
            battlefield_object: source,
            destination_object: zone_change.new_object,
            destination: zone_change.actual_destination,
            triggered,
        })
    }

    pub fn declare_soulshift_target(
        &mut self,
        trigger_id: PendingSoulshiftTriggerId,
        declaring_player: PlayerId,
        declaration: SoulshiftTargetDeclaration,
    ) -> Result<(), GraveyardTransformRuntimeError> {
        let trigger = self.pending_soulshift.get(&trigger_id).cloned().ok_or(
            GraveyardTransformRuntimeError::UnknownSoulshiftTrigger(trigger_id),
        )?;
        if trigger.target != SoulshiftTargetState::Undeclared {
            return Err(GraveyardTransformRuntimeError::TargetAlreadyDeclared);
        }
        if declaring_player != trigger.ability_controller {
            return Err(GraveyardTransformRuntimeError::WrongAbilityController);
        }
        let target_state = match declaration {
            SoulshiftTargetDeclaration::Target(target) => {
                if !self.soulshift_target_is_legal(&trigger, target) {
                    return Err(GraveyardTransformRuntimeError::IllegalSoulshiftTarget(
                        target,
                    ));
                }
                SoulshiftTargetState::Target(target)
            }
            SoulshiftTargetDeclaration::NoLegalTarget {
                graveyard_inventory_complete,
            } => {
                if !graveyard_inventory_complete {
                    return Err(GraveyardTransformRuntimeError::IncompleteGraveyardInventory);
                }
                if self
                    .objects
                    .values()
                    .any(|object| self.soulshift_target_is_legal(&trigger, object.object_ref))
                {
                    return Err(GraveyardTransformRuntimeError::LegalSoulshiftTargetExists);
                }
                SoulshiftTargetState::NoLegalTarget
            }
        };
        self.pending_soulshift
            .get_mut(&trigger_id)
            .expect("trigger existence checked")
            .target = target_state;
        Ok(())
    }

    pub fn resolve_soulshift_trigger(
        &mut self,
        trigger_id: PendingSoulshiftTriggerId,
        choice: SoulshiftResolutionChoice,
    ) -> Result<SoulshiftResolutionReceipt, GraveyardTransformRuntimeError> {
        let trigger = self.pending_soulshift.remove(&trigger_id).ok_or(
            GraveyardTransformRuntimeError::UnknownSoulshiftTrigger(trigger_id),
        )?;
        let resolution = match trigger.target {
            SoulshiftTargetState::Undeclared => {
                self.pending_soulshift.insert(trigger_id, trigger);
                return Err(GraveyardTransformRuntimeError::TargetNotDeclared);
            }
            SoulshiftTargetState::NoLegalTarget => SoulshiftResolution::NoLegalTarget,
            SoulshiftTargetState::Target(target) => {
                if !self.soulshift_target_is_legal(&trigger, target) {
                    SoulshiftResolution::CounteredForIllegalTarget { target }
                } else if choice == SoulshiftResolutionChoice::Decline {
                    SoulshiftResolution::Declined { target }
                } else {
                    let hand_object = self.move_exact_object_internal(
                        target,
                        Zone::Hand,
                        None,
                        FaceId::Front,
                        None,
                        None,
                    )?;
                    SoulshiftResolution::ReturnedToHand {
                        old_target: target,
                        hand_object,
                    }
                }
            }
        };
        let receipt = SoulshiftResolutionReceipt {
            trigger_id,
            semantic_digest: trigger.semantic_digest,
            resolution,
        };
        self.resolved_soulshift.insert(trigger_id, receipt.clone());
        Ok(receipt)
    }

    pub fn activate_craft(
        &mut self,
        source: ObjectRef,
        semantic_digest: &str,
        material_choices: Vec<ObjectRef>,
        evidence: CraftActivationEvidence,
    ) -> Result<CraftActivationReceipt, GraveyardTransformRuntimeError> {
        let program = self
            .find_program(source.object_id, semantic_digest, "Craft")?
            .clone();
        let GraveyardTransformKeywordKind::Craft(craft) = program.kind() else {
            return Err(GraveyardTransformRuntimeError::WrongProgramKind);
        };
        let source_object = self.exact_object(source)?;
        if source_object.zone != Zone::Battlefield
            || source_object.active_face != FaceId::Front
            || source_object.controller != Some(evidence.activator)
        {
            return Err(GraveyardTransformRuntimeError::IllegalCraftSource);
        }
        if !evidence.priority.sorcery_timing_for(evidence.activator) {
            return Err(GraveyardTransformRuntimeError::IllegalCraftTiming);
        }
        if !evidence.cost_choice_inventory_complete {
            return Err(GraveyardTransformRuntimeError::IncompleteCraftChoiceEvidence);
        }
        self.validate_mana_payment(
            evidence.activator,
            &craft.activation_cost,
            &evidence.mana_payment,
        )?;
        let validation = self.validate_craft_materials(
            source,
            evidence.activator,
            &craft.material_requirement,
            &material_choices,
        )?;
        let source_owner = source_object.owner;
        let required_replacement_keys = std::iter::once(source)
            .chain(material_choices.iter().copied())
            .collect::<BTreeSet<_>>();
        if evidence
            .zone_change_replacements
            .keys()
            .copied()
            .collect::<BTreeSet<_>>()
            != required_replacement_keys
        {
            return Err(GraveyardTransformRuntimeError::IncompleteCraftReplacementEvidence);
        }
        let source_replacement = self.resolve_zone_change_replacements(
            source_object,
            Zone::Exile,
            evidence
                .zone_change_replacements
                .get(&source)
                .expect("replacement key set checked"),
        )?;
        if source_replacement.destination != Some(Zone::Exile) {
            return Err(GraveyardTransformRuntimeError::CraftCostCouldNotExile(
                source,
            ));
        }
        let mut material_replacements = BTreeMap::new();
        for material in &material_choices {
            let outcome = self.resolve_zone_change_replacements(
                self.exact_object(*material)?,
                Zone::Exile,
                evidence
                    .zone_change_replacements
                    .get(material)
                    .expect("replacement key set checked"),
            )?;
            if outcome.destination != Some(Zone::Exile) {
                return Err(GraveyardTransformRuntimeError::CraftCostCouldNotExile(
                    *material,
                ));
            }
            material_replacements.insert(*material, outcome);
        }

        let mana_units_spent = evidence.mana_payment.mana_units.clone();
        self.spend_mana(evidence.activator, &mana_units_spent)?;

        let source_exile_object =
            self.move_exact_object_internal(source, Zone::Exile, None, FaceId::Front, None, None)?;
        let mut exile_events = vec![CraftExileEvent {
            old_object: source,
            exile_object: source_exile_object,
            origin: Zone::Battlefield,
            is_source: true,
            during_craft_activation: true,
            applied_replacements: source_replacement.applied,
        }];
        let mut material_exile_objects = BTreeSet::new();
        let mut old_to_new = BTreeMap::new();
        for material in &material_choices {
            let origin = self.exact_object(*material)?.zone;
            let exile_object = self.move_exact_object_internal(
                *material,
                Zone::Exile,
                None,
                FaceId::Front,
                None,
                None,
            )?;
            old_to_new.insert(*material, exile_object);
            material_exile_objects.insert(exile_object);
            exile_events.push(CraftExileEvent {
                old_object: *material,
                exile_object,
                origin,
                is_source: false,
                during_craft_activation: true,
                applied_replacements: material_replacements
                    .remove(material)
                    .expect("validated replacement outcome")
                    .applied,
            });
        }

        let id = PendingCraftAbilityId(self.next_craft_ability_id);
        self.next_craft_ability_id = self
            .next_craft_ability_id
            .checked_add(1)
            .ok_or(GraveyardTransformRuntimeError::IdentifierOverflow)?;
        let subtype_role_assignment = validation
            .subtype_role_assignment
            .into_iter()
            .map(|(role, old_object)| {
                (
                    role,
                    *old_to_new
                        .get(&old_object)
                        .expect("validated material has exile incarnation"),
                )
            })
            .collect();
        let ability = PendingCraftAbility {
            id,
            activator: evidence.activator,
            source_owner,
            source_exile_object,
            material_exile_objects,
            shared_card_types: validation.shared_card_types,
            subtype_role_assignment,
            semantic_digest: semantic_digest.to_owned(),
        };
        self.pending_craft.insert(id, ability.clone());
        Ok(CraftActivationReceipt {
            ability,
            exile_events,
            mana_units_spent,
        })
    }

    pub fn resolve_craft_ability(
        &mut self,
        ability_id: PendingCraftAbilityId,
    ) -> Result<CraftResolutionReceipt, GraveyardTransformRuntimeError> {
        let ability = self.pending_craft.remove(&ability_id).ok_or(
            GraveyardTransformRuntimeError::UnknownCraftAbility(ability_id),
        )?;
        let source_still_present = self
            .objects
            .get(&ability.source_exile_object.object_id)
            .is_some_and(|object| {
                object.object_ref == ability.source_exile_object && object.zone == Zone::Exile
            });
        let resolution = if !source_still_present {
            CraftResolution::SourceNoLongerInCraftExile {
                expected_source: ability.source_exile_object,
            }
        } else {
            let linked_materials = ability
                .material_exile_objects
                .iter()
                .copied()
                .filter(|material| {
                    self.objects.get(&material.object_id).is_some_and(|object| {
                        object.object_ref == *material && object.zone == Zone::Exile
                    })
                })
                .collect::<BTreeSet<_>>();
            let materials_no_longer_in_exile = ability
                .material_exile_objects
                .difference(&linked_materials)
                .copied()
                .collect::<BTreeSet<_>>();
            let craft_link = CraftLinkedExileState {
                craft_ability_id: ability.id,
                craft_program_digest: ability.semantic_digest.clone(),
                material_cards_in_exile: linked_materials.clone(),
                all_material_exile_incarnations: ability.material_exile_objects.clone(),
            };
            let battlefield_object = self.move_exact_object_internal(
                ability.source_exile_object,
                Zone::Battlefield,
                Some(ability.source_owner),
                FaceId::Back,
                None,
                Some(craft_link),
            )?;
            CraftResolution::ReturnedTransformed {
                exile_source: ability.source_exile_object,
                battlefield_object,
                controller: ability.source_owner,
                linked_materials,
                materials_no_longer_in_exile,
            }
        };
        let receipt = CraftResolutionReceipt {
            ability_id,
            semantic_digest: ability.semantic_digest,
            resolution,
        };
        self.resolved_craft.insert(ability_id, receipt.clone());
        Ok(receipt)
    }

    fn exact_object(
        &self,
        object_ref: ObjectRef,
    ) -> Result<&TrackedObject, GraveyardTransformRuntimeError> {
        let object = self.objects.get(&object_ref.object_id).ok_or(
            GraveyardTransformRuntimeError::UnknownObject(object_ref.object_id),
        )?;
        if object.object_ref != object_ref {
            return Err(GraveyardTransformRuntimeError::IncarnationMismatch {
                expected: object.object_ref,
                actual: object_ref,
            });
        }
        Ok(object)
    }

    fn find_program(
        &self,
        object_id: ObjectId,
        semantic_digest: &str,
        label: &str,
    ) -> Result<&GraveyardTransformKeywordProgram, GraveyardTransformRuntimeError> {
        self.bindings
            .get(&object_id)
            .into_iter()
            .flatten()
            .find(|binding| {
                binding.program.semantic_digest() == semantic_digest
                    && binding.program.kind().label() == label
            })
            .map(|binding| &binding.program)
            .ok_or_else(|| GraveyardTransformRuntimeError::ProgramNotInstalled {
                object_id,
                semantic_digest: semantic_digest.to_owned(),
            })
    }

    fn has_program_kind(&self, object_id: ObjectId, label: &str) -> bool {
        self.bindings.get(&object_id).is_some_and(|bindings| {
            bindings
                .iter()
                .any(|binding| binding.program.kind().label() == label)
        })
    }

    fn validate_mana_payment(
        &self,
        player: PlayerId,
        cost: &ManaCost,
        evidence: &ManaPaymentEvidence,
    ) -> Result<(), GraveyardTransformRuntimeError> {
        if evidence.player != player {
            return Err(GraveyardTransformRuntimeError::WrongManaPayer);
        }
        let state = self
            .players
            .get(&player)
            .ok_or(GraveyardTransformRuntimeError::UnknownPlayer(player))?;
        let required_units = cost
            .required_units()
            .ok_or(GraveyardTransformRuntimeError::ManaCostOverflow)?;
        let selected = evidence.mana_units.iter().copied().collect::<BTreeSet<_>>();
        if selected.len() != evidence.mana_units.len() || selected.len() != required_units {
            return Err(GraveyardTransformRuntimeError::IncorrectManaPayment);
        }
        let mut units = selected
            .iter()
            .map(|id| {
                state
                    .mana_pool
                    .get(id)
                    .cloned()
                    .ok_or(GraveyardTransformRuntimeError::MissingManaUnit(*id))
            })
            .collect::<Result<Vec<_>, _>>()?;

        for symbol in cost
            .symbols
            .iter()
            .filter(|symbol| !matches!(symbol, ManaSymbol::Generic(_)))
        {
            let index = units
                .iter()
                .position(|unit| mana_unit_matches_symbol(unit, *symbol))
                .ok_or(GraveyardTransformRuntimeError::IncorrectManaPayment)?;
            units.remove(index);
        }
        let generic_required = cost
            .symbols
            .iter()
            .filter_map(|symbol| match symbol {
                ManaSymbol::Generic(amount) => Some(*amount as usize),
                _ => None,
            })
            .sum::<usize>();
        if units.len() != generic_required {
            return Err(GraveyardTransformRuntimeError::IncorrectManaPayment);
        }
        Ok(())
    }

    fn spend_mana(
        &mut self,
        player: PlayerId,
        mana_units: &[ManaUnitId],
    ) -> Result<(), GraveyardTransformRuntimeError> {
        let state = self
            .players
            .get_mut(&player)
            .ok_or(GraveyardTransformRuntimeError::UnknownPlayer(player))?;
        for id in mana_units {
            if state.mana_pool.remove(id).is_none() {
                return Err(GraveyardTransformRuntimeError::MissingManaUnit(*id));
            }
        }
        Ok(())
    }

    fn soulshift_target_is_legal(
        &self,
        trigger: &PendingSoulshiftTrigger,
        target: ObjectRef,
    ) -> bool {
        let Ok(object) = self.exact_object(target) else {
            return false;
        };
        if object.zone != Zone::Graveyard || object.owner != trigger.ability_controller {
            return false;
        }
        let Some(face) = object.current_face() else {
            return false;
        };
        face.subtypes().contains("spirit")
            && object
                .effective_mana_value()
                .is_some_and(|mana_value| mana_value <= trigger.maximum_mana_value)
    }

    fn validate_craft_materials(
        &self,
        source: ObjectRef,
        activator: PlayerId,
        requirement: &CraftMaterialRequirement,
        choices: &[ObjectRef],
    ) -> Result<CraftMaterialValidation, GraveyardTransformRuntimeError> {
        let unique = choices.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != choices.len()
            || choices
                .iter()
                .any(|material| material.object_id == source.object_id)
        {
            return Err(GraveyardTransformRuntimeError::DuplicateOrSourceMaterial);
        }
        let objects = choices
            .iter()
            .map(|choice| self.exact_object(*choice))
            .collect::<Result<Vec<_>, _>>()?;
        let default_scope = MaterialLocationScope::ControlledBattlefieldOrYourGraveyard;
        let mut validation = CraftMaterialValidation::default();
        match requirement {
            CraftMaterialRequirement::Exact {
                count,
                filter,
                scope,
            } => {
                if choices.len() != *count {
                    return Err(GraveyardTransformRuntimeError::WrongCraftMaterialCount);
                }
                for object in &objects {
                    self.validate_material_location(object, activator, *scope)?;
                    if !material_matches_filter(object, filter)? {
                        return Err(GraveyardTransformRuntimeError::CraftMaterialFilterMismatch(
                            object.object_ref,
                        ));
                    }
                }
            }
            CraftMaterialRequirement::AtLeast {
                count,
                filter,
                scope,
            } => {
                if choices.len() < *count {
                    return Err(GraveyardTransformRuntimeError::WrongCraftMaterialCount);
                }
                for object in &objects {
                    self.validate_material_location(object, activator, *scope)?;
                    if !material_matches_filter(object, filter)? {
                        return Err(GraveyardTransformRuntimeError::CraftMaterialFilterMismatch(
                            object.object_ref,
                        ));
                    }
                }
            }
            CraftMaterialRequirement::TwoSharingCardType => {
                if choices.len() != 2 {
                    return Err(GraveyardTransformRuntimeError::WrongCraftMaterialCount);
                }
                for object in &objects {
                    self.validate_material_location(object, activator, default_scope)?;
                }
                let first_types = objects[0]
                    .current_face()
                    .ok_or(GraveyardTransformRuntimeError::IncompleteFaceContext)?
                    .card_types();
                let second_types = objects[1]
                    .current_face()
                    .ok_or(GraveyardTransformRuntimeError::IncompleteFaceContext)?
                    .card_types();
                validation.shared_card_types =
                    first_types.intersection(&second_types).copied().collect();
                if validation.shared_card_types.is_empty() {
                    return Err(GraveyardTransformRuntimeError::NoSharedCardType);
                }
            }
            CraftMaterialRequirement::FourDistinctCreatureSubtypes { required_subtypes } => {
                if choices.len() != required_subtypes.len() {
                    return Err(GraveyardTransformRuntimeError::WrongCraftMaterialCount);
                }
                for object in &objects {
                    self.validate_material_location(object, activator, default_scope)?;
                }
                validation.subtype_role_assignment =
                    assign_distinct_subtype_roles(required_subtypes, &objects)
                        .ok_or(GraveyardTransformRuntimeError::MissingRequiredCraftSubtype)?;
            }
        }
        Ok(validation)
    }

    fn validate_material_location(
        &self,
        object: &TrackedObject,
        activator: PlayerId,
        scope: MaterialLocationScope,
    ) -> Result<(), GraveyardTransformRuntimeError> {
        match object.zone {
            Zone::Battlefield
                if scope == MaterialLocationScope::ControlledBattlefieldOrYourGraveyard
                    && object.controller == Some(activator) =>
            {
                Ok(())
            }
            Zone::Graveyard if object.owner == activator => Ok(()),
            _ => Err(GraveyardTransformRuntimeError::IllegalCraftMaterialZone(
                object.object_ref,
            )),
        }
    }

    fn move_object_with_replacements(
        &mut self,
        source: ObjectRef,
        requested_destination: Zone,
        destination_controller: Option<PlayerId>,
        replacement_evidence: ZoneChangeReplacementEvidence,
    ) -> Result<ZoneChangeReceipt, GraveyardTransformRuntimeError> {
        let object = self.exact_object(source)?.clone();
        let origin = object.zone;
        let replacement_resolution = self.resolve_zone_change_replacements(
            &object,
            requested_destination,
            &replacement_evidence,
        )?;
        let graveyard_replaced_with_exile = replacement_resolution
            .applied
            .iter()
            .any(|identity| matches!(identity, ReplacementEffectIdentity::DisturbBackFace { .. }));
        let new_object = if let Some(actual_destination) = replacement_resolution.destination {
            let active_face = if matches!(actual_destination, Zone::Battlefield | Zone::Stack) {
                object.active_face
            } else {
                FaceId::Front
            };
            let actual_controller = if matches!(actual_destination, Zone::Battlefield | Zone::Stack)
            {
                destination_controller
            } else {
                None
            };
            if matches!(actual_destination, Zone::Battlefield | Zone::Stack)
                && actual_controller.is_none()
            {
                return Err(GraveyardTransformRuntimeError::InvalidDestinationController);
            }
            if origin == Zone::Stack {
                self.disturbed_spells.remove(&source);
            }
            Some(self.move_exact_object_internal(
                source,
                actual_destination,
                actual_controller,
                active_face,
                None,
                None,
            )?)
        } else {
            None
        };
        Ok(ZoneChangeReceipt {
            old_object: source,
            new_object,
            origin,
            requested_destination,
            actual_destination: replacement_resolution.destination,
            graveyard_replaced_with_exile,
            applied_replacements: replacement_resolution.applied,
        })
    }

    fn resolve_zone_change_replacements(
        &self,
        object: &TrackedObject,
        requested_destination: Zone,
        evidence: &ZoneChangeReplacementEvidence,
    ) -> Result<ReplacementResolution, GraveyardTransformRuntimeError> {
        let affected_player = object.controller.unwrap_or(object.owner);
        if evidence.affected_player != affected_player {
            return Err(GraveyardTransformRuntimeError::WrongReplacementChooser);
        }
        let mut destination = Some(requested_destination);
        let mut applied = BTreeSet::new();
        let mut ordered_applied = Vec::new();
        for step in &evidence.steps {
            if destination.is_none() {
                return Err(GraveyardTransformRuntimeError::ReplacementAfterPrevention);
            }
            if !step.applicable_effects_complete {
                return Err(GraveyardTransformRuntimeError::IncompleteReplacementEvidence);
            }
            if step.chooser != affected_player {
                return Err(GraveyardTransformRuntimeError::WrongReplacementChooser);
            }
            if !step.applicable.contains(&step.chosen) || applied.contains(&step.chosen) {
                return Err(GraveyardTransformRuntimeError::InvalidReplacementChoice);
            }
            let intrinsic =
                self.intrinsic_disturb_replacement(object, destination.expect("checked some"));
            validate_intrinsic_replacement_inventory(intrinsic.as_ref(), &step.applicable)?;
            match &step.chosen {
                ReplacementEffectIdentity::DisturbBackFace { .. } => {
                    if intrinsic.as_ref() != Some(&step.chosen) || step.external_outcome.is_some() {
                        return Err(GraveyardTransformRuntimeError::InvalidReplacementChoice);
                    }
                    destination = Some(Zone::Exile);
                }
                ReplacementEffectIdentity::External(_) => {
                    destination = match step
                        .external_outcome
                        .ok_or(GraveyardTransformRuntimeError::MissingExternalReplacementOutcome)?
                    {
                        ReplacementOutcome::MoveTo(zone) => Some(zone),
                        ReplacementOutcome::PreventZoneChange => None,
                    };
                }
            }
            applied.insert(step.chosen.clone());
            ordered_applied.push(step.chosen.clone());
        }
        if !evidence.final_applicable_effects_complete {
            return Err(GraveyardTransformRuntimeError::IncompleteReplacementEvidence);
        }
        let final_intrinsic = destination
            .and_then(|destination| self.intrinsic_disturb_replacement(object, destination))
            .filter(|identity| !applied.contains(identity));
        validate_intrinsic_replacement_inventory(
            final_intrinsic.as_ref(),
            &evidence.final_applicable,
        )?;
        if !evidence.final_applicable.is_empty() {
            return Err(GraveyardTransformRuntimeError::UnresolvedReplacementEffect);
        }
        Ok(ReplacementResolution {
            destination,
            applied: ordered_applied,
        })
    }

    fn intrinsic_disturb_replacement(
        &self,
        object: &TrackedObject,
        destination: Zone,
    ) -> Option<ReplacementEffectIdentity> {
        if destination != Zone::Graveyard || object.active_face != FaceId::Back {
            return None;
        }
        self.bindings
            .get(&object.object_ref.object_id)
            .into_iter()
            .flatten()
            .find_map(|binding| match binding.program.kind() {
                GraveyardTransformKeywordKind::Disturb(program)
                    if program.back_face_graveyard_move_is_replaced_with_exile =>
                {
                    Some(ReplacementEffectIdentity::DisturbBackFace {
                        semantic_digest: binding.program.semantic_digest().to_owned(),
                    })
                }
                _ => None,
            })
    }

    fn move_exact_object_internal(
        &mut self,
        source: ObjectRef,
        destination: Zone,
        controller: Option<PlayerId>,
        active_face: FaceId,
        cast_mode: Option<CastMode>,
        craft_link: Option<CraftLinkedExileState>,
    ) -> Result<ObjectRef, GraveyardTransformRuntimeError> {
        let object = self.exact_object(source)?.clone();
        if object.definition.face(active_face).is_none() {
            return Err(GraveyardTransformRuntimeError::MissingFace(active_face));
        }
        let expected_controller = matches!(destination, Zone::Battlefield | Zone::Stack);
        if expected_controller != controller.is_some() {
            return Err(GraveyardTransformRuntimeError::InvalidDestinationController);
        }
        let next_incarnation = IncarnationId(
            source
                .incarnation_id
                .0
                .checked_add(1)
                .ok_or(GraveyardTransformRuntimeError::IdentifierOverflow)?,
        );
        let new_ref = ObjectRef {
            object_id: source.object_id,
            incarnation_id: next_incarnation,
        };
        self.objects.insert(
            source.object_id,
            TrackedObject {
                object_ref: new_ref,
                owner: object.owner,
                controller,
                zone: destination,
                active_face,
                definition: object.definition,
                cast_mode,
                craft_link,
            },
        );
        Ok(new_ref)
    }
}

#[derive(Debug, Default)]
struct CraftMaterialValidation {
    shared_card_types: BTreeSet<CardType>,
    subtype_role_assignment: BTreeMap<String, ObjectRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplacementResolution {
    destination: Option<Zone>,
    applied: Vec<ReplacementEffectIdentity>,
}

fn validate_intrinsic_replacement_inventory(
    expected: Option<&ReplacementEffectIdentity>,
    applicable: &BTreeSet<ReplacementEffectIdentity>,
) -> Result<(), GraveyardTransformRuntimeError> {
    let supplied = applicable
        .iter()
        .filter(|identity| matches!(identity, ReplacementEffectIdentity::DisturbBackFace { .. }))
        .collect::<Vec<_>>();
    match expected {
        Some(expected) if supplied == [expected] => Ok(()),
        None if supplied.is_empty() => Ok(()),
        _ => Err(GraveyardTransformRuntimeError::IncompleteReplacementEvidence),
    }
}

fn validate_tracked_object(object: &TrackedObject) -> Result<(), GraveyardTransformRuntimeError> {
    if object.definition.face(object.active_face).is_none() {
        return Err(GraveyardTransformRuntimeError::MissingFace(
            object.active_face,
        ));
    }
    let requires_controller = matches!(object.zone, Zone::Battlefield | Zone::Stack);
    if requires_controller != object.controller.is_some() {
        return Err(GraveyardTransformRuntimeError::InvalidDestinationController);
    }
    if object.zone != Zone::Stack && object.cast_mode.is_some() {
        return Err(GraveyardTransformRuntimeError::CastModeOutsideStack);
    }
    if object.zone != Zone::Battlefield && object.craft_link.is_some() {
        return Err(GraveyardTransformRuntimeError::CraftLinkOutsideBattlefield);
    }
    match object.definition.layout {
        CardLayout::Normal if object.definition.back.is_some() => {
            Err(GraveyardTransformRuntimeError::IncompleteFaceContext)
        }
        CardLayout::Transform if object.definition.back.is_none() => {
            Err(GraveyardTransformRuntimeError::IncompleteFaceContext)
        }
        _ => Ok(()),
    }
}

fn mana_unit_matches_symbol(unit: &ManaUnit, symbol: ManaSymbol) -> bool {
    match symbol {
        ManaSymbol::White => unit.color == ManaColor::White,
        ManaSymbol::Blue => unit.color == ManaColor::Blue,
        ManaSymbol::Black => unit.color == ManaColor::Black,
        ManaSymbol::Red => unit.color == ManaColor::Red,
        ManaSymbol::Green => unit.color == ManaColor::Green,
        ManaSymbol::Colorless => unit.color == ManaColor::Colorless,
        ManaSymbol::Snow => unit.produced_by_snow_source,
        ManaSymbol::Generic(_) => true,
    }
}

fn material_matches_filter(
    object: &TrackedObject,
    filter: &MaterialFilter,
) -> Result<bool, GraveyardTransformRuntimeError> {
    let face = object
        .current_face()
        .ok_or(GraveyardTransformRuntimeError::IncompleteFaceContext)?;
    let card_types = face.card_types();
    Ok(match filter {
        MaterialFilter::AnyCard => true,
        MaterialFilter::CardType(card_type) => card_types.contains(card_type),
        MaterialFilter::Subtype(subtype) => face.subtypes().contains(&canonical_word(subtype)),
        MaterialFilter::NonlandWithActivatedAbility => {
            if card_types.contains(&CardType::Land) {
                false
            } else {
                face.has_activated_ability.ok_or(
                    GraveyardTransformRuntimeError::IncompleteActivatedAbilityInventory(
                        object.object_ref,
                    ),
                )?
            }
        }
        MaterialFilter::RedInstantOrSorcery => {
            (face.semantic.colors.contains(&ManaColor::Red)
                || face.semantic.color_indicator.contains(&ManaColor::Red))
                && (card_types.contains(&CardType::Instant)
                    || card_types.contains(&CardType::Sorcery))
        }
    })
}

fn assign_distinct_subtype_roles(
    required_subtypes: &BTreeSet<String>,
    objects: &[&TrackedObject],
) -> Option<BTreeMap<String, ObjectRef>> {
    fn assign(
        roles: &[String],
        role_index: usize,
        objects: &[&TrackedObject],
        used: &mut BTreeSet<ObjectRef>,
        output: &mut BTreeMap<String, ObjectRef>,
    ) -> bool {
        if role_index == roles.len() {
            return true;
        }
        let role = &roles[role_index];
        for object in objects {
            if used.contains(&object.object_ref)
                || !object
                    .current_face()
                    .is_some_and(|face| face.subtypes().contains(&canonical_word(role)))
            {
                continue;
            }
            used.insert(object.object_ref);
            output.insert(role.clone(), object.object_ref);
            if assign(roles, role_index + 1, objects, used, output) {
                return true;
            }
            output.remove(role);
            used.remove(&object.object_ref);
        }
        false
    }

    let roles = required_subtypes.iter().cloned().collect::<Vec<_>>();
    let mut used = BTreeSet::new();
    let mut output = BTreeMap::new();
    assign(&roles, 0, objects, &mut used, &mut output).then_some(output)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraveyardTransformRuntimeError {
    DuplicatePlayer(PlayerId),
    DuplicateObject(ObjectId),
    UnknownPlayer(PlayerId),
    UnknownObject(ObjectId),
    IncarnationMismatch {
        expected: ObjectRef,
        actual: ObjectRef,
    },
    MissingFace(FaceId),
    IncompleteFaceContext,
    ProgramContextMismatch,
    DuplicateBinding,
    ProgramNotInstalled {
        object_id: ObjectId,
        semantic_digest: String,
    },
    WrongProgramKind,
    IdentifierOverflow,
    InvalidDestinationController,
    CastModeOutsideStack,
    CraftLinkOutsideBattlefield,
    IllegalDisturbSource,
    NoPriority,
    IncompleteCastPermissionEvidence,
    IllegalCastTiming,
    IncompleteOtherCostEvidence,
    UnpaidOtherCastCost,
    WrongManaPayer,
    ManaCostOverflow,
    IncorrectManaPayment,
    MissingManaUnit(ManaUnitId),
    NotDisturbedSpell(ObjectRef),
    DisturbedSpellStateChanged,
    SoulshiftDeathEvidenceRequired,
    IncompleteReplacementEvidence,
    WrongReplacementChooser,
    ReplacementAfterPrevention,
    InvalidReplacementChoice,
    MissingExternalReplacementOutcome,
    UnresolvedReplacementEffect,
    NotBattlefieldObject(ObjectRef),
    MissingController,
    UnknownSoulshiftTrigger(PendingSoulshiftTriggerId),
    TargetAlreadyDeclared,
    WrongAbilityController,
    IllegalSoulshiftTarget(ObjectRef),
    IncompleteGraveyardInventory,
    LegalSoulshiftTargetExists,
    TargetNotDeclared,
    IllegalCraftSource,
    IllegalCraftTiming,
    IncompleteCraftChoiceEvidence,
    IncompleteCraftReplacementEvidence,
    CraftCostCouldNotExile(ObjectRef),
    DuplicateOrSourceMaterial,
    WrongCraftMaterialCount,
    CraftMaterialFilterMismatch(ObjectRef),
    IllegalCraftMaterialZone(ObjectRef),
    NoSharedCardType,
    MissingRequiredCraftSubtype,
    IncompleteActivatedAbilityInventory(ObjectRef),
    UnknownCraftAbility(PendingCraftAbilityId),
}

impl fmt::Display for GraveyardTransformRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for GraveyardTransformRuntimeError {}
