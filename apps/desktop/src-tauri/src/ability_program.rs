//! Fail-closed Oracle text to executable ability-program compiler.
//!
//! This version is intentionally narrow. It recognizes a reviewed set of
//! structure-based Oracle templates and never consults a card-name table.
//! Every Oracle paragraph becomes either one fully typed executable ability
//! or one unsupported ability with explicit reasons. The live simulator
//! consumes a deliberately narrow subset of these programs and leaves every
//! other clause visible as an execution-coverage gap.

#![allow(dead_code)]

use regex::{Regex, RegexBuilder};

pub(crate) const EXECUTABLE_ABILITY_PROGRAM_VERSION: &str = "executable-ability-program/v16";

#[derive(Debug, Clone, Copy)]
pub(crate) struct OracleCardInput<'a> {
    pub name: &'a str,
    pub layout: &'a str,
    pub type_line: &'a str,
    pub oracle_text: &'a str,
    /// Exact face records are retained separately by the card-data layer.
    /// Version 1 has no face-bound action identity, so a combined root must
    /// never be compiled as though all face text belonged to one spell.
    pub has_face_records: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct OracleCardFaceInput<'a> {
    pub name: &'a str,
    pub type_line: &'a str,
    pub oracle_text: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableAbilityProgramV1 {
    pub version: &'static str,
    pub abilities: Vec<AbilityCompilation>,
    /// A complete permanent lifecycle whose draw-step restriction, exact
    /// discarded-object exile trigger, and delayed top-card access must be
    /// retained together.
    ///
    /// Any recognized sibling claims the complete card root. This prevents
    /// the attractive life-payment activation from executing when either
    /// mandatory drawback changes, disappears, or gains an unsupported
    /// sibling.
    pub necropotence_lifecycle: Option<NecropotenceLifecycleCompilation>,
    /// A permanent whose replacement-entry counters, activated costs, search,
    /// and post-search control transfer form one reviewed lifecycle.
    ///
    /// The compiler owns the complete card root whenever this family is
    /// detected. This prevents the any-card search from executing if its
    /// counter setup, tap/counter costs, turn restriction, or mandatory
    /// control transfer changes.
    pub self_transfer_tutor_permanent: Option<SelfTransferTutorPermanentCompilation>,
    /// A permanent whose entry procedure and mana ability share state that
    /// cannot be represented safely as independent Oracle paragraphs.
    ///
    /// The compiler owns the complete card root whenever either reviewed
    /// entry-linked family is detected. This prevents a recognizable tap
    /// ability from executing when its imprint or replacement-entry sibling
    /// has changed or is unsupported.
    pub entry_linked_permanent: Option<EntryLinkedPermanentCompilation>,
    /// A complete card-level transaction whose costs and ordered resolution
    /// cannot be represented safely as independent Oracle paragraphs.
    ///
    /// This is singular by design in the current bounded compiler. When a
    /// root transaction candidate is recognized, it owns every paragraph of
    /// that candidate so a supported-looking sibling cannot execute alone.
    pub atomic_transaction: Option<AtomicTransactionCompilation>,
    /// Exact face-bound programs retained independently for supported
    /// transforming and modal double-faced layouts. Legacy consumers continue
    /// to see only the primary castable face in `abilities`; alternate faces
    /// remain separately attributable and cannot leak actions or costs into
    /// the primary face.
    pub face_programs: Vec<BoundFaceAbilityProgram>,
}

impl ExecutableAbilityProgramV1 {
    /// Fail-closed program for a deck entry whose card definition could not be
    /// resolved. With no Oracle text there is no executable clause to infer.
    pub(crate) fn unresolved() -> Self {
        Self {
            version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
            abilities: Vec::new(),
            necropotence_lifecycle: None,
            self_transfer_tutor_permanent: None,
            entry_linked_permanent: None,
            atomic_transaction: None,
            face_programs: Vec::new(),
        }
    }

    pub fn executable_abilities(&self) -> impl Iterator<Item = &ExecutableAbility> {
        self.abilities.iter().filter_map(|ability| match ability {
            AbilityCompilation::Executable(ability) => Some(ability),
            AbilityCompilation::Unsupported(_) => None,
        })
    }

    pub fn unsupported_abilities(&self) -> impl Iterator<Item = &UnsupportedAbility> {
        self.abilities.iter().filter_map(|ability| match ability {
            AbilityCompilation::Executable(_) => None,
            AbilityCompilation::Unsupported(ability) => Some(ability),
        })
    }

    pub fn executable_necropotence_lifecycle(&self) -> Option<&ExecutableNecropotenceLifecycle> {
        match self.necropotence_lifecycle.as_ref()? {
            NecropotenceLifecycleCompilation::Executable(lifecycle) => Some(lifecycle),
            NecropotenceLifecycleCompilation::Unsupported(_) => None,
        }
    }

    pub fn unsupported_necropotence_lifecycle(&self) -> Option<&UnsupportedNecropotenceLifecycle> {
        match self.necropotence_lifecycle.as_ref()? {
            NecropotenceLifecycleCompilation::Executable(_) => None,
            NecropotenceLifecycleCompilation::Unsupported(lifecycle) => Some(lifecycle),
        }
    }

    pub fn executable_self_transfer_tutor_permanent(
        &self,
    ) -> Option<&ExecutableSelfTransferTutorPermanent> {
        match self.self_transfer_tutor_permanent.as_ref()? {
            SelfTransferTutorPermanentCompilation::Executable(permanent) => Some(permanent),
            SelfTransferTutorPermanentCompilation::Unsupported(_) => None,
        }
    }

    pub fn unsupported_self_transfer_tutor_permanent(
        &self,
    ) -> Option<&UnsupportedSelfTransferTutorPermanent> {
        match self.self_transfer_tutor_permanent.as_ref()? {
            SelfTransferTutorPermanentCompilation::Executable(_) => None,
            SelfTransferTutorPermanentCompilation::Unsupported(permanent) => Some(permanent),
        }
    }

    pub fn executable_entry_linked_permanent(&self) -> Option<&ExecutableEntryLinkedPermanent> {
        match self.entry_linked_permanent.as_ref()? {
            EntryLinkedPermanentCompilation::Executable(permanent) => Some(permanent),
            EntryLinkedPermanentCompilation::Unsupported(_) => None,
        }
    }

    pub fn unsupported_entry_linked_permanent(&self) -> Option<&UnsupportedEntryLinkedPermanent> {
        match self.entry_linked_permanent.as_ref()? {
            EntryLinkedPermanentCompilation::Executable(_) => None,
            EntryLinkedPermanentCompilation::Unsupported(permanent) => Some(permanent),
        }
    }

    pub fn executable_atomic_transaction(&self) -> Option<&ExecutableAtomicTransaction> {
        match self.atomic_transaction.as_ref()? {
            AtomicTransactionCompilation::Executable(transaction) => Some(transaction),
            AtomicTransactionCompilation::Unsupported(_) => None,
        }
    }

    pub fn unsupported_atomic_transaction(&self) -> Option<&UnsupportedAtomicTransaction> {
        match self.atomic_transaction.as_ref()? {
            AtomicTransactionCompilation::Executable(_) => None,
            AtomicTransactionCompilation::Unsupported(transaction) => Some(transaction),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BoundFaceAbilityProgram {
    pub face_index: usize,
    pub name: String,
    pub type_line: String,
    pub disposition: FaceProgramDisposition,
    pub abilities: Vec<AbilityCompilation>,
    pub necropotence_lifecycle: Option<NecropotenceLifecycleCompilation>,
    pub self_transfer_tutor_permanent: Option<SelfTransferTutorPermanentCompilation>,
    pub entry_linked_permanent: Option<EntryLinkedPermanentCompilation>,
    pub atomic_transaction: Option<AtomicTransactionCompilation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FaceProgramDisposition {
    /// The first castable face. Its program is also exposed through the root
    /// compatibility fields while the simulator migrates to face-bound costs.
    PrimaryCastable,
    /// An alternate castable face whose complete program compiled without an
    /// unsupported clause. It remains face-bound and is never merged into the
    /// primary face's compatibility program.
    AlternateExecutable,
    /// A retained alternate face that cannot yet execute completely.
    ReportOnlyUnsupported,
}

impl Default for ExecutableAbilityProgramV1 {
    fn default() -> Self {
        Self::unresolved()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AbilityCompilation {
    Executable(ExecutableAbility),
    Unsupported(UnsupportedAbility),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NecropotenceLifecycleCompilation {
    Executable(ExecutableNecropotenceLifecycle),
    Unsupported(UnsupportedNecropotenceLifecycle),
}

/// One complete reviewed lifecycle for the enchantment whose three Oracle
/// paragraphs are mechanically linked. None of these fields is exposed as an
/// independent `AbilityCompilation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableNecropotenceLifecycle {
    pub normalized_oracle: String,
    pub draw_step: NecropotenceDrawStepRestriction,
    pub discarded_card: NecropotenceDiscardedCardTrigger,
    pub activation: NecropotenceCardAccessActivation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedNecropotenceLifecycle {
    pub normalized_oracle: String,
    pub reasons: Vec<UnsupportedReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NecropotenceDrawStepRestriction {
    pub player: ControllerRelation,
    pub step: TurnStep,
    pub procedure: StepProcedure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TurnStep {
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StepProcedure {
    Skip,
}

/// The trigger retains the identity of the one card discarded by its event.
/// The exact object is moved from its graveyard; this is not a generic exile
/// of any interchangeable graveyard card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NecropotenceDiscardedCardTrigger {
    pub player: ControllerRelation,
    pub event: NecropotenceDiscardEvent,
    pub tracked_object: DiscardedObjectReference,
    pub from: Zone,
    pub destination: Zone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NecropotenceDiscardEvent {
    WheneverYouDiscardOneCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiscardedObjectReference {
    CardDiscardedByThisTrigger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NecropotenceCardAccessActivation {
    pub source_zone: Zone,
    pub window: ActivationWindow,
    pub costs: Vec<AbilityCost>,
    pub access: LinkedDelayedCardAccessEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AtomicTransactionCompilation {
    Executable(ExecutableAtomicTransaction),
    Unsupported(UnsupportedAtomicTransaction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryLinkedPermanentCompilation {
    Executable(ExecutableEntryLinkedPermanent),
    Unsupported(UnsupportedEntryLinkedPermanent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelfTransferTutorPermanentCompilation {
    Executable(ExecutableSelfTransferTutorPermanent),
    Unsupported(UnsupportedSelfTransferTutorPermanent),
}

/// One complete first-controller lifecycle for the reviewed self-transfer
/// tutor permanent. Entry counters, activation costs, search, and source
/// transfer deliberately cannot be consumed as independent abilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableSelfTransferTutorPermanent {
    pub normalized_oracle: String,
    pub entry: SelfTransferTutorEntry,
    pub activation: SelfTransferTutorActivation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedSelfTransferTutorPermanent {
    pub normalized_oracle: String,
    pub reasons: Vec<UnsupportedReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelfTransferTutorEntry {
    pub counter: CounterKind,
    pub count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelfTransferTutorActivation {
    pub source_zone: Zone,
    pub window: SelfTransferTutorActivationWindow,
    /// Costs commit in printed order. Removing the counter is a cost, not a
    /// resolution effect, so a countered or otherwise stopped activation does
    /// not restore it.
    pub costs: Vec<SelfTransferTutorCost>,
    /// Resolution is ordered: search and shuffle first, then the opponent
    /// control transfer. Consumers must execute this vector atomically.
    pub resolution: Vec<SelfTransferTutorResolutionStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelfTransferTutorActivationWindow {
    DuringYourTurnOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelfTransferTutorCost {
    Mana(ManaCost),
    TapSelf,
    RemoveCounterFromSelf { counter: CounterKind, count: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SelfTransferTutorResolutionStep {
    SearchToHand(TutorEffect),
    OpponentGainsControlOfSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableEntryLinkedPermanent {
    /// Complete normalized root Oracle text. No paragraph in this root is
    /// compiled independently.
    pub normalized_oracle: String,
    pub entry: PermanentEntryProcedure,
    pub mana_ability: EntryLinkedManaAbility,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedEntryLinkedPermanent {
    pub normalized_oracle: String,
    pub reasons: Vec<UnsupportedReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PermanentEntryProcedure {
    /// On entry, the controller may move one matching hand card to exile and
    /// retain an exact link to that object for the permanent's mana output.
    OptionalImprint {
        filter: EntryLinkedCardFilter,
        from: Zone,
        to: Zone,
        link: LinkedEntryObject,
    },
    /// A replacement entry: discarding a matching hand card lets the source
    /// enter; declining or being unable moves the source to its owner's
    /// graveyard instead.
    DiscardOrFailToEnter {
        filter: EntryLinkedCardFilter,
        discard_from: Zone,
        discard_to: Zone,
        success_destination: Zone,
        failure_destination: Zone,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryLinkedCardFilter {
    NonartifactNonlandCard,
    LandCard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LinkedEntryObject {
    CardExiledByThisEntry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EntryLinkedManaAbility {
    pub costs: Vec<AbilityCost>,
    pub preconditions: Vec<AbilityPrecondition>,
    pub output: EntryLinkedManaOutput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryLinkedManaOutput {
    AnyColorOfLinkedCard { linked: LinkedEntryObject },
    AnyOneColor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableAtomicTransaction {
    /// Complete normalized root Oracle text. This deliberately spans every
    /// paragraph owned by the transaction.
    pub normalized_oracle: String,
    pub initiation: AtomicInitiation,
    pub source_zone: Zone,
    /// Ordered costs committed during initiation. Printed spell mana is
    /// supplied by the face-bound mana model at runtime; this marker prevents
    /// an additional-cost transaction from bypassing it.
    pub initiation_costs: Vec<AtomicCost>,
    /// Resolution effects execute in vector order and are all owned by this
    /// transaction. A countered spell commits initiation costs but none of
    /// these effects.
    pub resolution: Vec<AtomicEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedAtomicTransaction {
    pub normalized_oracle: String,
    pub reasons: Vec<UnsupportedReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicInitiation {
    /// A mana ability activated from the source card in its owner's hand. It
    /// is not a spell cast and does not use the stack.
    HandManaAbility,
    /// Casting the source spell. Printed mana and all additional costs commit
    /// before any response or counter branch.
    CastSpell,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AtomicCost {
    PrintedManaCost,
    ExileSelf {
        from: Zone,
    },
    SacrificePermanents {
        filter: ObjectFilter,
        count: u16,
        commander_eligibility: CommanderEligibility,
    },
    /// The optional additional cost represented by the Bargain keyword.
    ///
    /// Every eligibility dimension stays explicit so runtime revalidation can
    /// distinguish an artifact, enchantment, or token from a convenient
    /// generic permanent sacrifice. The cost is chosen and paid while casting
    /// the source spell, before any response or counter branch.
    Bargain(AtomicBargainCost),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommanderEligibility {
    Include,
    Exclude,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AtomicBargainCost {
    pub player: ControllerRelation,
    pub timing: AtomicAdditionalCostTiming,
    pub optional: bool,
    pub from: Zone,
    pub count: u16,
    pub eligible_kinds: Vec<BargainSacrificeKind>,
    pub commander_eligibility: CommanderEligibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicAdditionalCostTiming {
    AsThisSpellIsCast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BargainSacrificeKind {
    Artifact,
    Enchantment,
    Token,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AtomicEffect {
    AddFixedMana(FixedManaProfile),
    /// Adds one fixed mana profile for every matching card across the exact
    /// set of graveyards. The card-name reference stays linked to the source
    /// object instead of baking a particular printing's name into the IR.
    AddManaPerNamedCardInGraveyards(NameLinkedGraveyardManaEffect),
    ConditionalManaReplacement(ConditionalManaReplacementEffect),
    SearchToHand(TutorEffect),
    RandomDiscard(RandomDiscardEffect),
    ShuffleLibrary(ShuffleLibraryEffect),
    /// A continuous effect that grants a complete mana ability to the dynamic
    /// set of lands controlled by the spell's controller until end of turn.
    TemporaryLandSacrificeManaGrant(TemporaryLandSacrificeManaGrantEffect),
    /// One linked search/exile/shuffle/conditional-cast/fallback movement.
    /// None of its attractive substeps may execute independently.
    BargainSearchCastOrHand(BargainSearchCastOrHandEffect),
    /// One linked search/reveal/opponent-choice/split-movement/shuffle effect.
    /// Opponent selection is a decision, never a random-discard proxy.
    OpponentChoiceSearchSplit(OpponentChoiceSearchSplitEffect),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TemporaryLandSacrificeManaGrantEffect {
    pub player: ControllerRelation,
    pub affected_zone: Zone,
    pub affected_filter: ObjectFilter,
    /// The set is evaluated continuously, so a matching land entering after
    /// this spell resolves also receives the granted ability.
    pub applies_to_future_matching_objects: bool,
    pub duration: AtomicEffectDuration,
    pub granted_ability: GrantedLandSacrificeManaAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicEffectDuration {
    UntilEndOfTurn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GrantedLandSacrificeManaAbility {
    pub kind: GrantedAbilityKind,
    pub source_zone: Zone,
    pub controller: ControllerRelation,
    pub cost: GrantedSelfCost,
    pub output: FixedManaProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrantedAbilityKind {
    ManaAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrantedSelfCost {
    SacrificeSelf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AtomicLibrarySearch {
    pub player: ControllerRelation,
    pub from: Zone,
    pub filter: TutorFilter,
    /// A printed fixed quantity is represented as equal minimum and maximum
    /// values. This distinguishes "three cards" from "up to three cards."
    pub minimum: u16,
    pub maximum: u16,
    pub reveal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BargainSearchCastOrHandEffect {
    pub search: AtomicLibrarySearch,
    pub searched_card: AtomicTrackedObject,
    pub initial_destination: Zone,
    pub face_down: bool,
    pub shuffle: AtomicShuffleStep,
    pub conditional_cast: BargainedConditionalCast,
    pub if_not_cast: AtomicTrackedObjectMovement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicTrackedObject {
    OnlyCardFoundByThisSearch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AtomicShuffleStep {
    pub player: ControllerRelation,
    pub timing: AtomicShuffleTiming,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicShuffleTiming {
    AfterInitialSearchMovementBeforeConditionalCast,
    AfterOpponentChoiceMovements,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BargainedConditionalCast {
    pub condition: AtomicCastPermissionCondition,
    pub card: AtomicTrackedObject,
    pub from: Zone,
    pub optional: bool,
    pub mana_value: AtomicManaValueCondition,
    pub cost_waiver: AtomicCastCostWaiver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicCastPermissionCondition {
    ThisSpellWasBargained,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AtomicManaValueCondition {
    pub subject: AtomicManaValueSubject,
    pub maximum: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicManaValueSubject {
    SpellAsCast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicCastCostWaiver {
    ManaCostOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AtomicTrackedObjectMovement {
    pub condition: AtomicMovementCondition,
    pub object: AtomicTrackedObject,
    pub from: Zone,
    pub to: Zone,
    pub recipient: ControllerRelation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicMovementCondition {
    NotCastByThisEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpponentChoiceSearchSplitEffect {
    pub search: AtomicLibrarySearch,
    pub chooser: AtomicSearchChooser,
    pub chosen_count: u16,
    pub chosen_destination: Zone,
    pub chosen_recipient: ControllerRelation,
    pub remainder_count: u16,
    pub remainder_destination: Zone,
    pub remainder_recipient: ControllerRelation,
    pub shuffle: AtomicShuffleStep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicSearchChooser {
    TargetOpponent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NameLinkedGraveyardManaEffect {
    pub mana_per_card: FixedManaProfile,
    pub card_name: AtomicCardNameReference,
    /// The runtime counts matching objects currently present in this scope
    /// when this ordered resolution effect executes.
    pub graveyards: AtomicGraveyardScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicCardNameReference {
    SourceCardName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AtomicGraveyardScope {
    /// Every player's graveyard, including each opponent's graveyard. This is
    /// deliberately not represented as only the source controller's zone.
    EachPlayerGraveyard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConditionalManaReplacementEffect {
    pub default: FixedManaProfile,
    pub condition: AtomicStateCondition,
    /// This output replaces `default`; it is never added to it.
    pub replacement: FixedManaProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AtomicStateCondition {
    CardsInZoneAtLeast {
        player: ControllerRelation,
        zone: Zone,
        count: u16,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RandomDiscardEffect {
    pub player: ControllerRelation,
    pub count: u16,
    pub from: Zone,
    pub to: Zone,
    pub selection: RandomSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RandomSelection {
    UniformAmongObjectsInZone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShuffleLibraryEffect {
    pub player: ControllerRelation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableAbility {
    /// Zero-based Oracle paragraph index. This is stable within one printing.
    pub clause_index: usize,
    /// Card self-references are replaced with "this permanent", so this field
    /// is useful for audit without making programs depend on card names.
    pub normalized_oracle: String,
    pub timing: AbilityTiming,
    pub costs: Vec<AbilityCost>,
    pub preconditions: Vec<AbilityPrecondition>,
    pub effects: Vec<AbilityEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedAbility {
    pub clause_index: usize,
    pub normalized_oracle: String,
    pub reasons: Vec<UnsupportedReason>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnsupportedReason {
    pub code: UnsupportedReasonCode,
    pub detail: String,
}

impl UnsupportedReason {
    fn new(code: UnsupportedReasonCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnsupportedReasonCode {
    EmptyClause,
    MultifaceRequiresFaceBinding,
    ModalOrChoiceAbility,
    UnrecognizedTiming,
    UnrecognizedTrigger,
    UnrecognizedCost,
    UnrecognizedEffect,
    MixedKnownAndUnknownEffect,
    UnsupportedQualifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AbilityTiming {
    DeckConstruction,
    SpellResolution,
    Activated { window: ActivationWindow },
    Triggered { event: TriggerEvent },
    StaticModifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationWindow {
    NormalPriority,
    InstantSpeedOnly,
    SorcerySpeedOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TriggerEvent {
    pub kind: TriggerEventKind,
    pub actor: ControllerRelation,
    pub object_filter: ObjectFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriggerEventKind {
    BeginningOfUpkeep,
    BeginningOfEndStep,
    SpellCast,
    /// The first event this turn that matches `object_filter`; for example,
    /// the first noncreature spell even when a creature spell was cast first.
    FirstFilteredSpellCastEachTurn,
    SecondSpellCastEachTurn,
    CardDraw,
    ThisSpellCast,
    PermanentEntersBattlefield,
    PermanentBecomesTapped,
    PermanentTappedForMana,
    EnchantedCreatureDealsDamageToOpponent,
    EquippedCreatureDies,
    CreatureDealsCombatDamageToPlayer,
    OneOrMoreCreaturesDealCombatDamageToPlayer,
    ChosenTypeCreatureEntersOrAttacks,
    OtherFlyingCreatureEntersBattlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControllerRelation {
    You,
    Opponent,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ObjectFilter {
    pub card_type: Option<CardType>,
    /// Additional printed card types matched disjunctively after the broad
    /// `card_type` constraint. An empty list imposes no additional type
    /// constraint. This keeps "a spell" distinct from the narrower
    /// "an artifact or enchantment spell" event.
    pub any_of_card_types: Vec<SpecificCardType>,
    pub excluded_card_type: Option<CardType>,
    pub subtype: Option<String>,
    pub excluded_subtype: Option<String>,
    pub nonland: bool,
    pub controller: Option<ControllerRelation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpecificCardType {
    Artifact,
    Enchantment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CardType {
    Artifact,
    Creature,
    Dragon,
    Land,
    Permanent,
    Spell,
    Card,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AbilityCost {
    Mana(ManaCost),
    TapSelf,
    TapPermanents {
        filter: ObjectFilter,
        count: u16,
        exclude_source: bool,
    },
    SacrificeSelf,
    SacrificeResource {
        resource: ResourceKind,
        count: u16,
    },
    Discard(DiscardCost),
    ExileFromGraveyard {
        count: u16,
        other: bool,
    },
    PayLife(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ManaCost {
    /// A canonical upper-case Oracle string plus its parsed payment profile.
    PrintedSymbols {
        oracle: String,
        profile: ManaCostProfile,
    },
    /// The printed mana cost of the individual graveyard card being granted
    /// escape. It cannot be reduced to the source permanent's own mana cost.
    GrantedCardPrintedManaCost,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct ManaCostProfile {
    pub generic: u16,
    pub white: u16,
    pub blue: u16,
    pub black: u16,
    pub red: u16,
    pub green: u16,
    pub colorless: u16,
    pub variable_x: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DiscardCost {
    EntireHand,
    Cards(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResourceKind {
    Treasure,
    Creature,
    Artifact,
    Token,
    TypedPermanent(ObjectFilter),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AbilityPrecondition {
    SourceZone(Zone),
    SourceUntapped,
    /// The event object is the same physical object as the ability source.
    /// This keeps “when this permanent enters” distinct from another
    /// matching permanent entering under the same controller.
    EventObjectIsSource,
    EventObjectMatches(ObjectFilter),
    ControllerCondition(ControllerConditionPrecondition),
    SourceCounterAtLeast {
        counter: CounterKind,
        count: u16,
    },
    ResourceAtLeast {
        resource: ResourceKind,
        count: u16,
    },
    UntappedResourceAtLeast {
        resource: ResourceKind,
        count: u16,
    },
    GraveyardCardsAtLeast {
        count: u16,
        other_than_cast_card: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Zone {
    Battlefield,
    Stack,
    Graveyard,
    Hand,
    Library,
    Exile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AbilityEffect {
    PartnerCommanderPairing(PartnerCommanderPairingEffect),
    AddMana(ManaEffect),
    AddManaWithRetention(LinkedManaRetentionEffect),
    Draw(CardAccessEffect),
    CumulativeUpkeep(CumulativeUpkeepEffect),
    LoseLife(LifeLossEffect),
    UnlessEventPlayerPays(UnlessEventPlayerPaysEffect),
    WholeHandDiscardThenDraw(WholeHandDiscardThenDrawEffect),
    RepeatableTopCardReveal(RepeatableTopCardRevealEffect),
    LinkedDelayedCardAccess(LinkedDelayedCardAccessEffect),
    LookAtTopAndSelect(LibrarySelectionEffect),
    ExhaustiveTopCardAccess(TopCardAccessEffect),
    Tutor(TutorEffect),
    VariableCreatureTutor(VariableCreatureTutorEffect),
    VariableCreatureOverrun(VariableCreatureOverrunEffect),
    Mill(MillEffect),
    CopyThisSpell(SpellCopyEffect),
    Tap(TargetSelector),
    Untap(TargetSelector),
    OptionalUntap(TargetSelector),
    CreateToken(TokenEffect),
    MoveZone(ZoneMovementEffect),
    GrantCastPermission(CastPermissionEffect),
    ModifyNonlandMana(NonlandManaModifier),
    /// One indivisible mana-ability resolution. The source's damage cannot be
    /// dropped while retaining only the mana output.
    AddManaAndSourceDamage(LinkedManaDamageEffect),
    ModifyPowerToughnessUntilEndOfTurn(PowerToughnessModifierEffect),
    ApplyStaticCreatureModifier(StaticCreatureModifierEffect),
    ReduceSpellCost(SpellCostReductionEffect),
    AlternativeSpellCost(AlternativeSpellCostEffect),
    AttachSourceToTarget(AttachSourceEffect),
    ChooseCreatureType(ChooseCreatureTypeEffect),
    SourceHasChosenCreatureType(SourceHasChosenCreatureTypeEffect),
    MultiplyTriggeredAbility(TriggerMultiplierEffect),
    BecomeMonarch(BecomeMonarchEffect),
    Conditional(ConditionalEffect),
    AddCounters(AddCountersEffect),
    GrantAllCreatureTypes(AllCreatureTypesEffect),
    DoesNotUntapDuringUntapStep(SelfUntapStepRestriction),
    SacrificeSelf,
}

/// The exact deck-construction permission granted by the Partner keyword.
/// Keeping the two-commander limit and mutual-keyword requirement explicit
/// prevents related but distinct pairing mechanics from satisfying the same
/// structural witness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PartnerCommanderPairingEffect {
    pub maximum_commanders: u8,
    pub both_commanders_must_have_partner: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PowerToughnessModifierEffect {
    pub target: TargetSelector,
    pub power_delta: i16,
    pub toughness_delta: i16,
}

/// One complete continuous effect printed on an Aura, Equipment, or permanent
/// that cares whether creatures are enchanted or equipped. Power/toughness
/// values and keyword grants stay in the same node so a partially recognized
/// clause can never execute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaticCreatureModifierEffect {
    pub target: StaticCreatureModifierTarget,
    pub power_delta: StaticModifierValue,
    pub toughness_delta: StaticModifierValue,
    pub granted_keywords: Vec<GrantedCreatureKeyword>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StaticCreatureModifierTarget {
    SourceCreature,
    CreaturesYouControl,
    OtherCreaturesYouControl,
    OtherCreaturesYouControlWithKeyword(GrantedCreatureKeyword),
    CreatureTokensYouControl,
    CreaturesYouControlOfChosenType,
    CreatureEnchantedBySource,
    CreatureEquippedBySource,
    CreaturesYouControlThatAreEnchantedOrEquipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StaticModifierValue {
    Fixed(i16),
    PermanentsYouControl {
        multiplier: i16,
        any_of_card_types: Vec<SpecificCardType>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrantedCreatureKeyword {
    CantBeBlocked,
    Deathtouch,
    DoubleStrike,
    FirstStrike,
    Flying,
    Haste,
    Hexproof,
    Indestructible,
    Lifelink,
    Menace,
    Reach,
    Shroud,
    Trample,
    Vigilance,
}

/// A generic-mana reduction applied while proposing one matching spell cast.
///
/// The affected spell and any board-state condition remain explicit so a
/// consumer cannot broaden a flying-creature reduction to every creature
/// spell or apply a source-spell reduction without checking its condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpellCostReductionEffect {
    pub affected_spell: SpellCostReductionScope,
    pub generic_mana_reduction: u16,
    pub condition: Option<SpellCostReductionCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpellCostReductionScope {
    CreatureSpellYouCastWithKeyword(GrantedCreatureKeyword),
    SourceSpell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpellCostReductionCondition {
    YouControlCreatureWithKeyword(GrantedCreatureKeyword),
}

/// One complete optional cost that replaces the source spell's printed mana
/// cost. Replacement identity and every payment component remain linked so a
/// consumer cannot retain the cheap mana payment while omitting the required
/// tapped permanents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AlternativeSpellCostEffect {
    pub replaces: ReplacedSpellCost,
    /// Components are retained in printed order and must all be payable before
    /// the alternative cost can be selected.
    pub payment: Vec<AlternativeSpellCostComponent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReplacedSpellCost {
    PrintedManaCost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AlternativeSpellCostComponent {
    Mana(ManaCost),
    TapUntappedPermanents {
        count: u16,
        filter: AlternativeSpellCostPermanentFilter,
    },
}

/// The dynamic objects eligible to pay a permanent-tapping component of an
/// alternative spell cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AlternativeSpellCostPermanentFilter {
    pub controller: ControllerRelation,
    pub card_type: CardType,
    pub required_keyword: GrantedCreatureKeyword,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttachSourceEffect {
    pub attachment_kind: AttachmentKind,
    pub target: ObjectFilter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AttachmentKind {
    Aura,
    Equipment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChooseCreatureTypeEffect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SourceHasChosenCreatureTypeEffect;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TriggerMultiplierEffect {
    pub event: TriggerMultiplierEvent,
    pub ability_source: TriggerAbilitySource,
    pub additional_times: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TriggerMultiplierEvent {
    PermanentEntering { any_of_card_types: Vec<CardType> },
    AnyTriggeredAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriggerAbilitySource {
    PermanentYouControl,
    OtherCreatureYouControlOfChosenType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AllCreatureTypesEffect {
    /// The source applies to creature permanents you control.
    pub creatures_you_control: bool,
    /// The source also applies to creature spells you control.
    pub creature_spells_you_control: bool,
    /// The source also applies to creature cards you own outside the
    /// battlefield. Keeping this scope explicit prevents an Equipment-style
    /// one-object type grant from satisfying the same structural witness.
    pub nonbattlefield_creature_cards_you_own: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelfUntapStepRestriction {
    pub target: TargetSelector,
    pub affected_player: ControllerRelation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManaEffect {
    pub amount: u16,
    pub kind: ManaKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManaKind {
    AnyOneColor,
    /// One color present among legendary creature and planeswalker
    /// permanents controlled by the activating player at resolution.
    AnyColorAmongLegendaryCreaturesAndPlaneswalkersYouControl,
    AnyTypeProducedByTriggeringPermanent,
    Fixed(FixedManaProfile),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkedManaDamageEffect {
    pub mana: ManaEffect,
    pub damage: SourceDamageEffect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceDamageEffect {
    pub amount: u16,
    pub recipient: ControllerRelation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FixedManaProfile {
    pub white: u16,
    pub blue: u16,
    pub black: u16,
    pub red: u16,
    pub green: u16,
    pub colorless: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CardAccessEffect {
    pub count: u16,
    pub optional: bool,
    pub unless_event_player_pays: Option<OptionalManaPayment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OptionalManaPayment {
    pub payer: PaymentPayer,
    pub amount: ManaPaymentAmount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaymentPayer {
    TriggeringPlayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ManaPaymentAmount {
    Fixed(ManaCost),
    SourcePower,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnlessEventPlayerPaysEffect {
    pub payment: OptionalManaPayment,
    pub if_not_paid: Vec<AbilityEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LifeLossEffect {
    pub player: ControllerRelation,
    pub amount: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControllerStateCondition {
    IsMonarch,
    LostNoLifeThisTurn,
}

/// An intervening “if” condition is evaluated both when the ability would
/// trigger and again when it resolves. It must not be weakened to a
/// resolution-only branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ControllerConditionPrecondition {
    pub controller: ControllerRelation,
    pub condition: ControllerStateCondition,
    pub check_when_triggering_and_resolving: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BecomeMonarchEffect {
    pub player: ControllerRelation,
}

/// A resolution-time branch. Intervening trigger conditions belong in
/// `AbilityPrecondition::ControllerCondition` instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConditionalEffect {
    pub controller: ControllerRelation,
    pub condition: ControllerStateCondition,
    pub if_true: Vec<AbilityEffect>,
    pub if_false: Vec<AbilityEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CounterTarget {
    SourcePermanent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddCountersEffect {
    pub target: CounterTarget,
    pub counter: CounterKind,
    pub count: u16,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkedManaRetentionEffect {
    pub mana: ManaEffect,
    pub retention: ManaRetention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManaRetention {
    ThroughStepsAndPhasesUntilEndOfTurn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CumulativeUpkeepEffect {
    pub counter: CounterKind,
    pub counters_added: u16,
    pub payment_per_counter: ManaCost,
    pub if_not_paid: Vec<AbilityEffect>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CounterKind {
    Age,
    Quest,
    Wish,
}

/// One indivisible whole-hand refresh. The executor must apply `discard`
/// before `draw` for every affected player and must reject or roll back the
/// complete effect if it cannot preserve that ordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WholeHandDiscardThenDrawEffect {
    pub players: AffectedPlayers,
    pub discard: WholeHandDiscardStep,
    pub draw: FixedDrawStep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AffectedPlayers {
    EachPlayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WholeHandDiscardStep {
    pub from: Zone,
    pub to: Zone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FixedDrawStep {
    pub count: u16,
    pub from: Zone,
    pub to: Zone,
}

/// A mandatory first top-card iteration followed by the optional repetition
/// of that complete iteration. The three fields in `iteration` are ordered:
/// reveal, move that revealed object, then lose life coupled to that exact
/// moved object's mana value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepeatableTopCardRevealEffect {
    pub player: ControllerRelation,
    pub iteration: TopCardRevealIteration,
    pub repetition: RepetitionPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopCardRevealIteration {
    pub reveal: RevealTopCardsStep,
    pub movement: RevealedCardMovementStep,
    pub life_loss: CoupledLifeLoss,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RevealTopCardsStep {
    pub count: u16,
    pub from: Zone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RevealedCardMovementStep {
    pub from: Zone,
    pub to: Zone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CoupledLifeLoss {
    ManaValueOfCardMovedByThisIteration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepetitionPolicy {
    OneMandatoryThenMayRepeatEntireIterationAnyNumberOfTimes,
}

/// One tracked object moves from the library to face-down exile now and from
/// exile to hand at the linked future event. Keeping both movements in one IR
/// node prevents either half from executing independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinkedDelayedCardAccessEffect {
    pub player: ControllerRelation,
    pub count: u16,
    pub from: Zone,
    pub source_position: LibraryPosition,
    pub intermediate: Zone,
    pub face_down: bool,
    pub tracked_object: DelayedObjectReference,
    pub delayed_event: DelayedEvent,
    pub destination: Zone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibraryPosition {
    Top,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DelayedObjectReference {
    CardMovedByThisEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DelayedEvent {
    BeginningOfYourNextEndStep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LibrarySelectionEffect {
    pub look_count: u16,
    pub selection_count: u16,
    pub optional: bool,
    pub filter: ObjectFilter,
    pub destination: Zone,
    pub remainder: LibraryRemainderPlacement,
}

/// One activation is guaranteed to move one card off the top of a finite
/// library after the optional scry decision: lands enter tapped and every
/// nonland card is drawn. This deliberately models the complete reviewed
/// branch structure rather than treating the Oracle paragraph as generic
/// card draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopCardAccessEffect {
    pub scry_count: u16,
    pub reveal: bool,
    pub land_destination: Zone,
    pub land_enters_tapped: bool,
    pub nonland_destination: Zone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibraryRemainderPlacement {
    BottomInRandomOrder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TutorEffect {
    pub from: Zone,
    pub destination: Zone,
    pub filter: TutorFilter,
    pub shuffle_after: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TutorFilter {
    AnyOf(Vec<ObjectFilter>),
}

/// Variable-mana creature search used by the reviewed combat conversion.
/// Library and graveyard access remain explicit because treating this as a
/// generic library tutor would lose both legal source zones and the X bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VariableCreatureTutorEffect {
    pub from_library: bool,
    pub from_graveyard: bool,
    pub destination: Zone,
    pub mana_value_at_most_x: bool,
    pub shuffle_if_library_searched: bool,
}

/// The complete conditional second sentence of the reviewed variable-mana
/// creature search. Every field is retained so a partial pump, missing haste,
/// or a different X threshold cannot satisfy a conversion witness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VariableCreatureOverrunEffect {
    pub minimum_x: u16,
    pub creatures_you_control: bool,
    pub power_bonus_equals_x: bool,
    pub toughness_bonus_equals_x: bool,
    pub grants_haste: bool,
    pub until_end_of_turn: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MillEffect {
    pub player: PlayerSelector,
    pub count: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlayerSelector {
    TargetPlayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SpellCopyEffect {
    pub count: SpellCopyCount,
    pub target_choice: CopyTargetChoice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpellCopyCount {
    EachSpellCastBeforeThisSpellThisTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CopyTargetChoice {
    MayChooseNewTargets,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TargetSelector {
    SelfPermanent,
    Enchanted(ObjectFilter),
    Target(ObjectFilter),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TokenEffect {
    pub count: u16,
    pub kind: TokenKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Treasure,
    Creature {
        power: i16,
        toughness: i16,
        description: String,
        keywords: Vec<CreatureTokenKeyword>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CreatureTokenKeyword {
    Flying,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ZoneMovementEffect {
    pub object: TargetSelector,
    pub from: Zone,
    pub to: Zone,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CastPermissionEffect {
    pub from: Zone,
    /// Cards in a graveyard have owners, not controllers.
    pub owner: ControllerRelation,
    pub filter: ObjectFilter,
    pub mechanic: CastPermissionKind,
    pub alternative_cost: Vec<AbilityCost>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CastPermissionKind {
    Escape,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NonlandManaModifier {
    pub additional_amount: u16,
    pub kind: ManaKind,
}

/// Compile reviewed Oracle templates without consulting a card-name table.
///
/// The supplied name is used only to normalize that card's own self-reference
/// in Oracle text. Two differently named cards with otherwise identical text
/// therefore compile to identical programs.
pub(crate) fn compile_face_bound_ability_program(
    root: OracleCardInput<'_>,
    faces: &[OracleCardFaceInput<'_>],
) -> ExecutableAbilityProgramV1 {
    let layout = root.layout.trim().to_ascii_lowercase();
    if !matches!(layout.as_str(), "modal_dfc" | "transform")
        || faces.len() != 2
        || faces.iter().any(|face| {
            face.name.trim().is_empty()
                || face.type_line.trim().is_empty()
                || face.oracle_text.contains("\n//\n")
        })
    {
        return compile_executable_ability_program(root);
    }

    let compiled_faces = faces
        .iter()
        .map(|face| {
            compile_executable_ability_program(OracleCardInput {
                name: face.name,
                // The exact face has already been selected from a retained
                // face record, so root-layout ambiguity no longer applies.
                layout: "normal",
                type_line: face.type_line,
                oracle_text: face.oracle_text,
                has_face_records: false,
            })
        })
        .collect::<Vec<_>>();

    let face_programs = compiled_faces
        .iter()
        .enumerate()
        .map(|(face_index, program)| {
            let has_executable_program = program.executable_abilities().next().is_some()
                || program.executable_necropotence_lifecycle().is_some()
                || program.executable_self_transfer_tutor_permanent().is_some()
                || program.executable_entry_linked_permanent().is_some()
                || program.executable_atomic_transaction().is_some();
            let is_complete = program.unsupported_abilities().next().is_none()
                && program.unsupported_necropotence_lifecycle().is_none()
                && program
                    .unsupported_self_transfer_tutor_permanent()
                    .is_none()
                && program.unsupported_entry_linked_permanent().is_none()
                && program.unsupported_atomic_transaction().is_none();
            let disposition = if face_index == 0 {
                FaceProgramDisposition::PrimaryCastable
            } else if layout == "modal_dfc" && has_executable_program && is_complete {
                FaceProgramDisposition::AlternateExecutable
            } else {
                FaceProgramDisposition::ReportOnlyUnsupported
            };
            BoundFaceAbilityProgram {
                face_index,
                name: faces[face_index].name.to_string(),
                type_line: faces[face_index].type_line.to_string(),
                disposition,
                abilities: program.abilities.clone(),
                necropotence_lifecycle: program.necropotence_lifecycle.clone(),
                self_transfer_tutor_permanent: program.self_transfer_tutor_permanent.clone(),
                entry_linked_permanent: program.entry_linked_permanent.clone(),
                atomic_transaction: program.atomic_transaction.clone(),
            }
        })
        .collect();

    let primary = &compiled_faces[0];
    ExecutableAbilityProgramV1 {
        version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        abilities: primary.abilities.clone(),
        necropotence_lifecycle: primary.necropotence_lifecycle.clone(),
        self_transfer_tutor_permanent: primary.self_transfer_tutor_permanent.clone(),
        entry_linked_permanent: primary.entry_linked_permanent.clone(),
        atomic_transaction: primary.atomic_transaction.clone(),
        face_programs,
    }
}

pub(crate) fn compile_executable_ability_program(
    input: OracleCardInput<'_>,
) -> ExecutableAbilityProgramV1 {
    if input_requires_face_binding(input) {
        return ExecutableAbilityProgramV1 {
            version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
            abilities: vec![AbilityCompilation::Unsupported(UnsupportedAbility {
                clause_index: 0,
                normalized_oracle: normalize_self_reference(
                    input.oracle_text,
                    input.name,
                    input.type_line,
                ),
                reasons: vec![UnsupportedReason::new(
                    UnsupportedReasonCode::MultifaceRequiresFaceBinding,
                    "The current executable program cannot bind combined root text and costs to exact card faces.",
                )],
            })],
            necropotence_lifecycle: None,
            self_transfer_tutor_permanent: None,
            entry_linked_permanent: None,
            atomic_transaction: None,
            face_programs: Vec::new(),
        };
    }

    let normalized_oracle =
        normalize_self_reference(input.oracle_text, input.name, input.type_line);
    if let Some(necropotence_lifecycle) =
        compile_necropotence_lifecycle(&normalized_oracle, input.type_line)
    {
        let abilities = match &necropotence_lifecycle {
            NecropotenceLifecycleCompilation::Executable(_) => Vec::new(),
            NecropotenceLifecycleCompilation::Unsupported(lifecycle) => {
                vec![AbilityCompilation::Unsupported(UnsupportedAbility {
                    clause_index: 0,
                    normalized_oracle: lifecycle.normalized_oracle.clone(),
                    reasons: lifecycle.reasons.clone(),
                })]
            }
        };
        return ExecutableAbilityProgramV1 {
            version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
            abilities,
            necropotence_lifecycle: Some(necropotence_lifecycle),
            self_transfer_tutor_permanent: None,
            entry_linked_permanent: None,
            atomic_transaction: None,
            face_programs: Vec::new(),
        };
    }
    if let Some(self_transfer_tutor_permanent) =
        compile_self_transfer_tutor_permanent(&normalized_oracle, input.type_line)
    {
        let abilities = match &self_transfer_tutor_permanent {
            SelfTransferTutorPermanentCompilation::Executable(_) => Vec::new(),
            SelfTransferTutorPermanentCompilation::Unsupported(permanent) => {
                vec![AbilityCompilation::Unsupported(UnsupportedAbility {
                    clause_index: 0,
                    normalized_oracle: permanent.normalized_oracle.clone(),
                    reasons: permanent.reasons.clone(),
                })]
            }
        };
        return ExecutableAbilityProgramV1 {
            version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
            abilities,
            necropotence_lifecycle: None,
            self_transfer_tutor_permanent: Some(self_transfer_tutor_permanent),
            entry_linked_permanent: None,
            atomic_transaction: None,
            face_programs: Vec::new(),
        };
    }
    if let Some(entry_linked_permanent) =
        compile_entry_linked_permanent(&normalized_oracle, input.type_line)
    {
        let abilities = match &entry_linked_permanent {
            EntryLinkedPermanentCompilation::Executable(_) => Vec::new(),
            EntryLinkedPermanentCompilation::Unsupported(permanent) => {
                vec![AbilityCompilation::Unsupported(UnsupportedAbility {
                    clause_index: 0,
                    normalized_oracle: permanent.normalized_oracle.clone(),
                    reasons: permanent.reasons.clone(),
                })]
            }
        };
        return ExecutableAbilityProgramV1 {
            version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
            abilities,
            necropotence_lifecycle: None,
            self_transfer_tutor_permanent: None,
            entry_linked_permanent: Some(entry_linked_permanent),
            atomic_transaction: None,
            face_programs: Vec::new(),
        };
    }
    if let Some(atomic_transaction) =
        compile_card_level_atomic_transaction(&normalized_oracle, input.type_line)
    {
        let abilities = match &atomic_transaction {
            AtomicTransactionCompilation::Executable(_) => Vec::new(),
            AtomicTransactionCompilation::Unsupported(transaction) => {
                vec![AbilityCompilation::Unsupported(UnsupportedAbility {
                    clause_index: 0,
                    normalized_oracle: transaction.normalized_oracle.clone(),
                    reasons: transaction.reasons.clone(),
                })]
            }
        };
        return ExecutableAbilityProgramV1 {
            version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
            abilities,
            necropotence_lifecycle: None,
            self_transfer_tutor_permanent: None,
            entry_linked_permanent: None,
            atomic_transaction: Some(atomic_transaction),
            face_programs: Vec::new(),
        };
    }
    let abilities = if is_variable_creature_tutor_overrun_candidate(&normalized_oracle) {
        // The reviewed variable-X tutor and its conditional overrun are one
        // spell-resolution ability in current Oracle text. Some historical
        // source records wrapped the second sentence onto a new line. Compile the
        // complete normalized text atomically so presentation wrapping cannot
        // change behavior and no supported-looking subset can execute alone.
        vec![compile_clause(0, &normalized_oracle, input.type_line)]
    } else {
        oracle_clauses(input.oracle_text)
            .into_iter()
            .enumerate()
            .map(|(clause_index, clause)| {
                let normalized = normalize_self_reference(clause, input.name, input.type_line);
                compile_clause(clause_index, &normalized, input.type_line)
            })
            .collect()
    };

    ExecutableAbilityProgramV1 {
        version: EXECUTABLE_ABILITY_PROGRAM_VERSION,
        abilities,
        necropotence_lifecycle: None,
        self_transfer_tutor_permanent: None,
        entry_linked_permanent: None,
        atomic_transaction: None,
        face_programs: Vec::new(),
    }
}

fn compile_necropotence_lifecycle(
    normalized_oracle: &str,
    type_line: &str,
) -> Option<NecropotenceLifecycleCompilation> {
    let lower = trim_terminal_period(&normalized_oracle.to_ascii_lowercase()).to_string();
    if !is_necropotence_lifecycle_candidate(&lower) {
        return None;
    }

    const CURRENT: &str = "skip your draw step. whenever you discard a card, exile that card from your graveyard. pay 1 life: exile the top card of your library face down. put that card into your hand at the beginning of your next end step";

    if !type_line_has_card_type(type_line, "enchantment") || lower != CURRENT {
        return Some(unsupported_necropotence_lifecycle(
            normalized_oracle,
            "The reviewed delayed-access lifecycle requires the complete enchantment root: skip your draw step; whenever you discard one card, exile that exact card from your graveyard; and pay exactly 1 life to move the real top library card face down to exile and the same object to your hand at the beginning of your next end step.",
        ));
    }

    Some(NecropotenceLifecycleCompilation::Executable(
        ExecutableNecropotenceLifecycle {
            normalized_oracle: normalized_oracle.to_string(),
            draw_step: NecropotenceDrawStepRestriction {
                player: ControllerRelation::You,
                step: TurnStep::Draw,
                procedure: StepProcedure::Skip,
            },
            discarded_card: NecropotenceDiscardedCardTrigger {
                player: ControllerRelation::You,
                event: NecropotenceDiscardEvent::WheneverYouDiscardOneCard,
                tracked_object: DiscardedObjectReference::CardDiscardedByThisTrigger,
                from: Zone::Graveyard,
                destination: Zone::Exile,
            },
            activation: NecropotenceCardAccessActivation {
                source_zone: Zone::Battlefield,
                window: ActivationWindow::NormalPriority,
                costs: vec![AbilityCost::PayLife(1)],
                access: LinkedDelayedCardAccessEffect {
                    player: ControllerRelation::You,
                    count: 1,
                    from: Zone::Library,
                    source_position: LibraryPosition::Top,
                    intermediate: Zone::Exile,
                    face_down: true,
                    tracked_object: DelayedObjectReference::CardMovedByThisEffect,
                    delayed_event: DelayedEvent::BeginningOfYourNextEndStep,
                    destination: Zone::Hand,
                },
            },
        },
    ))
}

fn is_necropotence_lifecycle_candidate(lower: &str) -> bool {
    lower.contains("skip your draw step")
        || lower.contains("whenever you discard a card")
        || lower.contains("exile that card from your graveyard")
        || (lower.contains("exile the top")
            && lower.contains("of your library")
            && lower.contains("face down"))
        || (lower.contains("put that card into your hand")
            && lower.contains("beginning of your")
            && lower.contains("end step"))
}

fn unsupported_necropotence_lifecycle(
    normalized_oracle: &str,
    detail: impl Into<String>,
) -> NecropotenceLifecycleCompilation {
    NecropotenceLifecycleCompilation::Unsupported(UnsupportedNecropotenceLifecycle {
        normalized_oracle: normalized_oracle.to_string(),
        reasons: vec![UnsupportedReason::new(
            UnsupportedReasonCode::MixedKnownAndUnknownEffect,
            detail,
        )],
    })
}

fn compile_self_transfer_tutor_permanent(
    normalized_oracle: &str,
    type_line: &str,
) -> Option<SelfTransferTutorPermanentCompilation> {
    let lower = trim_terminal_period(&normalized_oracle.to_ascii_lowercase()).to_string();
    if !is_self_transfer_tutor_permanent_candidate(&lower) {
        return None;
    }

    // Current Oracle wording after typed self-reference normalization.
    const CURRENT: &str = "this permanent enters with three wish counters on it. {1}, {t}, remove a wish counter from this permanent: search your library for a card, put it into your hand, then shuffle. an opponent gains control of this permanent. activate only during your turn";
    // Reviewed pre-2025 Oracle wording after printed-name normalization.
    const LEGACY: &str = "this permanent enters the battlefield with three wish counters on it. {1}, {t}, remove a wish counter from this permanent: search your library for a card, put it into your hand, then shuffle your library. an opponent gains control of this permanent. activate this ability only during your turn";
    // Some retained card-data snapshots already used the shorter activation
    // reminder while still carrying the older entry and shuffle wording.
    const LEGACY_SHORT_ACTIVATION: &str = "this permanent enters the battlefield with three wish counters on it. {1}, {t}, remove a wish counter from this permanent: search your library for a card, put it into your hand, then shuffle your library. an opponent gains control of this permanent. activate only during your turn";

    if !type_line_has_card_type(type_line, "artifact")
        || !matches!(lower.as_str(), CURRENT | LEGACY | LEGACY_SHORT_ACTIVATION)
    {
        return Some(unsupported_self_transfer_tutor_permanent(
            normalized_oracle,
            "The reviewed self-transfer tutor requires the complete artifact root: three entry wish counters; {1}, tap, and one wish counter removed as costs; an any-card library-to-hand search and shuffle; mandatory opponent control transfer; and activation only during your turn.",
        ));
    }

    Some(SelfTransferTutorPermanentCompilation::Executable(
        ExecutableSelfTransferTutorPermanent {
            normalized_oracle: normalized_oracle.to_string(),
            entry: SelfTransferTutorEntry {
                counter: CounterKind::Wish,
                count: 3,
            },
            activation: SelfTransferTutorActivation {
                source_zone: Zone::Battlefield,
                window: SelfTransferTutorActivationWindow::DuringYourTurnOnly,
                costs: vec![
                    SelfTransferTutorCost::Mana(ManaCost::PrintedSymbols {
                        oracle: "{1}".to_string(),
                        profile: ManaCostProfile {
                            generic: 1,
                            ..ManaCostProfile::default()
                        },
                    }),
                    SelfTransferTutorCost::TapSelf,
                    SelfTransferTutorCost::RemoveCounterFromSelf {
                        counter: CounterKind::Wish,
                        count: 1,
                    },
                ],
                resolution: vec![
                    SelfTransferTutorResolutionStep::SearchToHand(any_card_tutor(true)),
                    SelfTransferTutorResolutionStep::OpponentGainsControlOfSource,
                ],
            },
        },
    ))
}

fn is_self_transfer_tutor_permanent_candidate(lower: &str) -> bool {
    lower.contains("wish counter")
        || (lower.contains("search your library for a card")
            && lower.contains("an opponent gains control of this permanent"))
}

fn unsupported_self_transfer_tutor_permanent(
    normalized_oracle: &str,
    detail: impl Into<String>,
) -> SelfTransferTutorPermanentCompilation {
    SelfTransferTutorPermanentCompilation::Unsupported(UnsupportedSelfTransferTutorPermanent {
        normalized_oracle: normalized_oracle.to_string(),
        reasons: vec![UnsupportedReason::new(
            UnsupportedReasonCode::MixedKnownAndUnknownEffect,
            detail,
        )],
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryLinkedPermanentFamily {
    ImprintColoredCard,
    DiscardLandOrFailEntry,
}

fn compile_entry_linked_permanent(
    normalized_oracle: &str,
    type_line: &str,
) -> Option<EntryLinkedPermanentCompilation> {
    let lower = trim_terminal_period(&normalized_oracle.to_ascii_lowercase()).to_string();
    let family = entry_linked_permanent_family(&lower, type_line)?;
    Some(match family {
        EntryLinkedPermanentFamily::ImprintColoredCard => {
            compile_imprint_colored_card_permanent(&lower, normalized_oracle, type_line)
        }
        EntryLinkedPermanentFamily::DiscardLandOrFailEntry => {
            compile_discard_land_or_fail_entry_permanent(&lower, normalized_oracle, type_line)
        }
    })
}

fn entry_linked_permanent_family(
    lower: &str,
    type_line: &str,
) -> Option<EntryLinkedPermanentFamily> {
    if !type_line_has_card_type(type_line, "artifact") {
        return None;
    }
    if lower.starts_with("imprint - when this permanent enters")
        || lower.starts_with("imprint - when this permanent enters")
        || lower.contains("any of the exiled card's colors")
        || (lower.contains("when this permanent enters")
            && lower.contains("from your hand")
            && lower.contains("{t}: add"))
    {
        return Some(EntryLinkedPermanentFamily::ImprintColoredCard);
    }
    if lower.starts_with("if this permanent would enter")
        || (lower.contains("discard a land card instead")
            && lower.contains("put it into its owner's graveyard"))
    {
        return Some(EntryLinkedPermanentFamily::DiscardLandOrFailEntry);
    }
    None
}

fn compile_imprint_colored_card_permanent(
    lower: &str,
    normalized_oracle: &str,
    type_line: &str,
) -> EntryLinkedPermanentCompilation {
    const CURRENT: &str = "imprint - when this permanent enters, you may exile a nonartifact, nonland card from your hand. {t}: add one mana of any of the exiled card's colors";
    const LEGACY: &str = "imprint - when this permanent enters the battlefield, you may exile a nonartifact, nonland card from your hand. {t}: add one mana of any of the exiled card's colors";
    if !type_line_has_card_type(type_line, "artifact") || (lower != CURRENT && lower != LEGACY) {
        return unsupported_entry_linked_permanent(
            normalized_oracle,
            "The reviewed imprint permanent requires the complete optional nonartifact/nonland hand exile and the exact tap output linked to that exiled card's colors.",
        );
    }

    executable_entry_linked_permanent(
        normalized_oracle,
        PermanentEntryProcedure::OptionalImprint {
            filter: EntryLinkedCardFilter::NonartifactNonlandCard,
            from: Zone::Hand,
            to: Zone::Exile,
            link: LinkedEntryObject::CardExiledByThisEntry,
        },
        EntryLinkedManaOutput::AnyColorOfLinkedCard {
            linked: LinkedEntryObject::CardExiledByThisEntry,
        },
    )
}

fn compile_discard_land_or_fail_entry_permanent(
    lower: &str,
    normalized_oracle: &str,
    type_line: &str,
) -> EntryLinkedPermanentCompilation {
    const CURRENT: &str = "if this permanent would enter, you may discard a land card instead. if you do, put this permanent onto the battlefield. if you don't, put it into its owner's graveyard. {t}: add one mana of any color";
    const LEGACY: &str = "if this permanent would enter the battlefield, you may discard a land card instead. if you do, put this permanent onto the battlefield. if you don't, put it into its owner's graveyard. {t}: add one mana of any color";
    if !type_line_has_card_type(type_line, "artifact") || (lower != CURRENT && lower != LEGACY) {
        return unsupported_entry_linked_permanent(
            normalized_oracle,
            "The reviewed replacement-entry permanent requires an optional land discard, exact success/failure source destinations, and the complete any-color tap ability.",
        );
    }

    executable_entry_linked_permanent(
        normalized_oracle,
        PermanentEntryProcedure::DiscardOrFailToEnter {
            filter: EntryLinkedCardFilter::LandCard,
            discard_from: Zone::Hand,
            discard_to: Zone::Graveyard,
            success_destination: Zone::Battlefield,
            failure_destination: Zone::Graveyard,
        },
        EntryLinkedManaOutput::AnyOneColor,
    )
}

fn executable_entry_linked_permanent(
    normalized_oracle: &str,
    entry: PermanentEntryProcedure,
    output: EntryLinkedManaOutput,
) -> EntryLinkedPermanentCompilation {
    EntryLinkedPermanentCompilation::Executable(ExecutableEntryLinkedPermanent {
        normalized_oracle: normalized_oracle.to_string(),
        entry,
        mana_ability: EntryLinkedManaAbility {
            costs: vec![AbilityCost::TapSelf],
            preconditions: vec![
                AbilityPrecondition::SourceZone(Zone::Battlefield),
                AbilityPrecondition::SourceUntapped,
            ],
            output,
        },
    })
}

fn unsupported_entry_linked_permanent(
    normalized_oracle: &str,
    detail: impl Into<String>,
) -> EntryLinkedPermanentCompilation {
    EntryLinkedPermanentCompilation::Unsupported(UnsupportedEntryLinkedPermanent {
        normalized_oracle: normalized_oracle.to_string(),
        reasons: vec![UnsupportedReason::new(
            UnsupportedReasonCode::MixedKnownAndUnknownEffect,
            detail,
        )],
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicTransactionFamily {
    TemporaryLandSacrificeManaGrant,
    BargainSearchCastOrHand,
    OpponentChoiceSearchSplit,
    HandExileForFixedMana,
    AdditionalCastCost,
    NameLinkedGraveyardMana,
    ThresholdManaReplacement,
    SearchRandomDiscardShuffle,
}

fn compile_card_level_atomic_transaction(
    normalized_oracle: &str,
    type_line: &str,
) -> Option<AtomicTransactionCompilation> {
    let lower = trim_terminal_period(&normalized_oracle.to_ascii_lowercase()).to_string();
    let family = atomic_transaction_family(&lower, type_line)?;
    let compiled = match family {
        AtomicTransactionFamily::TemporaryLandSacrificeManaGrant => {
            compile_temporary_land_sacrifice_mana_grant(&lower, normalized_oracle, type_line)
        }
        AtomicTransactionFamily::BargainSearchCastOrHand => {
            compile_bargain_search_cast_or_hand(&lower, normalized_oracle, type_line)
        }
        AtomicTransactionFamily::OpponentChoiceSearchSplit => {
            compile_opponent_choice_search_split(&lower, normalized_oracle, type_line)
        }
        AtomicTransactionFamily::HandExileForFixedMana => {
            compile_hand_exile_for_fixed_mana(&lower, normalized_oracle)
        }
        AtomicTransactionFamily::AdditionalCastCost => {
            compile_additional_cast_cost_transaction(&lower, normalized_oracle, type_line)
        }
        AtomicTransactionFamily::NameLinkedGraveyardMana => {
            compile_name_linked_graveyard_mana(&lower, normalized_oracle, type_line)
        }
        AtomicTransactionFamily::ThresholdManaReplacement => {
            compile_threshold_mana_replacement(&lower, normalized_oracle, type_line)
        }
        AtomicTransactionFamily::SearchRandomDiscardShuffle => {
            compile_search_random_discard_shuffle(&lower, normalized_oracle, type_line)
        }
    };
    Some(compiled)
}

fn atomic_transaction_family(lower: &str, type_line: &str) -> Option<AtomicTransactionFamily> {
    let temporary_land_mana_signals = [
        lower.contains("until end of turn"),
        lower.contains("lands you control gain"),
        lower.contains("sacrifice this land"),
    ]
    .into_iter()
    .filter(|signal| *signal)
    .count();
    if temporary_land_mana_signals >= 2 {
        return Some(AtomicTransactionFamily::TemporaryLandSacrificeManaGrant);
    }
    if lower.starts_with("bargain ")
        || lower.contains("if this spell was bargained") && lower.contains("search your library")
    {
        return Some(AtomicTransactionFamily::BargainSearchCastOrHand);
    }
    let opponent_choice_search_signals = [
        lower.contains("search your library"),
        lower.contains("opponent chooses"),
        lower.contains("rest into your graveyard"),
    ]
    .into_iter()
    .filter(|signal| *signal)
    .count();
    if opponent_choice_search_signals >= 2 {
        return Some(AtomicTransactionFamily::OpponentChoiceSearchSplit);
    }
    if lower.starts_with("exile this permanent ") && lower.contains(':') && lower.contains("add ") {
        return Some(AtomicTransactionFamily::HandExileForFixedMana);
    }
    if is_instant_or_sorcery(type_line)
        && lower.starts_with("as an additional cost to cast this spell")
    {
        return Some(AtomicTransactionFamily::AdditionalCastCost);
    }
    if lower.starts_with("add ")
        && lower.contains("for each card named")
        && lower.contains("graveyard")
    {
        return Some(AtomicTransactionFamily::NameLinkedGraveyardMana);
    }
    if is_instant_or_sorcery(type_line) && lower.contains("threshold") && lower.contains("add ") {
        return Some(AtomicTransactionFamily::ThresholdManaReplacement);
    }
    if is_instant_or_sorcery(type_line)
        && lower.contains("search your library")
        && lower.contains("discard")
        && lower.contains("shuffle")
    {
        return Some(AtomicTransactionFamily::SearchRandomDiscardShuffle);
    }
    None
}

fn compile_temporary_land_sacrifice_mana_grant(
    lower: &str,
    normalized_oracle: &str,
    type_line: &str,
) -> AtomicTransactionCompilation {
    const EXACT: &str =
        "until end of turn, lands you control gain \"sacrifice this land: add {b}.\"";
    if !type_line.trim().eq_ignore_ascii_case("instant") || lower != EXACT {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed temporary land-mana transaction requires the exact instant-speed until-end-of-turn grant to lands you control, with sacrifice of that land producing exactly one black mana.",
        );
    }

    executable_atomic_transaction(
        normalized_oracle,
        AtomicInitiation::CastSpell,
        Zone::Hand,
        vec![AtomicCost::PrintedManaCost],
        vec![AtomicEffect::TemporaryLandSacrificeManaGrant(
            TemporaryLandSacrificeManaGrantEffect {
                player: ControllerRelation::You,
                affected_zone: Zone::Battlefield,
                affected_filter: ObjectFilter {
                    card_type: Some(CardType::Land),
                    controller: Some(ControllerRelation::You),
                    ..ObjectFilter::default()
                },
                applies_to_future_matching_objects: true,
                duration: AtomicEffectDuration::UntilEndOfTurn,
                granted_ability: GrantedLandSacrificeManaAbility {
                    kind: GrantedAbilityKind::ManaAbility,
                    source_zone: Zone::Battlefield,
                    controller: ControllerRelation::You,
                    cost: GrantedSelfCost::SacrificeSelf,
                    output: FixedManaProfile {
                        black: 1,
                        ..FixedManaProfile::default()
                    },
                },
            },
        )],
    )
}

fn compile_bargain_search_cast_or_hand(
    lower: &str,
    normalized_oracle: &str,
    type_line: &str,
) -> AtomicTransactionCompilation {
    const EXACT: &str = "bargain (you may sacrifice an artifact, enchantment, or token as you cast this spell.) search your library for a card, exile it face down, then shuffle. if this spell was bargained, you may cast the exiled card without paying its mana cost if that spell's mana value is 4 or less. put the exiled card into your hand if it wasn't cast this way";
    if !type_line.trim().eq_ignore_ascii_case("sorcery") || lower != EXACT {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed Bargain transaction requires its exact optional artifact, enchantment, or token sacrifice while casting; face-down search-to-exile and shuffle; optional bargained mana-value-four-or-less cast without paying only the mana cost; and exact not-cast fallback to hand.",
        );
    }

    executable_atomic_transaction(
        normalized_oracle,
        AtomicInitiation::CastSpell,
        Zone::Hand,
        vec![
            AtomicCost::PrintedManaCost,
            AtomicCost::Bargain(AtomicBargainCost {
                player: ControllerRelation::You,
                timing: AtomicAdditionalCostTiming::AsThisSpellIsCast,
                optional: true,
                from: Zone::Battlefield,
                count: 1,
                eligible_kinds: vec![
                    BargainSacrificeKind::Artifact,
                    BargainSacrificeKind::Enchantment,
                    BargainSacrificeKind::Token,
                ],
                commander_eligibility: CommanderEligibility::Include,
            }),
        ],
        vec![AtomicEffect::BargainSearchCastOrHand(
            BargainSearchCastOrHandEffect {
                search: AtomicLibrarySearch {
                    player: ControllerRelation::You,
                    from: Zone::Library,
                    filter: TutorFilter::AnyOf(vec![ObjectFilter {
                        card_type: Some(CardType::Card),
                        ..ObjectFilter::default()
                    }]),
                    minimum: 1,
                    maximum: 1,
                    reveal: false,
                },
                searched_card: AtomicTrackedObject::OnlyCardFoundByThisSearch,
                initial_destination: Zone::Exile,
                face_down: true,
                shuffle: AtomicShuffleStep {
                    player: ControllerRelation::You,
                    timing: AtomicShuffleTiming::AfterInitialSearchMovementBeforeConditionalCast,
                },
                conditional_cast: BargainedConditionalCast {
                    condition: AtomicCastPermissionCondition::ThisSpellWasBargained,
                    card: AtomicTrackedObject::OnlyCardFoundByThisSearch,
                    from: Zone::Exile,
                    optional: true,
                    mana_value: AtomicManaValueCondition {
                        subject: AtomicManaValueSubject::SpellAsCast,
                        maximum: 4,
                    },
                    cost_waiver: AtomicCastCostWaiver::ManaCostOnly,
                },
                if_not_cast: AtomicTrackedObjectMovement {
                    condition: AtomicMovementCondition::NotCastByThisEffect,
                    object: AtomicTrackedObject::OnlyCardFoundByThisSearch,
                    from: Zone::Exile,
                    to: Zone::Hand,
                    recipient: ControllerRelation::You,
                },
            },
        )],
    )
}

fn compile_opponent_choice_search_split(
    lower: &str,
    normalized_oracle: &str,
    type_line: &str,
) -> AtomicTransactionCompilation {
    const EXACT: &str = "search your library for three cards and reveal them. target opponent chooses one. put that card into your hand and the rest into your graveyard. then shuffle";
    if !type_line.trim().eq_ignore_ascii_case("instant") || lower != EXACT {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed opponent-choice search requires exactly three revealed library cards, one chosen by target opponent to its controller's hand, the other two to that controller's graveyard, and a final shuffle.",
        );
    }

    executable_atomic_transaction(
        normalized_oracle,
        AtomicInitiation::CastSpell,
        Zone::Hand,
        vec![AtomicCost::PrintedManaCost],
        vec![AtomicEffect::OpponentChoiceSearchSplit(
            OpponentChoiceSearchSplitEffect {
                search: AtomicLibrarySearch {
                    player: ControllerRelation::You,
                    from: Zone::Library,
                    filter: TutorFilter::AnyOf(vec![ObjectFilter {
                        card_type: Some(CardType::Card),
                        ..ObjectFilter::default()
                    }]),
                    minimum: 3,
                    maximum: 3,
                    reveal: true,
                },
                chooser: AtomicSearchChooser::TargetOpponent,
                chosen_count: 1,
                chosen_destination: Zone::Hand,
                chosen_recipient: ControllerRelation::You,
                remainder_count: 2,
                remainder_destination: Zone::Graveyard,
                remainder_recipient: ControllerRelation::You,
                shuffle: AtomicShuffleStep {
                    player: ControllerRelation::You,
                    timing: AtomicShuffleTiming::AfterOpponentChoiceMovements,
                },
            },
        )],
    )
}

fn compile_hand_exile_for_fixed_mana(
    lower: &str,
    normalized_oracle: &str,
) -> AtomicTransactionCompilation {
    let Some(effect_text) = lower.strip_prefix("exile this permanent from your hand: ") else {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed hand-mana transaction must exile its own source from its owner's hand.",
        );
    };
    let Some(output) = parse_fixed_mana_output(effect_text) else {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed hand-mana transaction requires one exact fixed-mana output.",
        );
    };
    if fixed_mana_total(output) != Some(1) {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed hand-mana transaction must add exactly one fixed mana.",
        );
    }

    executable_atomic_transaction(
        normalized_oracle,
        AtomicInitiation::HandManaAbility,
        Zone::Hand,
        vec![AtomicCost::ExileSelf { from: Zone::Hand }],
        vec![AtomicEffect::AddFixedMana(output)],
    )
}

fn compile_additional_cast_cost_transaction(
    lower: &str,
    normalized_oracle: &str,
    type_line: &str,
) -> AtomicTransactionCompilation {
    if !is_instant_or_sorcery(type_line) {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed additional-cost transaction must be an instant or sorcery spell.",
        );
    }
    let prefix = "as an additional cost to cast this spell, sacrifice a creature. ";
    let Some(resolution) = lower.strip_prefix(prefix) else {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed cast transaction requires exactly one noncommander creature sacrifice as an additional cost.",
        );
    };

    let resolution = if let Some(output) = parse_fixed_mana_output(resolution) {
        if output
            != (FixedManaProfile {
                black: 4,
                ..FixedManaProfile::default()
            })
        {
            return unsupported_atomic_transaction(
                normalized_oracle,
                "The reviewed sacrifice ritual must add exactly four black mana.",
            );
        }
        vec![AtomicEffect::AddFixedMana(output)]
    } else if resolution
        == "search your library for a card, put that card into your hand, then shuffle"
    {
        vec![AtomicEffect::SearchToHand(any_card_tutor(true))]
    } else {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed additional-cost transaction requires either the exact fixed-mana or exact any-card search resolution.",
        );
    };

    executable_atomic_transaction(
        normalized_oracle,
        AtomicInitiation::CastSpell,
        Zone::Hand,
        vec![
            AtomicCost::PrintedManaCost,
            AtomicCost::SacrificePermanents {
                filter: ObjectFilter {
                    card_type: Some(CardType::Creature),
                    controller: Some(ControllerRelation::You),
                    ..ObjectFilter::default()
                },
                count: 1,
                commander_eligibility: CommanderEligibility::Exclude,
            },
        ],
        resolution,
    )
}

fn compile_name_linked_graveyard_mana(
    lower: &str,
    normalized_oracle: &str,
    type_line: &str,
) -> AtomicTransactionCompilation {
    const EXACT: &str =
        "add {r}{r}, then add {r} for each card named this permanent in each graveyard";
    if !type_line_has_card_type(type_line, "sorcery") || lower != EXACT {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed name-linked ritual requires the exact two-red output followed by one red for each card sharing the source card's name in every player's graveyard.",
        );
    }

    executable_atomic_transaction(
        normalized_oracle,
        AtomicInitiation::CastSpell,
        Zone::Hand,
        vec![AtomicCost::PrintedManaCost],
        vec![
            AtomicEffect::AddFixedMana(FixedManaProfile {
                red: 2,
                ..FixedManaProfile::default()
            }),
            AtomicEffect::AddManaPerNamedCardInGraveyards(NameLinkedGraveyardManaEffect {
                mana_per_card: FixedManaProfile {
                    red: 1,
                    ..FixedManaProfile::default()
                },
                card_name: AtomicCardNameReference::SourceCardName,
                graveyards: AtomicGraveyardScope::EachPlayerGraveyard,
            }),
        ],
    )
}

fn compile_threshold_mana_replacement(
    lower: &str,
    normalized_oracle: &str,
    type_line: &str,
) -> AtomicTransactionCompilation {
    const EXACT: &str = "add {b}{b}{b}. threshold - add {b}{b}{b}{b}{b} instead if seven or more cards are in your graveyard";
    if !is_instant_or_sorcery(type_line) || lower != EXACT {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed threshold ritual requires the exact three-black default and five-black replacement at seven graveyard cards.",
        );
    }

    executable_atomic_transaction(
        normalized_oracle,
        AtomicInitiation::CastSpell,
        Zone::Hand,
        vec![AtomicCost::PrintedManaCost],
        vec![AtomicEffect::ConditionalManaReplacement(
            ConditionalManaReplacementEffect {
                default: FixedManaProfile {
                    black: 3,
                    ..FixedManaProfile::default()
                },
                condition: AtomicStateCondition::CardsInZoneAtLeast {
                    player: ControllerRelation::You,
                    zone: Zone::Graveyard,
                    count: 7,
                },
                replacement: FixedManaProfile {
                    black: 5,
                    ..FixedManaProfile::default()
                },
            },
        )],
    )
}

fn compile_search_random_discard_shuffle(
    lower: &str,
    normalized_oracle: &str,
    type_line: &str,
) -> AtomicTransactionCompilation {
    const EXACT: &str = "search your library for a card, put that card into your hand, discard a card at random, then shuffle";
    if !is_instant_or_sorcery(type_line) || lower != EXACT {
        return unsupported_atomic_transaction(
            normalized_oracle,
            "The reviewed search transaction must put one searched card into hand, discard uniformly at random from the resulting hand, then shuffle.",
        );
    }

    executable_atomic_transaction(
        normalized_oracle,
        AtomicInitiation::CastSpell,
        Zone::Hand,
        vec![AtomicCost::PrintedManaCost],
        vec![
            AtomicEffect::SearchToHand(any_card_tutor(false)),
            AtomicEffect::RandomDiscard(RandomDiscardEffect {
                player: ControllerRelation::You,
                count: 1,
                from: Zone::Hand,
                to: Zone::Graveyard,
                selection: RandomSelection::UniformAmongObjectsInZone,
            }),
            AtomicEffect::ShuffleLibrary(ShuffleLibraryEffect {
                player: ControllerRelation::You,
            }),
        ],
    )
}

fn executable_atomic_transaction(
    normalized_oracle: &str,
    initiation: AtomicInitiation,
    source_zone: Zone,
    initiation_costs: Vec<AtomicCost>,
    resolution: Vec<AtomicEffect>,
) -> AtomicTransactionCompilation {
    AtomicTransactionCompilation::Executable(ExecutableAtomicTransaction {
        normalized_oracle: normalized_oracle.to_string(),
        initiation,
        source_zone,
        initiation_costs,
        resolution,
    })
}

fn unsupported_atomic_transaction(
    normalized_oracle: &str,
    detail: impl Into<String>,
) -> AtomicTransactionCompilation {
    AtomicTransactionCompilation::Unsupported(UnsupportedAtomicTransaction {
        normalized_oracle: normalized_oracle.to_string(),
        reasons: vec![UnsupportedReason::new(
            UnsupportedReasonCode::MixedKnownAndUnknownEffect,
            detail,
        )],
    })
}

fn fixed_mana_total(profile: FixedManaProfile) -> Option<u16> {
    [
        profile.white,
        profile.blue,
        profile.black,
        profile.red,
        profile.green,
        profile.colorless,
    ]
    .into_iter()
    .try_fold(0u16, u16::checked_add)
}

fn any_card_tutor(shuffle_after: bool) -> TutorEffect {
    TutorEffect {
        from: Zone::Library,
        destination: Zone::Hand,
        filter: TutorFilter::AnyOf(vec![ObjectFilter {
            card_type: Some(CardType::Card),
            ..ObjectFilter::default()
        }]),
        shuffle_after,
    }
}

fn input_requires_face_binding(input: OracleCardInput<'_>) -> bool {
    if input.has_face_records
        || input.oracle_text.contains("\n//\n")
        || input.type_line.contains(" // ")
        || input.name.contains(" // ")
    {
        return true;
    }

    matches!(
        input.layout.trim().to_ascii_lowercase().as_str(),
        "split" | "flip" | "transform" | "modal_dfc" | "meld" | "adventure" | "reversible_card"
    )
}

fn oracle_clauses(oracle_text: &str) -> Vec<&str> {
    let clauses = oracle_text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if clauses.is_empty() {
        vec![oracle_text.trim()]
    } else {
        clauses
    }
}

fn normalize_self_reference(clause: &str, card_name: &str, type_line: &str) -> String {
    let mut normalized = clause
        .trim()
        .replace('’', "'")
        .replace(['\u{2013}', '\u{2014}'], "-");
    if !card_name.trim().is_empty() {
        let self_reference = RegexBuilder::new(&regex::escape(card_name.trim()))
            .case_insensitive(true)
            .build()
            .expect("escaped card names always form a valid regular expression");
        normalized = self_reference
            .replace_all(&normalized, "this permanent")
            .into_owned();
    }
    // Modern Oracle templating can refer to the source by its printed
    // permanent type ("this artifact", "this creature", and so on) instead
    // of repeating its card name. Canonicalize only types the source actually
    // has, so an unrelated object mentioned by a spell is not recast as self.
    for card_type in [
        "artifact",
        "creature",
        "enchantment",
        "land",
        "planeswalker",
        "battle",
    ] {
        if type_line
            .split(|character: char| !character.is_alphabetic())
            .any(|word| word.eq_ignore_ascii_case(card_type))
        {
            let typed_self = RegexBuilder::new(&format!(r"\bthis {card_type}\b"))
                .case_insensitive(true)
                .build()
                .expect("static card types always form a valid regular expression");
            normalized = typed_self
                .replace_all(&normalized, "this permanent")
                .into_owned();
        }
    }
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_ability_word_prefix(clause: &str) -> &str {
    let Some((label, rules_text)) = clause.split_once(" - ") else {
        return clause;
    };
    let label_is_ability_word = !label.trim().is_empty()
        && label.chars().all(|character| {
            character.is_alphabetic() || character.is_whitespace() || character == '\''
        });
    let lower_rules = rules_text.trim_start().to_ascii_lowercase();
    if label_is_ability_word && starts_trigger(&lower_rules) {
        rules_text.trim_start()
    } else {
        clause
    }
}

fn compile_clause(
    clause_index: usize,
    normalized_oracle: &str,
    type_line: &str,
) -> AbilityCompilation {
    let normalized_oracle = strip_ability_word_prefix(normalized_oracle);
    if normalized_oracle.is_empty() {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::EmptyClause,
                "The Oracle paragraph is empty.",
            ),
        );
    }
    if let Some(compilation) =
        compile_self_alternative_spell_cost_clause(clause_index, normalized_oracle, type_line)
    {
        return compilation;
    }
    if contains_modal_structure(normalized_oracle) {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::ModalOrChoiceAbility,
                "Modal and player-choice abilities are outside the current executable program.",
            ),
        );
    }

    let lower = normalized_oracle.to_ascii_lowercase();
    if let Some(compilation) =
        compile_become_monarch_self_entry_clause(clause_index, normalized_oracle, type_line)
    {
        return compilation;
    }
    if let Some(compilation) =
        compile_monarch_upkeep_token_branch_clause(clause_index, normalized_oracle, type_line)
    {
        return compilation;
    }
    if let Some(compilation) = compile_no_life_loss_opponent_end_step_counter_clause(
        clause_index,
        normalized_oracle,
        type_line,
    ) {
        return compilation;
    }
    if let Some(compilation) = compile_counter_threshold_token_activation_clause(
        clause_index,
        normalized_oracle,
        type_line,
    ) {
        return compilation;
    }
    if is_partner_deck_construction_template(&lower) {
        return compile_partner_deck_construction_clause(clause_index, normalized_oracle);
    }
    if is_escape_template(&lower) {
        return compile_escape_clause(clause_index, normalized_oracle);
    }
    if lower.starts_with("storm") {
        return compile_storm_clause(clause_index, normalized_oracle);
    }
    if is_all_creature_types_static_template(&lower) {
        return compile_all_creature_types_static_clause(clause_index, normalized_oracle);
    }
    if is_self_untap_step_restriction_template(&lower) {
        return compile_self_untap_step_restriction_clause(clause_index, normalized_oracle);
    }
    if let Some(compilation) = compile_creature_type_choice_clause(clause_index, normalized_oracle)
    {
        return compilation;
    }
    if let Some(compilation) = compile_trigger_multiplier_clause(clause_index, normalized_oracle) {
        return compilation;
    }
    if let Some(compilation) =
        compile_spell_cost_reduction_clause(clause_index, normalized_oracle, type_line)
    {
        return compilation;
    }
    if let Some(compilation) =
        compile_static_creature_modifier_clause(clause_index, normalized_oracle, type_line)
    {
        return compilation;
    }
    if lower.starts_with("equip ") {
        return compile_equip_clause(clause_index, normalized_oracle, type_line);
    }
    if lower.starts_with("cumulative upkeep") {
        return compile_cumulative_upkeep_clause(clause_index, normalized_oracle);
    }
    if let Some(compilation) =
        compile_reviewed_conditional_mana_clause(clause_index, normalized_oracle, type_line)
    {
        return compilation;
    }

    if starts_trigger(&lower) {
        return compile_triggered_clause(clause_index, normalized_oracle);
    }

    if let Some((cost_text, effect_text)) = normalized_oracle.split_once(':') {
        return compile_activated_clause(clause_index, normalized_oracle, cost_text, effect_text);
    }

    if is_instant_or_sorcery(type_line) {
        return compile_spell_clause(clause_index, normalized_oracle);
    }

    unsupported(
        clause_index,
        normalized_oracle,
        UnsupportedReason::new(
            UnsupportedReasonCode::UnrecognizedTiming,
            "The paragraph is not a supported spell-resolution, activated, triggered, or static template.",
        ),
    )
}

fn compile_become_monarch_self_entry_clause(
    clause_index: usize,
    normalized_oracle: &str,
    type_line: &str,
) -> Option<AbilityCompilation> {
    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    if !lower.starts_with("when this permanent enters") {
        return None;
    }
    if !matches!(
        lower,
        "when this permanent enters, you become the monarch"
            | "when this permanent enters the battlefield, you become the monarch"
    ) {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The reviewed self-entry monarch trigger requires the exact source event and controller-becomes-monarch effect.",
            ),
        ));
    }
    if !is_permanent_source_type(type_line) {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "A self-entry monarch trigger requires a permanent source.",
            ),
        ));
    }

    let event_filter = ObjectFilter {
        card_type: Some(CardType::Permanent),
        controller: Some(ControllerRelation::You),
        ..ObjectFilter::default()
    };
    Some(AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::Triggered {
            event: TriggerEvent {
                kind: TriggerEventKind::PermanentEntersBattlefield,
                actor: ControllerRelation::You,
                object_filter: event_filter.clone(),
            },
        },
        costs: Vec::new(),
        preconditions: vec![
            AbilityPrecondition::SourceZone(Zone::Battlefield),
            AbilityPrecondition::EventObjectIsSource,
            AbilityPrecondition::EventObjectMatches(event_filter),
        ],
        effects: vec![AbilityEffect::BecomeMonarch(BecomeMonarchEffect {
            player: ControllerRelation::You,
        })],
    }))
}

fn compile_monarch_upkeep_token_branch_clause(
    clause_index: usize,
    normalized_oracle: &str,
    type_line: &str,
) -> Option<AbilityCompilation> {
    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    const PREFIX: &str = "at the beginning of your upkeep, ";
    if !lower.starts_with(PREFIX) || !lower.contains("if you're the monarch") {
        return None;
    }
    if !is_permanent_source_type(type_line) {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "A controller-upkeep monarch branch requires a permanent source.",
            ),
        ));
    }

    let Some((ordinary_text, monarch_text)) = lower
        .strip_prefix(PREFIX)
        .and_then(|body| body.split_once(". if you're the monarch, "))
    else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The reviewed upkeep branch must create one typed token normally and a complete replacement token while its controller is the monarch.",
            ),
        ));
    };
    let Some(monarch_text) = monarch_text.strip_suffix(" instead") else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The monarch token branch must replace, rather than supplement, the ordinary token creation.",
            ),
        ));
    };
    let (Some(ordinary), Some(monarch)) = (
        parse_token_effect(ordinary_text),
        parse_token_effect(monarch_text),
    ) else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "Both sides of the monarch branch must be complete supported token-creation effects.",
            ),
        ));
    };
    if !nonzero_creature_token(&ordinary) || !nonzero_creature_token(&monarch) {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "The reviewed monarch branch requires nonzero creature-token creation on both branches.",
            ),
        ));
    }

    let event_filter = ObjectFilter {
        controller: Some(ControllerRelation::You),
        ..ObjectFilter::default()
    };
    Some(AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::Triggered {
            event: TriggerEvent {
                kind: TriggerEventKind::BeginningOfUpkeep,
                actor: ControllerRelation::You,
                object_filter: event_filter.clone(),
            },
        },
        costs: Vec::new(),
        preconditions: vec![
            AbilityPrecondition::SourceZone(Zone::Battlefield),
            AbilityPrecondition::EventObjectMatches(event_filter),
        ],
        effects: vec![AbilityEffect::Conditional(ConditionalEffect {
            controller: ControllerRelation::You,
            condition: ControllerStateCondition::IsMonarch,
            if_true: vec![AbilityEffect::CreateToken(monarch)],
            if_false: vec![AbilityEffect::CreateToken(ordinary)],
        })],
    }))
}

fn compile_no_life_loss_opponent_end_step_counter_clause(
    clause_index: usize,
    normalized_oracle: &str,
    type_line: &str,
) -> Option<AbilityCompilation> {
    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    const PREFIX: &str = "at the beginning of each opponent's end step, ";
    if !lower.starts_with(PREFIX) {
        return None;
    }
    if !is_permanent_source_type(type_line) {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "An opponent-end-step counter trigger requires a permanent source.",
            ),
        ));
    }
    let without_reminder = lower
        .strip_suffix(" (damage causes loss of life.)")
        .unwrap_or(lower);
    let effect = trim_terminal_period(without_reminder)
        .strip_prefix(PREFIX)
        .unwrap_or_default();
    let pattern = Regex::new(
        r"^if you didn't lose life this turn, you may put (?P<count>a|one|two|three|four|five|[0-9]+) (?P<counter>[a-z]+) counters? on this permanent$",
    )
    .expect("static no-life-loss counter pattern is valid");
    let Some(captures) = pattern.captures(effect) else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The reviewed opponent-end-step trigger requires the exact no-life-loss intervening condition and optional source-counter effect.",
            ),
        ));
    };
    if captures.name("counter").map(|value| value.as_str()) != Some("quest") {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "Only the typed quest-counter family is supported by this trigger.",
            ),
        ));
    }
    let Some(count) = captures
        .name("count")
        .and_then(|value| parse_number_word(value.as_str()))
        .filter(|count| *count > 0)
    else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "The quest-counter trigger must add a positive bounded count.",
            ),
        ));
    };

    let event_filter = ObjectFilter {
        controller: Some(ControllerRelation::Opponent),
        ..ObjectFilter::default()
    };
    Some(AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::Triggered {
            event: TriggerEvent {
                kind: TriggerEventKind::BeginningOfEndStep,
                actor: ControllerRelation::Opponent,
                object_filter: event_filter.clone(),
            },
        },
        costs: Vec::new(),
        preconditions: vec![
            AbilityPrecondition::SourceZone(Zone::Battlefield),
            AbilityPrecondition::EventObjectMatches(event_filter),
            AbilityPrecondition::ControllerCondition(ControllerConditionPrecondition {
                controller: ControllerRelation::You,
                condition: ControllerStateCondition::LostNoLifeThisTurn,
                check_when_triggering_and_resolving: true,
            }),
        ],
        effects: vec![AbilityEffect::AddCounters(AddCountersEffect {
            target: CounterTarget::SourcePermanent,
            counter: CounterKind::Quest,
            count,
            optional: true,
        })],
    }))
}

fn compile_counter_threshold_token_activation_clause(
    clause_index: usize,
    normalized_oracle: &str,
    type_line: &str,
) -> Option<AbilityCompilation> {
    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    if !lower.contains("activate only if this permanent has") || !lower.contains(" counters on it")
    {
        return None;
    }
    if !is_permanent_source_type(type_line) {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "A source-counter activation requires a permanent source.",
            ),
        ));
    }
    let pattern = Regex::new(
        r"^(?P<cost>(?:\{[0-9wubrgcx]+\})+): (?P<effect>.+)\. activate only if this permanent has (?P<threshold>a|one|two|three|four|five|[0-9]+) or more (?P<counter>[a-z]+) counters? on it$",
    )
    .expect("static counter-threshold activation pattern is valid");
    let Some(captures) = pattern.captures(lower) else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The reviewed activation must retain its complete mana cost, token effect, source-counter threshold, and at-least comparison.",
            ),
        ));
    };
    if captures.name("counter").map(|value| value.as_str()) != Some("quest") {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "Only a typed quest-counter threshold is supported by this activation.",
            ),
        ));
    }
    let Some(threshold) = captures
        .name("threshold")
        .and_then(|value| parse_number_word(value.as_str()))
        .filter(|count| *count > 0)
    else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "The source-counter threshold must be a positive bounded count.",
            ),
        ));
    };
    let mana = match captures
        .name("cost")
        .map(|value| parse_mana_cost(value.as_str()))
    {
        Some(Ok(mana)) => mana,
        Some(Err(reason)) => return Some(unsupported(clause_index, normalized_oracle, reason)),
        None => unreachable!("the threshold-activation pattern always captures mana"),
    };
    let Some(token) = captures
        .name("effect")
        .and_then(|value| parse_token_effect(value.as_str()))
        .filter(nonzero_creature_token)
    else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The counter-threshold activation must resolve as one complete supported creature-token effect.",
            ),
        ));
    };

    Some(AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::Activated {
            window: ActivationWindow::NormalPriority,
        },
        costs: vec![AbilityCost::Mana(mana)],
        preconditions: vec![
            AbilityPrecondition::SourceZone(Zone::Battlefield),
            AbilityPrecondition::SourceCounterAtLeast {
                counter: CounterKind::Quest,
                count: threshold,
            },
        ],
        effects: vec![AbilityEffect::CreateToken(token)],
    }))
}

fn nonzero_creature_token(token: &TokenEffect) -> bool {
    token.count > 0 && matches!(&token.kind, TokenKind::Creature { .. })
}

fn compile_reviewed_conditional_mana_clause(
    clause_index: usize,
    normalized_oracle: &str,
    type_line: &str,
) -> Option<AbilityCompilation> {
    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    let (expected_source_type, effects, extra_preconditions) = match lower {
        "{t}: add one mana of any color among legendary creatures and planeswalkers you control" => {
            (
                "artifact",
                vec![AbilityEffect::AddMana(ManaEffect {
                    amount: 1,
                    kind: ManaKind::AnyColorAmongLegendaryCreaturesAndPlaneswalkersYouControl,
                })],
                Vec::new(),
            )
        }
        "{t}: add one mana of any color. activate only if you control three or more artifacts"
        | "metalcraft - {t}: add one mana of any color. activate only if you control three or more artifacts" => {
            (
                "artifact",
                vec![AbilityEffect::AddMana(ManaEffect {
                    amount: 1,
                    kind: ManaKind::AnyOneColor,
                })],
                vec![AbilityPrecondition::ResourceAtLeast {
                    resource: ResourceKind::Artifact,
                    count: 3,
                }],
            )
        }
        "{t}: add {c}{c}. this permanent deals 2 damage to you" => (
            "land",
            vec![AbilityEffect::AddManaAndSourceDamage(
                LinkedManaDamageEffect {
                    mana: ManaEffect {
                        amount: 2,
                        kind: ManaKind::Fixed(FixedManaProfile {
                            colorless: 2,
                            ..FixedManaProfile::default()
                        }),
                    },
                    damage: SourceDamageEffect {
                        amount: 2,
                        recipient: ControllerRelation::You,
                    },
                },
            )],
            Vec::new(),
        ),
        "{t}: add one mana of any color. this permanent deals 3 damage to you" => (
            "land",
            vec![AbilityEffect::AddManaAndSourceDamage(
                LinkedManaDamageEffect {
                    mana: ManaEffect {
                        amount: 1,
                        kind: ManaKind::AnyOneColor,
                    },
                    damage: SourceDamageEffect {
                        amount: 3,
                        recipient: ControllerRelation::You,
                    },
                },
            )],
            Vec::new(),
        ),
        _ => return None,
    };

    if !type_line_has_card_type(type_line, expected_source_type) {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                format!(
                    "The reviewed conditional mana template requires a {expected_source_type} source."
                ),
            ),
        ));
    }

    let mut preconditions = vec![
        AbilityPrecondition::SourceZone(Zone::Battlefield),
        AbilityPrecondition::SourceUntapped,
    ];
    preconditions.extend(extra_preconditions);
    Some(AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::Activated {
            window: ActivationWindow::NormalPriority,
        },
        costs: vec![AbilityCost::TapSelf],
        preconditions,
        effects,
    }))
}

fn compile_activated_clause(
    clause_index: usize,
    normalized_oracle: &str,
    cost_text: &str,
    effect_and_restriction: &str,
) -> AbilityCompilation {
    let (effect_text, window) = match split_activation_restriction(effect_and_restriction) {
        Ok(parts) => parts,
        Err(reason) => return unsupported(clause_index, normalized_oracle, reason),
    };
    let costs = match parse_costs(cost_text) {
        Ok(costs) if !costs.is_empty() => costs,
        Ok(_) => {
            return unsupported(
                clause_index,
                normalized_oracle,
                UnsupportedReason::new(
                    UnsupportedReasonCode::UnrecognizedCost,
                    "An activated ability must have at least one fully recognized cost.",
                ),
            );
        }
        Err(reasons) => {
            return AbilityCompilation::Unsupported(UnsupportedAbility {
                clause_index,
                normalized_oracle: normalized_oracle.to_string(),
                reasons,
            });
        }
    };
    let effects = match parse_activated_effects(effect_text) {
        Ok(effects) => effects,
        Err(reasons) => {
            return AbilityCompilation::Unsupported(UnsupportedAbility {
                clause_index,
                normalized_oracle: normalized_oracle.to_string(),
                reasons,
            });
        }
    };
    if matches!(
        effects.as_slice(),
        [AbilityEffect::LinkedDelayedCardAccess(_)]
    ) && (costs != [AbilityCost::PayLife(1)] || window != ActivationWindow::NormalPriority)
    {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The reviewed linked delayed-access activation requires exactly Pay 1 life and no additional timing qualifier.",
            ),
        );
    }

    let mut preconditions = vec![AbilityPrecondition::SourceZone(Zone::Battlefield)];
    append_cost_preconditions(&costs, &mut preconditions);
    AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::Activated { window },
        costs,
        preconditions,
        effects,
    })
}

fn compile_triggered_clause(clause_index: usize, normalized_oracle: &str) -> AbilityCompilation {
    let Some((trigger_text, effect_text)) = normalized_oracle.split_once(',') else {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnrecognizedTrigger,
                "A supported trigger needs a recognized event followed by a comma.",
            ),
        );
    };
    let event = match parse_trigger_event(trigger_text) {
        Ok(event) => event,
        Err(reason) => return unsupported(clause_index, normalized_oracle, reason),
    };
    let effect_compilation = if event.kind == TriggerEventKind::BeginningOfUpkeep
        && event.actor == ControllerRelation::You
    {
        parse_upkeep_life_loss_creature_token_effects(effect_text)
            .map(Ok)
            .unwrap_or_else(|| parse_effects(effect_text))
    } else {
        parse_effects(effect_text)
    };
    let effects = match effect_compilation {
        Ok(effects) => effects,
        Err(reasons) => {
            return AbilityCompilation::Unsupported(UnsupportedAbility {
                clause_index,
                normalized_oracle: normalized_oracle.to_string(),
                reasons,
            });
        }
    };
    let mut preconditions = vec![
        AbilityPrecondition::SourceZone(Zone::Battlefield),
        AbilityPrecondition::EventObjectMatches(event.object_filter.clone()),
    ];

    if event.kind == TriggerEventKind::PermanentTappedForMana {
        let Some(AbilityEffect::AddMana(mana)) = effects.first() else {
            return unsupported(
                clause_index,
                normalized_oracle,
                UnsupportedReason::new(
                    UnsupportedReasonCode::UnsupportedQualifier,
                    "The supported nonland mana trigger must add mana.",
                ),
            );
        };
        if effects.len() != 1
            || mana.kind != ManaKind::AnyTypeProducedByTriggeringPermanent
            || mana.amount != 1
        {
            return unsupported(
                clause_index,
                normalized_oracle,
                UnsupportedReason::new(
                    UnsupportedReasonCode::UnsupportedQualifier,
                    "Only the reviewed plus-one, same-produced-type nonland mana trigger is executable.",
                ),
            );
        }
        preconditions.dedup();
        return AbilityCompilation::Executable(ExecutableAbility {
            clause_index,
            normalized_oracle: normalized_oracle.to_string(),
            timing: AbilityTiming::Triggered {
                event: event.clone(),
            },
            costs: Vec::new(),
            preconditions,
            effects: vec![AbilityEffect::ModifyNonlandMana(NonlandManaModifier {
                additional_amount: 1,
                kind: ManaKind::AnyTypeProducedByTriggeringPermanent,
            })],
        });
    }

    AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::Triggered { event },
        costs: Vec::new(),
        preconditions,
        effects,
    })
}

fn compile_cumulative_upkeep_clause(
    clause_index: usize,
    normalized_oracle: &str,
) -> AbilityCompilation {
    let lower = normalized_oracle.to_ascii_lowercase();
    let pattern = Regex::new(
        r"^cumulative upkeep (?P<cost>(?:\{[0-9WUBRGCXwubrgcx]+\})+) \(at the beginning of your upkeep, put an age counter on this permanent, then sacrifice it unless you pay its upkeep cost for each age counter on it\.\)$",
    )
    .expect("static cumulative-upkeep pattern is valid");
    let Some(captures) = pattern.captures(&lower) else {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "Cumulative upkeep is executable only with its complete reviewed age-counter, scaled-payment, and sacrifice procedure.",
            ),
        );
    };
    let Some(cost_text) = captures.name("cost") else {
        unreachable!("the cumulative-upkeep pattern always captures its payment")
    };
    let payment_per_counter = match parse_mana_cost(cost_text.as_str()) {
        Ok(cost) => cost,
        Err(reason) => return unsupported(clause_index, normalized_oracle, reason),
    };
    let event = TriggerEvent {
        kind: TriggerEventKind::BeginningOfUpkeep,
        actor: ControllerRelation::You,
        object_filter: ObjectFilter {
            controller: Some(ControllerRelation::You),
            ..ObjectFilter::default()
        },
    };
    AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::Triggered {
            event: event.clone(),
        },
        costs: Vec::new(),
        preconditions: vec![
            AbilityPrecondition::SourceZone(Zone::Battlefield),
            AbilityPrecondition::EventObjectMatches(event.object_filter),
        ],
        effects: vec![AbilityEffect::CumulativeUpkeep(CumulativeUpkeepEffect {
            counter: CounterKind::Age,
            counters_added: 1,
            payment_per_counter,
            if_not_paid: vec![AbilityEffect::SacrificeSelf],
        })],
    })
}

fn compile_spell_clause(clause_index: usize, normalized_oracle: &str) -> AbilityCompilation {
    match parse_spell_effects(normalized_oracle) {
        Ok(effects) => AbilityCompilation::Executable(ExecutableAbility {
            clause_index,
            normalized_oracle: normalized_oracle.to_string(),
            timing: AbilityTiming::SpellResolution,
            costs: Vec::new(),
            preconditions: vec![AbilityPrecondition::SourceZone(Zone::Stack)],
            effects,
        }),
        Err(reasons) => AbilityCompilation::Unsupported(UnsupportedAbility {
            clause_index,
            normalized_oracle: normalized_oracle.to_string(),
            reasons,
        }),
    }
}

fn compile_escape_clause(clause_index: usize, normalized_oracle: &str) -> AbilityCompilation {
    let alternative_cost = vec![
        AbilityCost::Mana(ManaCost::GrantedCardPrintedManaCost),
        AbilityCost::ExileFromGraveyard {
            count: 3,
            other: true,
        },
    ];
    AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::StaticModifier,
        costs: Vec::new(),
        // The permission exists whenever the source is on the battlefield.
        // Graveyard sufficiency belongs to each granted cast's alternative
        // cost and may become true later in the same turn.
        preconditions: vec![AbilityPrecondition::SourceZone(Zone::Battlefield)],
        effects: vec![AbilityEffect::GrantCastPermission(CastPermissionEffect {
            from: Zone::Graveyard,
            owner: ControllerRelation::You,
            filter: ObjectFilter {
                nonland: true,
                ..ObjectFilter::default()
            },
            mechanic: CastPermissionKind::Escape,
            alternative_cost,
        })],
    })
}

fn compile_partner_deck_construction_clause(
    clause_index: usize,
    normalized_oracle: &str,
) -> AbilityCompilation {
    AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::DeckConstruction,
        costs: Vec::new(),
        preconditions: Vec::new(),
        effects: vec![AbilityEffect::PartnerCommanderPairing(
            PartnerCommanderPairingEffect {
                maximum_commanders: 2,
                both_commanders_must_have_partner: true,
            },
        )],
    })
}

fn compile_storm_clause(clause_index: usize, normalized_oracle: &str) -> AbilityCompilation {
    let lower = normalized_oracle.to_ascii_lowercase();
    if lower
        != "storm (when you cast this spell, copy it for each spell cast before it this turn. you may choose new targets for the copies.)"
    {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "Storm is executable only when its reviewed copy count and optional new-target choice are both explicit.",
            ),
        );
    }

    let spell_filter = ObjectFilter {
        card_type: Some(CardType::Spell),
        controller: Some(ControllerRelation::You),
        ..ObjectFilter::default()
    };
    AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::Triggered {
            event: TriggerEvent {
                kind: TriggerEventKind::ThisSpellCast,
                actor: ControllerRelation::You,
                object_filter: spell_filter.clone(),
            },
        },
        costs: Vec::new(),
        preconditions: vec![
            AbilityPrecondition::SourceZone(Zone::Stack),
            AbilityPrecondition::EventObjectMatches(spell_filter),
        ],
        effects: vec![AbilityEffect::CopyThisSpell(SpellCopyEffect {
            count: SpellCopyCount::EachSpellCastBeforeThisSpellThisTurn,
            target_choice: CopyTargetChoice::MayChooseNewTargets,
        })],
    })
}

fn compile_all_creature_types_static_clause(
    clause_index: usize,
    normalized_oracle: &str,
) -> AbilityCompilation {
    AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::StaticModifier,
        costs: Vec::new(),
        preconditions: vec![AbilityPrecondition::SourceZone(Zone::Battlefield)],
        effects: vec![AbilityEffect::GrantAllCreatureTypes(
            AllCreatureTypesEffect {
                creatures_you_control: true,
                creature_spells_you_control: true,
                nonbattlefield_creature_cards_you_own: true,
            },
        )],
    })
}

fn compile_self_untap_step_restriction_clause(
    clause_index: usize,
    normalized_oracle: &str,
) -> AbilityCompilation {
    AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::StaticModifier,
        costs: Vec::new(),
        preconditions: vec![AbilityPrecondition::SourceZone(Zone::Battlefield)],
        effects: vec![AbilityEffect::DoesNotUntapDuringUntapStep(
            SelfUntapStepRestriction {
                target: TargetSelector::SelfPermanent,
                affected_player: ControllerRelation::You,
            },
        )],
    })
}

fn compile_static_creature_modifier_clause(
    clause_index: usize,
    normalized_oracle: &str,
    type_line: &str,
) -> Option<AbilityCompilation> {
    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);

    if let Some(body) = lower.strip_prefix("creatures you control get ") {
        return Some(compile_team_creature_modifier(
            clause_index,
            normalized_oracle,
            StaticCreatureModifierTarget::CreaturesYouControl,
            body,
        ));
    }
    if let Some(body) = lower.strip_prefix("other creatures you control get ") {
        return Some(compile_team_creature_modifier(
            clause_index,
            normalized_oracle,
            StaticCreatureModifierTarget::OtherCreaturesYouControl,
            body,
        ));
    }
    if let Some(body) = lower.strip_prefix("other creatures you control have ") {
        return Some(compile_team_creature_keywords(
            clause_index,
            normalized_oracle,
            StaticCreatureModifierTarget::OtherCreaturesYouControl,
            body,
        ));
    }
    if let Some(body) = lower.strip_prefix("other creatures you control with flying get ") {
        return Some(compile_team_creature_modifier(
            clause_index,
            normalized_oracle,
            StaticCreatureModifierTarget::OtherCreaturesYouControlWithKeyword(
                GrantedCreatureKeyword::Flying,
            ),
            body,
        ));
    }
    if let Some(body) = lower.strip_prefix("other creatures you control with flying have ") {
        return Some(compile_team_creature_keywords(
            clause_index,
            normalized_oracle,
            StaticCreatureModifierTarget::OtherCreaturesYouControlWithKeyword(
                GrantedCreatureKeyword::Flying,
            ),
            body,
        ));
    }
    if let Some(body) = lower.strip_prefix("creature tokens you control get ") {
        return Some(compile_team_creature_modifier(
            clause_index,
            normalized_oracle,
            StaticCreatureModifierTarget::CreatureTokensYouControl,
            body,
        ));
    }
    if let Some(body) = lower.strip_prefix("creatures you control of the chosen type get ") {
        return Some(compile_team_creature_modifier(
            clause_index,
            normalized_oracle,
            StaticCreatureModifierTarget::CreaturesYouControlOfChosenType,
            body,
        ));
    }

    if lower == "creatures you control that are enchanted or equipped have double strike" {
        return Some(executable_static_creature_modifier(
            clause_index,
            normalized_oracle,
            StaticCreatureModifierTarget::CreaturesYouControlThatAreEnchantedOrEquipped,
            StaticModifierValue::Fixed(0),
            StaticModifierValue::Fixed(0),
            vec![GrantedCreatureKeyword::DoubleStrike],
        ));
    }

    let (target, body, required_subtype) =
        if let Some(body) = lower.strip_prefix("enchanted creature ") {
            (
                StaticCreatureModifierTarget::CreatureEnchantedBySource,
                body,
                "aura",
            )
        } else if let Some(body) = lower.strip_prefix("equipped creature ") {
            (
                StaticCreatureModifierTarget::CreatureEquippedBySource,
                body,
                "equipment",
            )
        } else {
            return None;
        };

    if !type_line_has_card_type(type_line, required_subtype) {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                format!(
                    "The attached-creature static template requires a source with the {required_subtype} subtype."
                ),
            ),
        ));
    }

    let Some((power_delta, toughness_delta, granted_keywords)) =
        parse_attached_creature_modifier_body(body)
    else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The attached-creature clause must contain one complete reviewed power/toughness modifier and/or an exact supported keyword grant.",
            ),
        ));
    };

    Some(executable_static_creature_modifier(
        clause_index,
        normalized_oracle,
        target,
        power_delta,
        toughness_delta,
        granted_keywords,
    ))
}

fn compile_team_creature_modifier(
    clause_index: usize,
    normalized_oracle: &str,
    target: StaticCreatureModifierTarget,
    body: &str,
) -> AbilityCompilation {
    let singular = format!("gets {}", body.replace(" and have ", " and has "));
    let Some((power_delta, toughness_delta, granted_keywords)) =
        parse_attached_creature_modifier_body(&singular)
    else {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The team modifier must contain one exact additive power/toughness modifier and only supported keyword grants.",
            ),
        );
    };
    executable_static_creature_modifier(
        clause_index,
        normalized_oracle,
        target,
        power_delta,
        toughness_delta,
        granted_keywords,
    )
}

fn compile_team_creature_keywords(
    clause_index: usize,
    normalized_oracle: &str,
    target: StaticCreatureModifierTarget,
    body: &str,
) -> AbilityCompilation {
    let Some(granted_keywords) = parse_granted_creature_keywords(body) else {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The team keyword grant contains an unsupported or partial keyword.",
            ),
        );
    };
    executable_static_creature_modifier(
        clause_index,
        normalized_oracle,
        target,
        StaticModifierValue::Fixed(0),
        StaticModifierValue::Fixed(0),
        granted_keywords,
    )
}

fn compile_creature_type_choice_clause(
    clause_index: usize,
    normalized_oracle: &str,
) -> Option<AbilityCompilation> {
    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    let effect = match lower {
        "as this permanent enters, choose a creature type"
        | "as this permanent enters the battlefield, choose a creature type" => {
            AbilityEffect::ChooseCreatureType(ChooseCreatureTypeEffect)
        }
        "this permanent is the chosen type in addition to its other types" => {
            AbilityEffect::SourceHasChosenCreatureType(SourceHasChosenCreatureTypeEffect)
        }
        _ => return None,
    };
    Some(AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::StaticModifier,
        costs: Vec::new(),
        preconditions: vec![AbilityPrecondition::SourceZone(Zone::Battlefield)],
        effects: vec![effect],
    }))
}

fn compile_trigger_multiplier_clause(
    clause_index: usize,
    normalized_oracle: &str,
) -> Option<AbilityCompilation> {
    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    let (event, ability_source) = match lower {
        "if an artifact or creature entering causes a triggered ability of a permanent you control to trigger, that ability triggers an additional time" => {
            (
                TriggerMultiplierEvent::PermanentEntering {
                    any_of_card_types: vec![CardType::Artifact, CardType::Creature],
                },
                TriggerAbilitySource::PermanentYouControl,
            )
        }
        "if a triggered ability of another creature you control of the chosen type triggers, it triggers an additional time" => {
            (
                TriggerMultiplierEvent::AnyTriggeredAbility,
                TriggerAbilitySource::OtherCreatureYouControlOfChosenType,
            )
        }
        _ => return None,
    };
    Some(AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::StaticModifier,
        costs: Vec::new(),
        preconditions: vec![AbilityPrecondition::SourceZone(Zone::Battlefield)],
        effects: vec![AbilityEffect::MultiplyTriggeredAbility(
            TriggerMultiplierEffect {
                event,
                ability_source,
                additional_times: 1,
            },
        )],
    }))
}

fn compile_spell_cost_reduction_clause(
    clause_index: usize,
    normalized_oracle: &str,
    type_line: &str,
) -> Option<AbilityCompilation> {
    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    const FLYING_CREATURE_SPELLS: &str =
        "creature spells with flying you cast cost {1} less to cast";
    const CONDITIONAL_SOURCE_SPELL: &str =
        "this spell costs {1} less to cast if you control a creature with flying";

    let (affected_spell, condition, source_zone) = match lower {
        FLYING_CREATURE_SPELLS => {
            if !is_permanent_source_type(type_line) {
                return Some(unsupported(
                    clause_index,
                    normalized_oracle,
                    UnsupportedReason::new(
                        UnsupportedReasonCode::UnsupportedQualifier,
                        "The flying-creature spell reduction requires a permanent source on the battlefield.",
                    ),
                ));
            }
            (
                SpellCostReductionScope::CreatureSpellYouCastWithKeyword(
                    GrantedCreatureKeyword::Flying,
                ),
                None,
                Some(Zone::Battlefield),
            )
        }
        CONDITIONAL_SOURCE_SPELL => {
            if !is_nonland_spell_source_type(type_line) {
                return Some(unsupported(
                    clause_index,
                    normalized_oracle,
                    UnsupportedReason::new(
                        UnsupportedReasonCode::UnsupportedQualifier,
                        "The conditional source-spell reduction requires a castable nonland spell source.",
                    ),
                ));
            }
            (
                SpellCostReductionScope::SourceSpell,
                Some(SpellCostReductionCondition::YouControlCreatureWithKeyword(
                    GrantedCreatureKeyword::Flying,
                )),
                None,
            )
        }
        _ => {
            let is_reviewed_family_candidate = lower.starts_with("creature spells with flying ")
                && lower.contains(" less to cast")
                || lower.starts_with("this spell costs ")
                    && lower.contains(" less to cast if you control ")
                    && lower.contains(" with flying");
            if !is_reviewed_family_candidate {
                return None;
            }
            return Some(unsupported(
                clause_index,
                normalized_oracle,
                UnsupportedReason::new(
                    UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                    "The reviewed generic spell-cost reduction requires either exactly one generic mana from creature spells with flying you cast, or exactly one generic mana from the source spell while you control a creature with flying.",
                ),
            ));
        }
    };

    Some(AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::StaticModifier,
        costs: Vec::new(),
        preconditions: source_zone
            .map(AbilityPrecondition::SourceZone)
            .into_iter()
            .collect(),
        effects: vec![AbilityEffect::ReduceSpellCost(SpellCostReductionEffect {
            affected_spell,
            generic_mana_reduction: 1,
            condition,
        })],
    }))
}

fn compile_self_alternative_spell_cost_clause(
    clause_index: usize,
    normalized_oracle: &str,
    type_line: &str,
) -> Option<AbilityCompilation> {
    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    let is_reviewed_family_candidate = lower.starts_with("you may pay ")
        && lower.contains(" and tap ")
        && lower.contains(" rather than pay ")
        && lower.contains("this spell")
        && lower.ends_with("mana cost");
    if !is_reviewed_family_candidate {
        return None;
    }
    if !is_nonland_spell_source_type(type_line) {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "A self alternative spell cost requires a castable nonland spell source.",
            ),
        ));
    }

    let pattern = Regex::new(
        r"^you may pay (?P<mana>(?:\{[^{}]+\})+) and tap (?P<count>[a-z0-9]+) untapped creatures you control with (?P<keyword>[a-z ]+) rather than pay this spell's mana cost$",
    )
    .expect("static self alternative spell-cost pattern is valid");
    let Some(captures) = pattern.captures(lower) else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The reviewed self alternative spell cost requires one mana payment plus a fixed number of untapped creatures you control sharing one supported keyword, replacing the printed mana cost.",
            ),
        ));
    };

    let mana_text = captures
        .name("mana")
        .expect("the alternative-cost pattern always captures mana")
        .as_str();
    let mana = match parse_mana_cost(mana_text) {
        Ok(mana) => mana,
        Err(reason) => return Some(unsupported(clause_index, normalized_oracle, reason)),
    };
    let count_text = captures
        .name("count")
        .expect("the alternative-cost pattern always captures a count")
        .as_str();
    let Some(count) = parse_number_word(count_text).filter(|count| *count > 0) else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnrecognizedCost,
                "The alternative spell cost requires a positive fixed number of tapped permanents.",
            ),
        ));
    };
    let keyword_text = captures
        .name("keyword")
        .expect("the alternative-cost pattern always captures a keyword")
        .as_str();
    let Some(required_keyword) = parse_granted_creature_keywords(keyword_text)
        .filter(|keywords| keywords.len() == 1)
        .and_then(|keywords| keywords.into_iter().next())
    else {
        return Some(unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "The alternative spell cost requires exactly one supported creature keyword.",
            ),
        ));
    };

    Some(AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::StaticModifier,
        costs: Vec::new(),
        preconditions: Vec::new(),
        effects: vec![AbilityEffect::AlternativeSpellCost(
            AlternativeSpellCostEffect {
                replaces: ReplacedSpellCost::PrintedManaCost,
                payment: vec![
                    AlternativeSpellCostComponent::Mana(mana),
                    AlternativeSpellCostComponent::TapUntappedPermanents {
                        count,
                        filter: AlternativeSpellCostPermanentFilter {
                            controller: ControllerRelation::You,
                            card_type: CardType::Creature,
                            required_keyword,
                        },
                    },
                ],
            },
        )],
    }))
}

fn parse_attached_creature_modifier_body(
    body: &str,
) -> Option<(
    StaticModifierValue,
    StaticModifierValue,
    Vec<GrantedCreatureKeyword>,
)> {
    if body == "gets +1/+1 for each artifact and/or enchantment you control" {
        let counted = StaticModifierValue::PermanentsYouControl {
            multiplier: 1,
            any_of_card_types: vec![SpecificCardType::Artifact, SpecificCardType::Enchantment],
        };
        return Some((counted.clone(), counted, Vec::new()));
    }

    if let Some(keywords) = body.strip_prefix("has ") {
        return Some((
            StaticModifierValue::Fixed(0),
            StaticModifierValue::Fixed(0),
            parse_granted_creature_keywords(keywords)?,
        ));
    }

    if let Some(keywords) = body.strip_prefix("can't be blocked and has ") {
        let mut parsed = vec![GrantedCreatureKeyword::CantBeBlocked];
        parsed.extend(parse_granted_creature_keywords(keywords)?);
        return Some((
            StaticModifierValue::Fixed(0),
            StaticModifierValue::Fixed(0),
            parsed,
        ));
    }

    let remainder = body.strip_prefix("gets ")?;
    let (modifier, keyword_text) = match remainder.split_once(" and has ") {
        Some((modifier, keywords)) => (modifier, Some(keywords)),
        None => (remainder, None),
    };
    let pattern = Regex::new(r"^(?P<power>[+-][0-9]+)/(?P<toughness>[+-][0-9]+)$")
        .expect("static attached-creature modifier pattern is valid");
    let captures = pattern.captures(modifier)?;
    let power_delta = captures.name("power")?.as_str().parse::<i16>().ok()?;
    let toughness_delta = captures.name("toughness")?.as_str().parse::<i16>().ok()?;
    let granted_keywords = match keyword_text {
        Some(keywords) => parse_granted_creature_keywords(keywords)?,
        None => Vec::new(),
    };

    Some((
        StaticModifierValue::Fixed(power_delta),
        StaticModifierValue::Fixed(toughness_delta),
        granted_keywords,
    ))
}

fn parse_granted_creature_keywords(text: &str) -> Option<Vec<GrantedCreatureKeyword>> {
    let text = text
        .rfind(" (")
        .and_then(|reminder_start| {
            text.ends_with(')')
                .then_some(trim_terminal_period(&text[..reminder_start]))
        })
        .unwrap_or(text);
    let mut keywords = Vec::new();
    for keyword in text.split(" and ") {
        let parsed = match keyword.trim() {
            "deathtouch" => GrantedCreatureKeyword::Deathtouch,
            "double strike" => GrantedCreatureKeyword::DoubleStrike,
            "first strike" => GrantedCreatureKeyword::FirstStrike,
            "flying" => GrantedCreatureKeyword::Flying,
            "haste" => GrantedCreatureKeyword::Haste,
            "hexproof" => GrantedCreatureKeyword::Hexproof,
            "indestructible" => GrantedCreatureKeyword::Indestructible,
            "lifelink" => GrantedCreatureKeyword::Lifelink,
            "menace" => GrantedCreatureKeyword::Menace,
            "reach" => GrantedCreatureKeyword::Reach,
            "shroud" => GrantedCreatureKeyword::Shroud,
            "trample" => GrantedCreatureKeyword::Trample,
            "vigilance" => GrantedCreatureKeyword::Vigilance,
            _ => return None,
        };
        if keywords.contains(&parsed) {
            return None;
        }
        keywords.push(parsed);
    }
    (!keywords.is_empty()).then_some(keywords)
}

fn executable_static_creature_modifier(
    clause_index: usize,
    normalized_oracle: &str,
    target: StaticCreatureModifierTarget,
    power_delta: StaticModifierValue,
    toughness_delta: StaticModifierValue,
    granted_keywords: Vec<GrantedCreatureKeyword>,
) -> AbilityCompilation {
    AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::StaticModifier,
        costs: Vec::new(),
        preconditions: vec![AbilityPrecondition::SourceZone(Zone::Battlefield)],
        effects: vec![AbilityEffect::ApplyStaticCreatureModifier(
            StaticCreatureModifierEffect {
                target,
                power_delta,
                toughness_delta,
                granted_keywords,
            },
        )],
    })
}

fn compile_equip_clause(
    clause_index: usize,
    normalized_oracle: &str,
    type_line: &str,
) -> AbilityCompilation {
    if !type_line_has_card_type(type_line, "equipment") {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::UnsupportedQualifier,
                "Equip is executable only on a source with the Equipment subtype.",
            ),
        );
    }

    let lowercase = normalized_oracle.to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    let pattern = Regex::new(
        r"^equip (?P<cost>(?:\{(?:[0-9]+|[wubrgcx])\})+)(?: \((?P<reminder_cost>(?:\{(?:[0-9]+|[wubrgcx])\})+): attach to target creature you control\. equip only as a sorcery\.\))?$",
    )
    .expect("static Equip pattern is valid");
    let Some(captures) = pattern.captures(lower) else {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "Equip requires one exact printed mana cost and, when present, its complete target-controller and sorcery-speed reminder.",
            ),
        );
    };
    let Some(cost_text) = captures.name("cost").map(|capture| capture.as_str()) else {
        unreachable!("the Equip pattern always captures its cost")
    };
    if captures
        .name("reminder_cost")
        .is_some_and(|reminder| reminder.as_str() != cost_text)
    {
        return unsupported(
            clause_index,
            normalized_oracle,
            UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The Equip reminder cost must match the printed keyword cost.",
            ),
        );
    }
    let cost = match parse_mana_cost(cost_text) {
        Ok(cost) => cost,
        Err(reason) => return unsupported(clause_index, normalized_oracle, reason),
    };

    AbilityCompilation::Executable(ExecutableAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        timing: AbilityTiming::Activated {
            window: ActivationWindow::SorcerySpeedOnly,
        },
        costs: vec![AbilityCost::Mana(cost)],
        preconditions: vec![AbilityPrecondition::SourceZone(Zone::Battlefield)],
        effects: vec![AbilityEffect::AttachSourceToTarget(AttachSourceEffect {
            attachment_kind: AttachmentKind::Equipment,
            target: ObjectFilter {
                card_type: Some(CardType::Creature),
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        })],
    })
}

fn unsupported(
    clause_index: usize,
    normalized_oracle: &str,
    reason: UnsupportedReason,
) -> AbilityCompilation {
    AbilityCompilation::Unsupported(UnsupportedAbility {
        clause_index,
        normalized_oracle: normalized_oracle.to_string(),
        reasons: vec![reason],
    })
}

fn contains_modal_structure(clause: &str) -> bool {
    let lower = clause.to_ascii_lowercase();
    lower.contains("choose one")
        || lower.contains("choose two")
        || lower.contains("choose any number")
        || lower.contains("•")
        || lower.starts_with("mode ")
}

fn starts_trigger(lower: &str) -> bool {
    lower.starts_with("when ")
        || lower.starts_with("whenever ")
        || lower.starts_with("at the beginning of ")
}

fn is_partner_deck_construction_template(lower: &str) -> bool {
    matches!(
        trim_terminal_period(lower),
        "partner" | "partner (you can have two commanders if both have partner.)"
    )
}

fn is_instant_or_sorcery(type_line: &str) -> bool {
    type_line_has_card_type(type_line, "instant") || type_line_has_card_type(type_line, "sorcery")
}

fn is_permanent_source_type(type_line: &str) -> bool {
    [
        "artifact",
        "battle",
        "creature",
        "enchantment",
        "land",
        "planeswalker",
    ]
    .into_iter()
    .any(|card_type| type_line_has_card_type(type_line, card_type))
}

fn is_nonland_spell_source_type(type_line: &str) -> bool {
    !type_line_has_card_type(type_line, "land")
        && [
            "artifact",
            "battle",
            "creature",
            "enchantment",
            "instant",
            "kindred",
            "planeswalker",
            "sorcery",
        ]
        .into_iter()
        .any(|card_type| type_line_has_card_type(type_line, card_type))
}

fn type_line_has_card_type(type_line: &str, expected: &str) -> bool {
    type_line
        .split(|character: char| !character.is_alphabetic())
        .any(|word| word.eq_ignore_ascii_case(expected))
}

fn is_escape_template(lower: &str) -> bool {
    let canonical = trim_terminal_period(lower);
    canonical
        == "each nonland card in your graveyard has escape. the escape cost is equal to the card's mana cost plus exile three other cards from your graveyard. (you may cast cards from your graveyard for their escape cost.)"
}

fn is_all_creature_types_static_template(lower: &str) -> bool {
    trim_terminal_period(lower)
        == "creatures you control are every creature type. the same is true for creature spells you control and creature cards you own that aren't on the battlefield"
}

fn is_self_untap_step_restriction_template(lower: &str) -> bool {
    trim_terminal_period(lower) == "this permanent doesn't untap during your untap step"
}

fn split_activation_restriction(
    effect_and_restriction: &str,
) -> Result<(&str, ActivationWindow), UnsupportedReason> {
    let trimmed = effect_and_restriction.trim();
    let lower = trimmed.to_ascii_lowercase();
    for suffix in [
        " activate only as an instant.",
        " activate only as an instant",
        " activate only any time you could cast an instant.",
        " activate only any time you could cast an instant",
    ] {
        if lower.ends_with(suffix) {
            let split_at = trimmed.len() - suffix.len();
            return Ok((
                trimmed[..split_at].trim_end_matches('.').trim(),
                ActivationWindow::InstantSpeedOnly,
            ));
        }
    }
    if lower.contains("activate only") {
        return Err(UnsupportedReason::new(
            UnsupportedReasonCode::UnsupportedQualifier,
            "The activation restriction is not a reviewed executable timing window.",
        ));
    }
    Ok((trimmed, ActivationWindow::NormalPriority))
}

fn parse_trigger_event(trigger: &str) -> Result<TriggerEvent, UnsupportedReason> {
    let lower = trigger.trim().to_ascii_lowercase();
    match lower.as_str() {
        "at the beginning of your upkeep" => Ok(TriggerEvent {
            kind: TriggerEventKind::BeginningOfUpkeep,
            actor: ControllerRelation::You,
            object_filter: ObjectFilter {
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        }),
        "at the beginning of each opponent's upkeep" => Ok(TriggerEvent {
            kind: TriggerEventKind::BeginningOfUpkeep,
            actor: ControllerRelation::Opponent,
            object_filter: ObjectFilter {
                controller: Some(ControllerRelation::Opponent),
                ..ObjectFilter::default()
            },
        }),
        "at the beginning of the end step" => Ok(TriggerEvent {
            kind: TriggerEventKind::BeginningOfEndStep,
            actor: ControllerRelation::Any,
            object_filter: ObjectFilter::default(),
        }),
        "whenever an opponent casts a spell" => Ok(TriggerEvent {
            kind: TriggerEventKind::SpellCast,
            actor: ControllerRelation::Opponent,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Spell),
                controller: Some(ControllerRelation::Opponent),
                ..ObjectFilter::default()
            },
        }),
        "whenever an opponent casts a noncreature spell" => Ok(TriggerEvent {
            kind: TriggerEventKind::SpellCast,
            actor: ControllerRelation::Opponent,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Spell),
                excluded_card_type: Some(CardType::Creature),
                controller: Some(ControllerRelation::Opponent),
                ..ObjectFilter::default()
            },
        }),
        "whenever an opponent casts their first noncreature spell each turn" => Ok(TriggerEvent {
            kind: TriggerEventKind::FirstFilteredSpellCastEachTurn,
            actor: ControllerRelation::Opponent,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Spell),
                excluded_card_type: Some(CardType::Creature),
                controller: Some(ControllerRelation::Opponent),
                ..ObjectFilter::default()
            },
        }),
        "whenever an opponent casts their second spell each turn" => Ok(TriggerEvent {
            kind: TriggerEventKind::SecondSpellCastEachTurn,
            actor: ControllerRelation::Opponent,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Spell),
                controller: Some(ControllerRelation::Opponent),
                ..ObjectFilter::default()
            },
        }),
        "whenever a player casts their second spell each turn" => Ok(TriggerEvent {
            kind: TriggerEventKind::SecondSpellCastEachTurn,
            actor: ControllerRelation::Any,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Spell),
                controller: Some(ControllerRelation::Any),
                ..ObjectFilter::default()
            },
        }),
        "whenever an opponent draws a card" => Ok(TriggerEvent {
            kind: TriggerEventKind::CardDraw,
            actor: ControllerRelation::Opponent,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Card),
                ..ObjectFilter::default()
            },
        }),
        "whenever you cast a spell" => Ok(TriggerEvent {
            kind: TriggerEventKind::SpellCast,
            actor: ControllerRelation::You,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Spell),
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        }),
        "whenever you cast an artifact or enchantment spell" => Ok(TriggerEvent {
            kind: TriggerEventKind::SpellCast,
            actor: ControllerRelation::You,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Spell),
                any_of_card_types: vec![SpecificCardType::Artifact, SpecificCardType::Enchantment],
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        }),
        "whenever an artifact enters" => Ok(TriggerEvent {
            kind: TriggerEventKind::PermanentEntersBattlefield,
            actor: ControllerRelation::Any,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Artifact),
                ..ObjectFilter::default()
            },
        }),
        "whenever an enchantment you control enters"
        | "whenever an enchantment enters the battlefield under your control" => Ok(TriggerEvent {
            kind: TriggerEventKind::PermanentEntersBattlefield,
            actor: ControllerRelation::You,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Permanent),
                any_of_card_types: vec![SpecificCardType::Enchantment],
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        }),
        "whenever enchanted creature deals damage to an opponent" => Ok(TriggerEvent {
            kind: TriggerEventKind::EnchantedCreatureDealsDamageToOpponent,
            actor: ControllerRelation::You,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Creature),
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        }),
        "whenever equipped creature dies" => Ok(TriggerEvent {
            kind: TriggerEventKind::EquippedCreatureDies,
            actor: ControllerRelation::You,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Creature),
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        }),
        "whenever a creature you control deals combat damage to a player" => Ok(TriggerEvent {
            kind: TriggerEventKind::CreatureDealsCombatDamageToPlayer,
            actor: ControllerRelation::You,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Creature),
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        }),
        "whenever one or more creatures you control deal combat damage to a player" => {
            Ok(TriggerEvent {
                kind: TriggerEventKind::OneOrMoreCreaturesDealCombatDamageToPlayer,
                actor: ControllerRelation::You,
                object_filter: ObjectFilter {
                    card_type: Some(CardType::Creature),
                    controller: Some(ControllerRelation::You),
                    ..ObjectFilter::default()
                },
            })
        }
        "whenever a creature you control of the chosen type enters or attacks" => {
            Ok(TriggerEvent {
                kind: TriggerEventKind::ChosenTypeCreatureEntersOrAttacks,
                actor: ControllerRelation::You,
                object_filter: ObjectFilter {
                    card_type: Some(CardType::Creature),
                    controller: Some(ControllerRelation::You),
                    ..ObjectFilter::default()
                },
            })
        }
        "whenever another creature you control with flying enters" => Ok(TriggerEvent {
            kind: TriggerEventKind::OtherFlyingCreatureEntersBattlefield,
            actor: ControllerRelation::You,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Creature),
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        }),
        "whenever a dwarf you control becomes tapped" => Ok(TriggerEvent {
            kind: TriggerEventKind::PermanentBecomesTapped,
            actor: ControllerRelation::You,
            object_filter: ObjectFilter {
                subtype: Some("Dwarf".to_string()),
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        }),
        "whenever you tap a nonland permanent for mana" => Ok(TriggerEvent {
            kind: TriggerEventKind::PermanentTappedForMana,
            actor: ControllerRelation::You,
            object_filter: ObjectFilter {
                card_type: Some(CardType::Permanent),
                nonland: true,
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
        }),
        _ => Err(UnsupportedReason::new(
            UnsupportedReasonCode::UnrecognizedTrigger,
            format!("Unrecognized trigger event: “{}”.", trigger.trim()),
        )),
    }
}

fn parse_costs(cost_text: &str) -> Result<Vec<AbilityCost>, Vec<UnsupportedReason>> {
    let mut costs = Vec::new();
    let mut reasons = Vec::new();
    for raw_component in cost_text.split(',') {
        let component = trim_terminal_period(raw_component.trim());
        if component.is_empty() {
            reasons.push(UnsupportedReason::new(
                UnsupportedReasonCode::UnrecognizedCost,
                "An empty activated-cost component is not executable.",
            ));
            continue;
        }
        match parse_cost_component(component) {
            Ok(mut parsed) => costs.append(&mut parsed),
            Err(reason) => reasons.push(reason),
        }
    }
    if reasons.is_empty() {
        Ok(costs)
    } else {
        Err(reasons)
    }
}

fn parse_cost_component(component: &str) -> Result<Vec<AbilityCost>, UnsupportedReason> {
    let lower = component.to_ascii_lowercase();
    if lower == "{t}" {
        return Ok(vec![AbilityCost::TapSelf]);
    }
    if lower == "tap two untapped artifacts you control" {
        return Ok(vec![AbilityCost::TapPermanents {
            filter: ObjectFilter {
                card_type: Some(CardType::Artifact),
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
            count: 2,
            // Clock of Omens may tap itself to pay this cost.
            exclude_source: false,
        }]);
    }
    if lower == "tap an untapped dwarf you control" {
        return Ok(vec![AbilityCost::TapPermanents {
            filter: ObjectFilter {
                subtype: Some("Dwarf".to_string()),
                controller: Some(ControllerRelation::You),
                ..ObjectFilter::default()
            },
            count: 1,
            // A Dwarf source may tap itself for this worded cost. This is not
            // a {T} cost, so source identity and summoning-sickness semantics
            // must not be silently substituted.
            exclude_source: false,
        }]);
    }
    if lower == "sacrifice this permanent" {
        return Ok(vec![AbilityCost::SacrificeSelf]);
    }
    if lower == "discard your hand" {
        return Ok(vec![AbilityCost::Discard(DiscardCost::EntireHand)]);
    }
    if lower == "discard a card" {
        return Ok(vec![AbilityCost::Discard(DiscardCost::Cards(1))]);
    }
    if let Some(count) = parse_numbered_phrase(&lower, "discard ", " cards") {
        return Ok(vec![AbilityCost::Discard(DiscardCost::Cards(count))]);
    }
    if let Some(count) = parse_numbered_phrase(&lower, "sacrifice ", " treasures") {
        return Ok(vec![AbilityCost::SacrificeResource {
            resource: ResourceKind::Treasure,
            count,
        }]);
    }
    for (suffix, resource) in [
        (" creatures", ResourceKind::Creature),
        (" artifacts", ResourceKind::Artifact),
        (" tokens", ResourceKind::Token),
    ] {
        if let Some(count) = parse_numbered_phrase(&lower, "sacrifice ", suffix) {
            return Ok(vec![AbilityCost::SacrificeResource { resource, count }]);
        }
    }
    if lower == "sacrifice a creature" {
        return Ok(vec![AbilityCost::SacrificeResource {
            resource: ResourceKind::Creature,
            count: 1,
        }]);
    }
    if lower == "sacrifice an artifact" {
        return Ok(vec![AbilityCost::SacrificeResource {
            resource: ResourceKind::Artifact,
            count: 1,
        }]);
    }
    if let Some(count) = parse_numbered_phrase(&lower, "exile ", " other cards from your graveyard")
    {
        return Ok(vec![AbilityCost::ExileFromGraveyard { count, other: true }]);
    }
    if let Some(count) = parse_numbered_phrase(&lower, "exile ", " cards from your graveyard") {
        return Ok(vec![AbilityCost::ExileFromGraveyard {
            count,
            other: false,
        }]);
    }
    if let Some(life) = parse_numbered_phrase(&lower, "pay ", " life") {
        return Ok(vec![AbilityCost::PayLife(life)]);
    }
    if component.starts_with('{') {
        return parse_mana_cost(component).map(|cost| vec![AbilityCost::Mana(cost)]);
    }

    Err(UnsupportedReason::new(
        UnsupportedReasonCode::UnrecognizedCost,
        format!("Unrecognized activated cost component: “{component}”."),
    ))
}

fn parse_mana_cost(text: &str) -> Result<ManaCost, UnsupportedReason> {
    let compact = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let symbol_pattern =
        Regex::new(r"\{([0-9]+|[WUBRGCX])\}").expect("static mana-symbol pattern is valid");
    let mut matched = String::new();
    let mut profile = ManaCostProfile::default();
    let uppercase = compact.to_ascii_uppercase();
    for capture in symbol_pattern.captures_iter(&uppercase) {
        let whole = capture.get(0).expect("whole mana-symbol capture").as_str();
        let symbol = capture.get(1).expect("mana-symbol capture").as_str();
        matched.push_str(whole);
        match symbol {
            "W" => checked_add_mana(&mut profile.white, 1, text)?,
            "U" => checked_add_mana(&mut profile.blue, 1, text)?,
            "B" => checked_add_mana(&mut profile.black, 1, text)?,
            "R" => checked_add_mana(&mut profile.red, 1, text)?,
            "G" => checked_add_mana(&mut profile.green, 1, text)?,
            "C" => checked_add_mana(&mut profile.colorless, 1, text)?,
            "X" => checked_add_mana(&mut profile.variable_x, 1, text)?,
            generic => {
                let amount = generic.parse::<u16>().map_err(|_| {
                    UnsupportedReason::new(
                        UnsupportedReasonCode::UnrecognizedCost,
                        format!("Mana cost exceeds executable-program limits: “{text}”."),
                    )
                })?;
                checked_add_mana(&mut profile.generic, amount, text)?;
            }
        }
    }
    if matched != uppercase || matched.is_empty() {
        return Err(UnsupportedReason::new(
            UnsupportedReasonCode::UnrecognizedCost,
            format!("Unsupported or malformed mana cost: “{text}”."),
        ));
    }
    Ok(ManaCost::PrintedSymbols {
        oracle: matched,
        profile,
    })
}

fn checked_add_mana(slot: &mut u16, amount: u16, original: &str) -> Result<(), UnsupportedReason> {
    *slot = slot.checked_add(amount).ok_or_else(|| {
        UnsupportedReason::new(
            UnsupportedReasonCode::UnrecognizedCost,
            format!("Mana cost exceeds executable-program limits: “{original}”."),
        )
    })?;
    Ok(())
}

fn append_cost_preconditions(costs: &[AbilityCost], preconditions: &mut Vec<AbilityPrecondition>) {
    for cost in costs {
        match cost {
            AbilityCost::TapSelf => preconditions.push(AbilityPrecondition::SourceUntapped),
            AbilityCost::TapPermanents { filter, count, .. } => {
                preconditions.push(AbilityPrecondition::UntappedResourceAtLeast {
                    resource: ResourceKind::TypedPermanent(filter.clone()),
                    count: *count,
                });
            }
            AbilityCost::SacrificeResource { resource, count } => {
                preconditions.push(AbilityPrecondition::ResourceAtLeast {
                    resource: resource.clone(),
                    count: *count,
                });
            }
            AbilityCost::ExileFromGraveyard { count, other } => {
                preconditions.push(AbilityPrecondition::GraveyardCardsAtLeast {
                    count: *count,
                    other_than_cast_card: *other,
                });
            }
            AbilityCost::Mana(_)
            | AbilityCost::SacrificeSelf
            | AbilityCost::Discard(_)
            | AbilityCost::PayLife(_) => {}
        }
    }
}

fn parse_effects(effect_text: &str) -> Result<Vec<AbilityEffect>, Vec<UnsupportedReason>> {
    let trimmed = effect_text.trim();
    let lowercase = trimmed.to_ascii_lowercase();
    let without_reminder = trim_exact_treasure_reminder(trim_terminal_period(&lowercase));
    let lower = trim_terminal_period(without_reminder);

    if lower == "add {r}. until end of turn, you don't lose this mana as steps and phases end" {
        return Ok(vec![AbilityEffect::AddManaWithRetention(
            LinkedManaRetentionEffect {
                mana: ManaEffect {
                    amount: 1,
                    kind: ManaKind::Fixed(FixedManaProfile {
                        red: 1,
                        ..FixedManaProfile::default()
                    }),
                },
                retention: ManaRetention::ThroughStepsAndPhasesUntilEndOfTurn,
            },
        )]);
    }
    if lower == "you lose 1 life and create a treasure token" {
        return Ok(vec![
            AbilityEffect::LoseLife(LifeLossEffect {
                player: ControllerRelation::You,
                amount: 1,
            }),
            AbilityEffect::CreateToken(TokenEffect {
                count: 1,
                kind: TokenKind::Treasure,
            }),
        ]);
    }
    if lower == "that player may pay {2}. if the player doesn't, you create a treasure token" {
        let payment = parse_mana_cost("{2}").expect("the reviewed payment is valid");
        return Ok(vec![AbilityEffect::UnlessEventPlayerPays(
            UnlessEventPlayerPaysEffect {
                payment: OptionalManaPayment {
                    payer: PaymentPayer::TriggeringPlayer,
                    amount: ManaPaymentAmount::Fixed(payment),
                },
                if_not_paid: vec![AbilityEffect::CreateToken(TokenEffect {
                    count: 1,
                    kind: TokenKind::Treasure,
                })],
            },
        )]);
    }

    if let Some(effect) = parse_draw_effect(lower) {
        return Ok(vec![AbilityEffect::Draw(effect)]);
    }
    if let Some(effect) = parse_library_selection_effect(lower) {
        return Ok(vec![AbilityEffect::LookAtTopAndSelect(effect)]);
    }
    if let Some(effect) = parse_exhaustive_top_card_access_effect(lower) {
        return Ok(vec![AbilityEffect::ExhaustiveTopCardAccess(effect)]);
    }
    if let Some(effect) = parse_mana_effect(lower) {
        return Ok(vec![AbilityEffect::AddMana(effect)]);
    }
    if lower == "sacrifice this permanent" {
        return Ok(vec![AbilityEffect::SacrificeSelf]);
    }
    if let Some(effect) = parse_token_effect(lower) {
        return Ok(vec![AbilityEffect::CreateToken(effect)]);
    }
    if let Some(effect) = parse_tutor_effect(lower) {
        return Ok(vec![AbilityEffect::Tutor(effect)]);
    }
    if is_variable_creature_tutor_overrun_candidate(lower) {
        return parse_variable_creature_tutor_overrun_effects(lower).ok_or_else(|| {
            vec![UnsupportedReason::new(
                UnsupportedReasonCode::MixedKnownAndUnknownEffect,
                "The reviewed variable-X creature tutor and conditional overrun must match atomically, including library/graveyard access, shuffle, X >= 10, +X/+X, haste, and duration.",
            )]
        });
    }
    if let Some(effect) = parse_mill_effect(lower) {
        return Ok(vec![AbilityEffect::Mill(effect)]);
    }
    if lower == "you may untap this permanent" {
        return Ok(vec![AbilityEffect::OptionalUntap(
            TargetSelector::SelfPermanent,
        )]);
    }
    if lower == "target creature gets +2/+0 until end of turn" {
        return Ok(vec![AbilityEffect::ModifyPowerToughnessUntilEndOfTurn(
            PowerToughnessModifierEffect {
                target: TargetSelector::Target(ObjectFilter {
                    card_type: Some(CardType::Creature),
                    ..ObjectFilter::default()
                }),
                power_delta: 2,
                toughness_delta: 0,
            },
        )]);
    }
    if lower == "this permanent gets +1/+1 until end of turn" {
        return Ok(vec![AbilityEffect::ModifyPowerToughnessUntilEndOfTurn(
            PowerToughnessModifierEffect {
                target: TargetSelector::SelfPermanent,
                power_delta: 1,
                toughness_delta: 1,
            },
        )]);
    }
    if let Some((tap, target)) = parse_tap_effect(lower) {
        return Ok(vec![if tap {
            AbilityEffect::Tap(target)
        } else {
            AbilityEffect::Untap(target)
        }]);
    }
    if let Some(effect) = parse_zone_movement(lower) {
        return Ok(vec![AbilityEffect::MoveZone(effect)]);
    }

    let code = if contains_known_effect_signal(lower) {
        UnsupportedReasonCode::MixedKnownAndUnknownEffect
    } else {
        UnsupportedReasonCode::UnrecognizedEffect
    };
    Err(vec![UnsupportedReason::new(
        code,
        if code == UnsupportedReasonCode::MixedKnownAndUnknownEffect {
            "The paragraph contains a recognized effect signal plus unrecognized text; the entire ability was rejected."
        } else {
            "No reviewed executable effect template matched the entire paragraph."
        },
    )])
}

fn parse_upkeep_life_loss_creature_token_effects(effect_text: &str) -> Option<Vec<AbilityEffect>> {
    let lowercase = effect_text.trim().to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);
    let remainder = lower.strip_prefix("you lose ")?;
    let (life_amount, token_text) = remainder.split_once(" life and ")?;
    let amount = parse_number_word(life_amount)?;
    let token = parse_token_effect(token_text)?;
    if !matches!(&token.kind, TokenKind::Creature { .. }) {
        return None;
    }

    Some(vec![
        AbilityEffect::LoseLife(LifeLossEffect {
            player: ControllerRelation::You,
            amount,
        }),
        AbilityEffect::CreateToken(token),
    ])
}

fn parse_spell_effects(effect_text: &str) -> Result<Vec<AbilityEffect>, Vec<UnsupportedReason>> {
    let lowercase = effect_text.trim().to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);

    if let Some(effect) = parse_whole_hand_discard_then_draw_effect(lower) {
        return Ok(vec![AbilityEffect::WholeHandDiscardThenDraw(effect)]);
    }
    if let Some(effect) = parse_repeatable_top_card_reveal_effect(lower) {
        return Ok(vec![AbilityEffect::RepeatableTopCardReveal(effect)]);
    }
    if looks_like_atomic_spell_card_access(lower) {
        return Err(vec![UnsupportedReason::new(
            UnsupportedReasonCode::MixedKnownAndUnknownEffect,
            "The reviewed burst-card-access spell must match atomically, including player scope, effect order, fixed count, zones, life coupling, and optional repetition.",
        )]);
    }

    parse_effects(effect_text)
}

fn parse_activated_effects(
    effect_text: &str,
) -> Result<Vec<AbilityEffect>, Vec<UnsupportedReason>> {
    let lowercase = effect_text.trim().to_ascii_lowercase();
    let lower = trim_terminal_period(&lowercase);

    if let Some(effect) = parse_linked_delayed_card_access_effect(lower) {
        return Ok(vec![AbilityEffect::LinkedDelayedCardAccess(effect)]);
    }
    if looks_like_linked_delayed_card_access(lower) {
        return Err(vec![UnsupportedReason::new(
            UnsupportedReasonCode::MixedKnownAndUnknownEffect,
            "The reviewed delayed card-access activation must preserve the face-down library-to-exile move, the linked object identity, and the move to hand at your next end step.",
        )]);
    }

    parse_effects(effect_text)
}

fn parse_whole_hand_discard_then_draw_effect(
    lower: &str,
) -> Option<WholeHandDiscardThenDrawEffect> {
    if lower != "each player discards their hand, then draws seven cards" {
        return None;
    }
    Some(WholeHandDiscardThenDrawEffect {
        players: AffectedPlayers::EachPlayer,
        discard: WholeHandDiscardStep {
            from: Zone::Hand,
            to: Zone::Graveyard,
        },
        draw: FixedDrawStep {
            count: 7,
            from: Zone::Library,
            to: Zone::Hand,
        },
    })
}

fn parse_repeatable_top_card_reveal_effect(lower: &str) -> Option<RepeatableTopCardRevealEffect> {
    if lower
        != "reveal the top card of your library and put that card into your hand. you lose life equal to its mana value. you may repeat this process any number of times"
    {
        return None;
    }
    Some(RepeatableTopCardRevealEffect {
        player: ControllerRelation::You,
        iteration: TopCardRevealIteration {
            reveal: RevealTopCardsStep {
                count: 1,
                from: Zone::Library,
            },
            movement: RevealedCardMovementStep {
                from: Zone::Library,
                to: Zone::Hand,
            },
            life_loss: CoupledLifeLoss::ManaValueOfCardMovedByThisIteration,
        },
        repetition: RepetitionPolicy::OneMandatoryThenMayRepeatEntireIterationAnyNumberOfTimes,
    })
}

fn looks_like_atomic_spell_card_access(lower: &str) -> bool {
    (lower.contains("each player")
        && (lower.contains("discards their hand") || lower.contains("draws ")))
        || lower.contains("repeat this process")
        || lower.contains("reveal the top card of your library")
            && lower.contains("lose life equal to")
}

fn parse_linked_delayed_card_access_effect(lower: &str) -> Option<LinkedDelayedCardAccessEffect> {
    if lower
        != "exile the top card of your library face down. put that card into your hand at the beginning of your next end step"
    {
        return None;
    }
    Some(LinkedDelayedCardAccessEffect {
        player: ControllerRelation::You,
        count: 1,
        from: Zone::Library,
        source_position: LibraryPosition::Top,
        intermediate: Zone::Exile,
        face_down: true,
        tracked_object: DelayedObjectReference::CardMovedByThisEffect,
        delayed_event: DelayedEvent::BeginningOfYourNextEndStep,
        destination: Zone::Hand,
    })
}

fn looks_like_linked_delayed_card_access(lower: &str) -> bool {
    lower.contains("exile the top card of your library face down")
        || lower.contains("at the beginning of your next end step")
}

fn parse_draw_effect(lower: &str) -> Option<CardAccessEffect> {
    let pattern = Regex::new(
        r"^(?P<optional>you may )?draw (?P<count>a|one|two|three|four|five|[0-9]+) cards?(?: unless (?:that player|its controller) pays (?P<pay>(?:\{[0-9WUBRGCXwubrgcx]+\})+)(?P<source_power>, where x is this permanent's power)?)?$",
    )
    .expect("static draw pattern is valid");
    let captures = pattern.captures(lower)?;
    let count = parse_number_word(captures.name("count")?.as_str())?;
    let unless_event_player_pays = match (
        captures.name("pay").map(|payment| payment.as_str()),
        captures.name("source_power"),
    ) {
        (None, None) => None,
        (Some("{x}"), Some(_)) => Some(OptionalManaPayment {
            payer: PaymentPayer::TriggeringPlayer,
            amount: ManaPaymentAmount::SourcePower,
        }),
        (Some(payment), None) if !payment.eq_ignore_ascii_case("{x}") => {
            Some(OptionalManaPayment {
                payer: PaymentPayer::TriggeringPlayer,
                amount: ManaPaymentAmount::Fixed(parse_mana_cost(payment).ok()?),
            })
        }
        // A variable payment without its source-power binding, or a binding
        // attached to a fixed payment, is materially incomplete.
        _ => return None,
    };
    Some(CardAccessEffect {
        count,
        optional: captures.name("optional").is_some(),
        unless_event_player_pays,
    })
}

fn trim_exact_treasure_reminder(text: &str) -> &str {
    const REMINDER: &str =
        " (it's an artifact with \"{t}, sacrifice this token: add one mana of any color.\")";
    text.strip_suffix(REMINDER).unwrap_or(text)
}

fn parse_library_selection_effect(lower: &str) -> Option<LibrarySelectionEffect> {
    let pattern = Regex::new(
        r"^look at the top (?P<look>one|two|three|four|five|[0-9]+) cards of your library\. you may put a non-human creature card from among them onto the battlefield\. put the rest on the bottom of your library in a random order$",
    )
    .expect("static library-selection pattern is valid");
    let captures = pattern.captures(lower)?;
    Some(LibrarySelectionEffect {
        look_count: parse_number_word(captures.name("look")?.as_str())?,
        selection_count: 1,
        optional: true,
        filter: ObjectFilter {
            card_type: Some(CardType::Creature),
            excluded_subtype: Some("Human".to_string()),
            ..ObjectFilter::default()
        },
        destination: Zone::Battlefield,
        remainder: LibraryRemainderPlacement::BottomInRandomOrder,
    })
}

fn parse_exhaustive_top_card_access_effect(lower: &str) -> Option<TopCardAccessEffect> {
    if lower
        != "scry 1, then reveal the top card of your library. if it's a land card, put it onto the battlefield tapped. otherwise, draw a card"
    {
        return None;
    }
    Some(TopCardAccessEffect {
        scry_count: 1,
        reveal: true,
        land_destination: Zone::Battlefield,
        land_enters_tapped: true,
        nonland_destination: Zone::Hand,
    })
}

fn parse_mana_effect(lower: &str) -> Option<ManaEffect> {
    if let Some(profile) = parse_fixed_mana_output(lower) {
        let amount = [
            profile.white,
            profile.blue,
            profile.black,
            profile.red,
            profile.green,
            profile.colorless,
        ]
        .into_iter()
        .try_fold(0u16, u16::checked_add)?;
        return Some(ManaEffect {
            amount,
            kind: ManaKind::Fixed(profile),
        });
    }

    let pattern = Regex::new(
        r"^add (?P<count>one|two|three|four|five|[0-9]+) mana of (?P<kind>any one color|any type that permanent produced)$",
    )
    .expect("static mana-effect pattern is valid");
    let captures = pattern.captures(lower)?;
    let amount = parse_number_word(captures.name("count")?.as_str())?;
    let kind = match captures.name("kind")?.as_str() {
        "any one color" => ManaKind::AnyOneColor,
        "any type that permanent produced" => ManaKind::AnyTypeProducedByTriggeringPermanent,
        _ => return None,
    };
    Some(ManaEffect { amount, kind })
}

fn parse_fixed_mana_output(lower: &str) -> Option<FixedManaProfile> {
    let symbols = lower.strip_prefix("add ")?;
    let pattern =
        Regex::new(r"(?:\{[wubrgc]\})+").expect("static fixed-mana output pattern is valid");
    if pattern.find(symbols)?.as_str() != symbols {
        return None;
    }

    let mut profile = FixedManaProfile::default();
    for symbol in symbols.as_bytes().chunks_exact(3) {
        let slot = match symbol {
            b"{w}" => &mut profile.white,
            b"{u}" => &mut profile.blue,
            b"{b}" => &mut profile.black,
            b"{r}" => &mut profile.red,
            b"{g}" => &mut profile.green,
            b"{c}" => &mut profile.colorless,
            _ => return None,
        };
        *slot = slot.checked_add(1)?;
    }
    Some(profile)
}

fn parse_token_effect(lower: &str) -> Option<TokenEffect> {
    let treasure_pattern =
        Regex::new(r"^create (?P<count>a|one|two|three|four|five|[0-9]+) treasure tokens?$")
            .expect("static Treasure pattern is valid");
    if let Some(captures) = treasure_pattern.captures(lower) {
        return Some(TokenEffect {
            count: parse_number_word(captures.name("count")?.as_str())?,
            kind: TokenKind::Treasure,
        });
    }

    let creature_pattern = Regex::new(
        r"^create (?P<count>a|one|two|three|four|five|[0-9]+) (?P<power>[0-9]+)/(?P<toughness>[0-9]+) (?P<description>[a-z0-9 ]+) creature tokens?(?: with (?P<keyword>flying))?$",
    )
    .expect("static creature-token pattern is valid");
    let captures = creature_pattern.captures(lower)?;
    let keywords = match captures.name("keyword").map(|keyword| keyword.as_str()) {
        None => Vec::new(),
        Some("flying") => vec![CreatureTokenKeyword::Flying],
        Some(_) => return None,
    };
    Some(TokenEffect {
        count: parse_number_word(captures.name("count")?.as_str())?,
        kind: TokenKind::Creature {
            power: captures.name("power")?.as_str().parse().ok()?,
            toughness: captures.name("toughness")?.as_str().parse().ok()?,
            description: captures.name("description")?.as_str().to_string(),
            keywords,
        },
    })
}

fn parse_tutor_effect(lower: &str) -> Option<TutorEffect> {
    if lower
        != "search your library for an artifact or dragon card, put that card onto the battlefield, then shuffle"
    {
        return None;
    }
    Some(TutorEffect {
        from: Zone::Library,
        destination: Zone::Battlefield,
        filter: TutorFilter::AnyOf(vec![
            ObjectFilter {
                card_type: Some(CardType::Artifact),
                ..ObjectFilter::default()
            },
            ObjectFilter {
                subtype: Some("Dragon".to_string()),
                ..ObjectFilter::default()
            },
        ]),
        shuffle_after: true,
    })
}

fn is_variable_creature_tutor_overrun_candidate(normalized_oracle: &str) -> bool {
    let lower = normalized_oracle.to_ascii_lowercase();
    (lower.contains("search your library")
        && lower.contains("creature card with mana value x or less"))
        || lower.contains("if x is 10 or more, creatures you control")
        || lower.contains("creatures you control get +x/+x")
}

fn parse_variable_creature_tutor_overrun_effects(lower: &str) -> Option<Vec<AbilityEffect>> {
    if lower
        != "search your library and/or graveyard for a creature card with mana value x or less and put it onto the battlefield. if you search your library this way, shuffle. if x is 10 or more, creatures you control get +x/+x and gain haste until end of turn"
    {
        return None;
    }
    Some(vec![
        AbilityEffect::VariableCreatureTutor(VariableCreatureTutorEffect {
            from_library: true,
            from_graveyard: true,
            destination: Zone::Battlefield,
            mana_value_at_most_x: true,
            shuffle_if_library_searched: true,
        }),
        AbilityEffect::VariableCreatureOverrun(VariableCreatureOverrunEffect {
            minimum_x: 10,
            creatures_you_control: true,
            power_bonus_equals_x: true,
            toughness_bonus_equals_x: true,
            grants_haste: true,
            until_end_of_turn: true,
        }),
    ])
}

fn parse_mill_effect(lower: &str) -> Option<MillEffect> {
    let pattern =
        Regex::new(r"^target player mills (?P<count>a|one|two|three|four|five|[0-9]+) cards?$")
            .expect("static mill pattern is valid");
    let captures = pattern.captures(lower)?;
    Some(MillEffect {
        player: PlayerSelector::TargetPlayer,
        count: parse_number_word(captures.name("count")?.as_str())?,
    })
}

fn parse_tap_effect(lower: &str) -> Option<(bool, TargetSelector)> {
    let (tap, target) = if let Some(target) = lower.strip_prefix("tap ") {
        (true, target)
    } else if let Some(target) = lower.strip_prefix("untap ") {
        (false, target)
    } else {
        return None;
    };
    let selector = match target {
        "this permanent" => TargetSelector::SelfPermanent,
        "enchanted creature" => TargetSelector::Enchanted(ObjectFilter {
            card_type: Some(CardType::Creature),
            ..ObjectFilter::default()
        }),
        "target artifact" => TargetSelector::Target(ObjectFilter {
            card_type: Some(CardType::Artifact),
            ..ObjectFilter::default()
        }),
        "target creature" => TargetSelector::Target(ObjectFilter {
            card_type: Some(CardType::Creature),
            ..ObjectFilter::default()
        }),
        "target permanent" => TargetSelector::Target(ObjectFilter {
            card_type: Some(CardType::Permanent),
            ..ObjectFilter::default()
        }),
        _ => return None,
    };
    Some((tap, selector))
}

fn parse_zone_movement(lower: &str) -> Option<ZoneMovementEffect> {
    let pattern = Regex::new(
        r"^(?:return|put) target (?P<kind>creature|artifact|permanent|card) card from your graveyard (?:to|into) your hand$",
    )
    .expect("static zone-movement pattern is valid");
    let captures = pattern.captures(lower)?;
    let card_type = match captures.name("kind")?.as_str() {
        "creature" => CardType::Creature,
        "artifact" => CardType::Artifact,
        "permanent" => CardType::Permanent,
        "card" => CardType::Card,
        _ => return None,
    };
    Some(ZoneMovementEffect {
        object: TargetSelector::Target(ObjectFilter {
            card_type: Some(card_type),
            controller: Some(ControllerRelation::You),
            ..ObjectFilter::default()
        }),
        from: Zone::Graveyard,
        to: Zone::Hand,
    })
}

fn contains_known_effect_signal(lower: &str) -> bool {
    [
        "draw ",
        "look at ",
        "add ",
        "create ",
        "search your library",
        "mill",
        "copy ",
        "sacrifice ",
        "tap ",
        "untap ",
        "return ",
        "put ",
        "cast ",
        "escape",
    ]
    .iter()
    .any(|signal| lower.contains(signal))
}

fn parse_numbered_phrase(text: &str, prefix: &str, suffix: &str) -> Option<u16> {
    let count = text.strip_prefix(prefix)?.strip_suffix(suffix)?;
    parse_number_word(count)
}

fn parse_number_word(word: &str) -> Option<u16> {
    match word.trim() {
        "a" | "an" | "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        value => value.parse().ok(),
    }
}

fn trim_terminal_period(text: &str) -> &str {
    text.trim().strip_suffix('.').unwrap_or(text.trim()).trim()
}
