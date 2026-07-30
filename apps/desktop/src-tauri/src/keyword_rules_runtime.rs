//! Generic execution contracts for official Magic keyword rules.
//!
//! These contracts are card-name independent. A compiled program records the
//! exact printed keyword occurrence and the official rule identifiers that
//! authorize its behavior. Unsupported syntax and mechanics that do not exist
//! in the installed Comprehensive Rules fail closed before state is changed.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

pub const KEYWORD_RULES_RUNTIME_VERSION: &str = "official-keyword-rules-2026-06-19/v12";
pub const KEYWORD_RULES_EVIDENCE_VERSION: &str = "official-keyword-evidence/v1";
pub const KEYWORD_RULES_EFFECTIVE_DATE: &str = "2026-06-19";
pub const KEYWORD_RULES_SOURCE_URL: &str =
    "https://media.wizards.com/2026/downloads/MagicCompRules%2020260619.txt";
pub const AFTERMATH_CANONICAL_ORACLE_CLAUSE: &str =
    "Aftermath (Cast this spell only from your graveyard. Then exile it.)";
pub const PERSIST_CANONICAL_ORACLE_CLAUSE: &str = "Persist (When this creature dies, if it had no -1/-1 counters on it, return it to the battlefield under its owner's control with a -1/-1 counter on it.)";
pub const UNDYING_CANONICAL_ORACLE_CLAUSE: &str = "Undying (When this creature dies, if it had no +1/+1 counters on it, return it to the battlefield under its owner's control with a +1/+1 counter on it.)";
pub const DAYBOUND_CANONICAL_ORACLE_CLAUSE: &str =
    "Daybound (If a player casts no spells during their own turn, it becomes night next turn.)";
pub const NIGHTBOUND_CANONICAL_ORACLE_CLAUSE: &str = "Nightbound (If a player casts at least two spells during their own turn, it becomes day next turn.)";
pub const START_YOUR_ENGINES_CANONICAL_ORACLE_CLAUSE: &str = "Start your engines! (If you have no speed, it starts at 1. It increases once on each of your turns when an opponent loses life. Max speed is 4.)";
pub const CHOOSE_A_BACKGROUND_CANONICAL_ORACLE_CLAUSE: &str =
    "Choose a Background (You can have a Background as a second commander.)";
pub const DOCTORS_COMPANION_CANONICAL_ORACLE_CLAUSE: &str =
    "Doctor's companion (You can have two commanders if the other is the Doctor.)";
pub const EXPLOIT_CANONICAL_ORACLE_CLAUSE: &str =
    "Exploit (When this creature enters, you may sacrifice a creature.)";
pub const SOULBOND_CANONICAL_ORACLE_CLAUSE: &str = "Soulbond (You may pair this creature with another unpaired creature when either enters. They remain paired for as long as you control both of them.)";
pub const EVOLVE_CANONICAL_ORACLE_CLAUSE: &str = "Evolve (Whenever a creature you control enters, if that creature has greater power or toughness than this creature, put a +1/+1 counter on this creature.)";
pub const IMPROVISE_CANONICAL_ORACLE_CLAUSE: &str = "Improvise (Your artifacts can help cast this spell. Each artifact you tap after you're done activating mana abilities pays for {1}.)";
pub const INTIMIDATE_CANONICAL_ORACLE_CLAUSE: &str = "Intimidate (This creature can't be blocked except by artifact creatures and/or creatures that share a color with it.)";
pub const SPREE_CANONICAL_ORACLE_CLAUSE: &str = "Spree (Choose one or more additional costs.)";
pub const BARGAIN_CANONICAL_ORACLE_CLAUSE: &str =
    "Bargain (You may sacrifice an artifact, enchantment, or token as you cast this spell.)";
pub const MENTOR_CANONICAL_ORACLE_CLAUSE: &str = "Mentor (Whenever this creature attacks, put a +1/+1 counter on target attacking creature with lesser power.)";
pub const EXTORT_CANONICAL_ORACLE_CLAUSE: &str = "Extort (Whenever you cast a spell, you may pay {W/B}. If you do, each opponent loses 1 life and you gain that much life.)";
pub const LIVING_WEAPON_CANONICAL_ORACLE_CLAUSE: &str = "Living weapon (When this Equipment enters, create a 0/0 black Phyrexian Germ creature token, then attach this to it.)";
pub const MYRIAD_CANONICAL_ORACLE_CLAUSE: &str = "Myriad (Whenever this creature attacks, for each opponent other than defending player, you may create a token copy that's tapped and attacking that player or a planeswalker they control. Exile the tokens at end of combat.)";
pub const RETRACE_CANONICAL_ORACLE_CLAUSE: &str = "Retrace (You may cast this card from your graveyard by discarding a land card in addition to paying its other costs.)";
pub const BACKUP_ONE_CANONICAL_ORACLE_CLAUSE: &str = "Backup 1 (When this creature enters, put a +1/+1 counter on target creature. If that's another creature, it gains the following ability until end of turn.)";
pub const UMBRA_ARMOR_CANONICAL_ORACLE_CLAUSE: &str = "Umbra armor (If enchanted creature would be destroyed, instead remove all damage from it and destroy this Aura.)";
pub const CIPHER_CANONICAL_ORACLE_CLAUSE: &str = "Cipher (Then you may exile this spell card encoded on a creature you control. Whenever that creature deals combat damage to a player, its controller may cast a copy of the encoded card without paying its mana cost.)";
pub const RENOWN_ONE_CANONICAL_ORACLE_CLAUSE: &str = "Renown 1 (When this creature deals combat damage to a player, if it isn't renowned, put a +1/+1 counter on it and it becomes renowned.)";
pub const CONVOKE_CANONICAL_ORACLE_CLAUSE: &str = "Convoke (Your creatures can help cast this spell. Each creature you tap while casting this spell pays for {1} or one mana of that creature's color.)";
pub const CHANGELING_CANONICAL_ORACLE_CLAUSE: &str =
    "Changeling (This card is every creature type.)";
pub const INFECT_CANONICAL_ORACLE_CLAUSE: &str = "Infect (This creature deals damage to creatures in the form of -1/-1 counters and to players in the form of poison counters.)";
pub const AFFINITY_FOR_ARTIFACTS_CANONICAL_ORACLE_CLAUSE: &str =
    "Affinity for artifacts (This spell costs {1} less to cast for each artifact you control.)";
pub const EXALTED_CANONICAL_ORACLE_CLAUSE: &str = "Exalted (Whenever a creature you control attacks alone, that creature gets +1/+1 until end of turn.)";
pub const REBOUND_CANONICAL_ORACLE_CLAUSE: &str = "Rebound (If you cast this spell from your hand, exile it as it resolves. At the beginning of your next upkeep, you may cast this card from exile without paying its mana cost.)";
pub const CASCADE_CANONICAL_ORACLE_CLAUSE: &str = "Cascade (When you cast this spell, exile cards from the top of your library until you exile a nonland card that costs less. You may cast it without paying its mana cost. Put the exiled cards on the bottom in a random order.)";
pub const ASCEND_CANONICAL_ORACLE_CLAUSE: &str = "Ascend (If you control ten or more permanents, you get the city's blessing for the rest of the game.)";
pub const DELVE_CANONICAL_ORACLE_CLAUSE: &str =
    "Delve (Each card you exile from your graveyard while casting this spell pays for {1}.)";
pub const FUSE_CANONICAL_ORACLE_CLAUSE: &str =
    "Fuse (You may cast one or both halves of this card from your hand.)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OfficialRule {
    MillDefinition,
    MillAvailability,
    Destroy,
    RegenerateResolvingEffect,
    RegenerateStaticEffect,
    RegenerateCannotBeApplied,
    SpellTargetSelection,
    ResolveTargetLegality,
    ResolveInstructionsInOrder,
    ZoneChangeCreatesNewObject,
    ReplacementEffectChoice,
    ProtectionDefinition,
    ProtectionTargeting,
    ProtectionAuras,
    ProtectionEquipmentAndFortifications,
    ProtectionDamage,
    ProtectionBlocking,
    ProtectionMultipleQualities,
    ProtectionEverything,
    ProtectionPlayer,
    FlyingDefinition,
    FlyingBlocking,
    FlyingRedundancy,
    FightDefinition,
    FightResolutionLegality,
    InvestigateDefinition,
    ClueTokenDefinition,
    CastDeclareAdditionalOrAlternativeCost,
    CastChooseTargets,
    CastLegalityCheck,
    CastDetermineTotalCost,
    CastActivateManaAbilities,
    CastPayTotalCost,
    AdditionalCostDefinition,
    AdditionalCostMultiplicity,
    AdditionalCostOptional,
    AdditionalCostDoesNotChangeManaCost,
    ModalSpellDefinition,
    ModalSpellControllerChooses,
    ModalTargetsFollowChosenModes,
    ModalModeNormallyChosenOnce,
    ModalRetargetingDoesNotChangeMode,
    ModalCopyRetainsModes,
    ModalAssociatedAdditionalCosts,
    KickerDefinition,
    KickerMultipleCosts,
    KickerDeclaration,
    KickerLinkedAbilities,
    FlashbackDefinition,
    MorphDefinition,
    MorphCastProcedure,
    MorphSpecialAction,
    FaceDownCharacteristics,
    FlashDefinition,
    FlashRedundancy,
    MenaceDefinition,
    MenaceBlocking,
    MenaceRedundancy,
    DefenderDefinition,
    DefenderAttackRestriction,
    DefenderRedundancy,
    ReachDefinition,
    ReachBlocking,
    ReachRedundancy,
    ShadowDefinition,
    ShadowBlocking,
    ShadowRedundancy,
    LandwalkDefinition,
    LandwalkEvasion,
    LandwalkBlocking,
    LandwalkNoCancellation,
    LandwalkRedundancy,
    FearDefinition,
    FearBlocking,
    FearRedundancy,
    CharacteristicDefiningAbility,
    ChangelingDefinition,
    DevoidDefinition,
    AffinityDefinition,
    AffinityMultipleInstances,
    ConvokeDefinition,
    ConvokeAfterTotalCost,
    ConvokeTappedCreatureDesignation,
    ConvokeRedundancy,
    Attach,
    EquipmentDefinition,
    EquipmentEntryAndEquip,
    EquipmentLegality,
    EquipmentControl,
    EquipDefinition,
    EquipQuality,
    EquipMultipleAbilities,
    EquipPlaneswalker,
    AuraSpellTarget,
    AuraDefinition,
    AuraTarget,
    AuraIllegalAttachment,
    AuraSelfAndCreatureRestriction,
    AuraNoLegalEntry,
    AuraBattlefieldAttachFailure,
    EnchantDefinition,
    EnchantMultipleRestrictions,
    EnchantPlayer,
    SagaChapterSymbol,
    SagaChapterTrigger,
    SagaFinalChapter,
    SagaEntryLoreCounter,
    SagaPrecombatLoreCounter,
    SagaSacrifice,
    SagaStateBasedSacrifice,
    Sacrifice,
    PayCompleteCost,
    PayLife,
    CumulativeUpkeepDefinition,
    CumulativeUpkeepSharedAgeCounters,
    DelveDefinition,
    DelveAfterTotalCost,
    DelveRedundancy,
    ExaltedDefinition,
    ExaltedAttacksAlone,
    CascadeDefinition,
    CascadeActionWindow,
    CascadeMultipleInstances,
    ReboundDefinition,
    ReboundAlternativeCost,
    ReboundRedundancy,
    InfectStatic,
    InfectPlayerDamage,
    InfectCreatureDamage,
    InfectLastKnownInformation,
    InfectAllZones,
    InfectRedundancy,
    FuseDefinition,
    FuseCombinedCharacteristics,
    FuseCombinedManaCost,
    FuseResolutionOrder,
    AftermathDefinition,
    SplitCardSinglePhysicalCard,
    SplitCardChooseHalf,
    SplitCardOnlyChosenHalfEvaluated,
    SplitSpellChosenHalfCharacteristics,
    AscendSpell,
    AscendPermanent,
    AscendDesignation,
    AscendContinuousEffectRecheck,
    SummoningSickness,
    DeclareAttackerTap,
    DeclareBlockerRestrictions,
    ObjectColors,
    CombatDamageSteps,
    PlayerDamage,
    CreatureDamage,
    LifelinkDamage,
    LethalDamageStateBasedAction,
    DeathtouchStateBasedAction,
    Discard,
    Draw,
    EmptyLibraryDraw,
    HasteDefinition,
    HasteAttack,
    HasteTapAbility,
    HasteRedundancy,
    VigilanceDefinition,
    VigilanceAttack,
    VigilanceRedundancy,
    TrampleDefinition,
    TrampleAssignment,
    TrampleNoBlockers,
    TrampleRedundancy,
    DeathtouchDefinition,
    DeathtouchDestruction,
    DeathtouchLethalAssignment,
    DeathtouchZoneFunction,
    DeathtouchRedundancy,
    LifelinkDefinition,
    LifelinkLifeGain,
    LifelinkZoneFunction,
    LifelinkSeparateEvents,
    LifelinkRedundancy,
    FirstStrikeDefinition,
    FirstStrikeDamageSteps,
    FirstStrikeChanges,
    FirstStrikeRedundancy,
    DoubleStrikeDefinition,
    DoubleStrikeDamageSteps,
    DoubleStrikeChanges,
    DoubleStrikeGainedAfterFirst,
    DoubleStrikeRedundancy,
    HexproofDefinition,
    HexproofPermanent,
    HexproofPlayer,
    HexproofQuality,
    HexproofMultipleQualities,
    ShroudDefinition,
    ShroudRedundancy,
    IndestructibleStatic,
    IndestructibleDefinition,
    IndestructibleRedundancy,
    ProwessDefinition,
    ProwessMultipleInstances,
    BushidoDefinition,
    BushidoMultipleInstances,
    WitherDefinition,
    WitherLastKnownInformation,
    WitherAllZones,
    WitherRedundancy,
    HorsemanshipDefinition,
    HorsemanshipBlocking,
    HorsemanshipRedundancy,
    FlankingDefinition,
    FlankingMultipleInstances,
    PersistDefinition,
    UndyingDefinition,
    ToxicDefinition,
    ToxicTotalValue,
    ToxicCombatDamage,
    ZoneChangeTriggerFindsNewPublicObject,
    TriggeredAbilityEvent,
    TriggeredAbilityOncePerEvent,
    TriggeredAbilityStackLifecycle,
    TriggeredAbilityController,
    InterveningIfClause,
    EntersBattlefieldTrigger,
    ResolutionObjectInformation,
    LeavesBattlefieldTrigger,
    LeavesBattlefieldLookBack,
    ReplacedEventNeverHappens,
    TokenOutsideBattlefieldCeases,
    TokenStateBasedActionCeases,
    CounterDefinition,
    PowerToughnessCounterEffect,
    CounterPlacedOnBattlefieldObject,
    CounterCeasesOnZoneChange,
    DamageProcessingSequence,
    DamageDealtAfterReplacementAndPrevention,
    DamageProcessedIntoResults,
    PoisonLossStateBasedAction,
    AbilityDefaultZone,
    AbilityEntryModificationZone,
    UntapDayNightTransition,
    UntapDayNightTeamTransition,
    TransformDefinition,
    TransformRequiresDoubleFaced,
    TransformRejectsInstantSorceryBack,
    InstantCannotEnterBattlefield,
    SorceryCannotEnterBattlefield,
    DoubleFacedEntryTransformed,
    DoubleFacedNonstackEntry,
    DoubleFacedNonstackEntryTransformed,
    TransformPreservesObject,
    DayNightFaces,
    DayboundDefinition,
    DayboundImmediateAlignment,
    DayboundInitialDay,
    NightboundDefinition,
    NightboundImmediateAlignment,
    NightboundInitialNight,
    DayNightDesignation,
    DayNightUntapCheck,
    DayToNightSpellCount,
    NightToDaySpellCount,
    StartYourEnginesDefinition,
    SpeedAbsentUntilSet,
    SpeedIncreaseFromNoSpeed,
    SpeedInherentTrigger,
    SpeedMaximum,
    SpeedNoSpeedAsZero,
    SpeedStateBasedInitialization,
    DeckConstructionAbilityZone,
    CommanderDesignation,
    CommanderDeckConstructionAbility,
    CommanderColorIdentity,
    CommanderColorIdentityLockedBeforeGame,
    CommanderDeckSize,
    CommanderStartsInCommandZone,
    CommanderTax,
    CommanderDamageLoss,
    PartnerAbilityDefinition,
    PartnerDeckSize,
    PartnerCombinedColorIdentity,
    PartnerIndependentCommanders,
    PartnerCommanderReference,
    PartnerVariantsDistinct,
    PartnerAtMostTwo,
    ChooseBackgroundDefinition,
    DoctorsCompanionDefinition,
    PartnerNamedReferenceScope,
    ExploitDefinition,
    ExploitEventDefinition,
    SoulbondDefinition,
    SoulbondPairedDesignation,
    SoulbondResolutionRecheck,
    SoulbondOnePartnerLimit,
    SoulbondUnpairEvents,
    EvolveDefinition,
    EvolveEventDefinition,
    EvolveNoncreatureComparison,
    EvolveMultipleInstances,
    ImproviseDefinition,
    ImproviseAfterTotalCost,
    ImproviseRedundancy,
    IntimidateDefinition,
    IntimidateBlocking,
    IntimidateRedundancy,
    SpreeDefinition,
    SpreeSymbols,
    BargainDefinition,
    BargainDeclaredStatus,
    BargainLinkedAbilities,
    BargainConditionalTargets,
    MentorDefinition,
    MentorMultipleInstances,
    MentorEventDefinition,
    ExtortDefinition,
    ExtortMultipleInstances,
    LivingWeaponDefinition,
    MyriadDefinition,
    MyriadMultipleInstances,
    RetraceDefinition,
    BackupDefinition,
    BackupCopyOrdering,
    BackupPrintedAbilitiesOnly,
    BackupAbilitySnapshot,
    UmbraArmorDefinition,
    UmbraArmorOracleTerminology,
    CipherDefinition,
    CipherEncodedRelationship,
    CipherRelationshipDuration,
    RenownDefinition,
    RenownedDesignation,
    RenownMultipleInstances,
    TapPermanent,
    WardDefinition,
    WardVariable,
    ScryProcedure,
    ScryZero,
    ScrySimultaneous,
    ScryTriggerCompletion,
    SurveilProcedure,
    SurveilAdditionalCards,
    SurveilZero,
    SurveilTriggerCompletion,
    CyclingDefinition,
    CyclingZoneExistence,
    CyclingSelfTrigger,
    CyclingOrDiscardTrigger,
}

impl OfficialRule {
    pub const fn id(self) -> &'static str {
        match self {
            Self::MillDefinition => "701.17a",
            Self::MillAvailability => "701.17b",
            Self::Destroy => "701.8a",
            Self::RegenerateResolvingEffect => "701.19a",
            Self::RegenerateStaticEffect => "701.19b",
            Self::RegenerateCannotBeApplied => "701.19c",
            Self::SpellTargetSelection => "115.1a",
            Self::ResolveTargetLegality => "608.2b",
            Self::ResolveInstructionsInOrder => "608.2c",
            Self::ZoneChangeCreatesNewObject => "400.7",
            Self::ReplacementEffectChoice => "616.1",
            Self::ProtectionDefinition => "702.16a",
            Self::ProtectionTargeting => "702.16b",
            Self::ProtectionAuras => "702.16c",
            Self::ProtectionEquipmentAndFortifications => "702.16d",
            Self::ProtectionDamage => "702.16e",
            Self::ProtectionBlocking => "702.16f",
            Self::ProtectionMultipleQualities => "702.16g",
            Self::ProtectionEverything => "702.16j",
            Self::ProtectionPlayer => "702.16k",
            Self::FlyingDefinition => "702.9a",
            Self::FlyingBlocking => "702.9b",
            Self::FlyingRedundancy => "702.9c",
            Self::FightDefinition => "701.14a",
            Self::FightResolutionLegality => "701.14b",
            Self::InvestigateDefinition => "701.16a",
            Self::ClueTokenDefinition => "111.10f",
            Self::CastDeclareAdditionalOrAlternativeCost => "601.2b",
            Self::CastChooseTargets => "601.2c",
            Self::CastLegalityCheck => "601.2e",
            Self::CastDetermineTotalCost => "601.2f",
            Self::CastActivateManaAbilities => "601.2g",
            Self::CastPayTotalCost => "601.2h",
            Self::AdditionalCostDefinition => "118.8",
            Self::AdditionalCostMultiplicity => "118.8a",
            Self::AdditionalCostOptional => "118.8b",
            Self::AdditionalCostDoesNotChangeManaCost => "118.8d",
            Self::ModalSpellDefinition => "700.2",
            Self::ModalSpellControllerChooses => "700.2a",
            Self::ModalTargetsFollowChosenModes => "700.2c",
            Self::ModalModeNormallyChosenOnce => "700.2d",
            Self::ModalRetargetingDoesNotChangeMode => "700.2f",
            Self::ModalCopyRetainsModes => "700.2g",
            Self::ModalAssociatedAdditionalCosts => "700.2h",
            Self::KickerDefinition => "702.33a",
            Self::KickerMultipleCosts => "702.33b",
            Self::KickerDeclaration => "702.33d",
            Self::KickerLinkedAbilities => "702.33e",
            Self::FlashbackDefinition => "702.34a",
            Self::MorphDefinition => "702.37a",
            Self::MorphCastProcedure => "702.37c",
            Self::MorphSpecialAction => "702.37e",
            Self::FaceDownCharacteristics => "708.2a",
            Self::FlashDefinition => "702.8a",
            Self::FlashRedundancy => "702.8b",
            Self::MenaceDefinition => "702.111a",
            Self::MenaceBlocking => "702.111b",
            Self::MenaceRedundancy => "702.111c",
            Self::DefenderDefinition => "702.3a",
            Self::DefenderAttackRestriction => "702.3b",
            Self::DefenderRedundancy => "702.3c",
            Self::ReachDefinition => "702.17a",
            Self::ReachBlocking => "702.17b",
            Self::ReachRedundancy => "702.17c",
            Self::ShadowDefinition => "702.28a",
            Self::ShadowBlocking => "702.28b",
            Self::ShadowRedundancy => "702.28c",
            Self::LandwalkDefinition => "702.14a",
            Self::LandwalkEvasion => "702.14b",
            Self::LandwalkBlocking => "702.14c",
            Self::LandwalkNoCancellation => "702.14d",
            Self::LandwalkRedundancy => "702.14e",
            Self::FearDefinition => "702.36a",
            Self::FearBlocking => "702.36b",
            Self::FearRedundancy => "702.36c",
            Self::CharacteristicDefiningAbility => "604.3",
            Self::ChangelingDefinition => "702.73a",
            Self::DevoidDefinition => "702.114a",
            Self::AffinityDefinition => "702.41a",
            Self::AffinityMultipleInstances => "702.41b",
            Self::ConvokeDefinition => "702.51a",
            Self::ConvokeAfterTotalCost => "702.51b",
            Self::ConvokeTappedCreatureDesignation => "702.51c",
            Self::ConvokeRedundancy => "702.51d",
            Self::Attach => "701.3a",
            Self::EquipmentDefinition => "301.5",
            Self::EquipmentEntryAndEquip => "301.5b",
            Self::EquipmentLegality => "301.5c",
            Self::EquipmentControl => "301.5d",
            Self::EquipDefinition => "702.6a",
            Self::EquipQuality => "702.6c",
            Self::EquipMultipleAbilities => "702.6d",
            Self::EquipPlaneswalker => "702.6e",
            Self::AuraSpellTarget => "115.1b",
            Self::AuraDefinition => "303.4",
            Self::AuraTarget => "303.4a",
            Self::AuraIllegalAttachment => "303.4c",
            Self::AuraSelfAndCreatureRestriction => "303.4d",
            Self::AuraNoLegalEntry => "303.4g",
            Self::AuraBattlefieldAttachFailure => "303.4j",
            Self::EnchantDefinition => "702.5a",
            Self::EnchantMultipleRestrictions => "702.5c",
            Self::EnchantPlayer => "702.5d",
            Self::SagaChapterSymbol => "714.2a",
            Self::SagaChapterTrigger => "714.2b",
            Self::SagaFinalChapter => "714.2d",
            Self::SagaEntryLoreCounter => "714.3a",
            Self::SagaPrecombatLoreCounter => "714.3c",
            Self::SagaSacrifice => "714.4",
            Self::SagaStateBasedSacrifice => "704.5s",
            Self::Sacrifice => "701.21a",
            Self::PayCompleteCost => "118.3",
            Self::PayLife => "119.4",
            Self::CumulativeUpkeepDefinition => "702.24a",
            Self::CumulativeUpkeepSharedAgeCounters => "702.24b",
            Self::DelveDefinition => "702.66a",
            Self::DelveAfterTotalCost => "702.66b",
            Self::DelveRedundancy => "702.66c",
            Self::ExaltedDefinition => "702.83a",
            Self::ExaltedAttacksAlone => "702.83b",
            Self::CascadeDefinition => "702.85a",
            Self::CascadeActionWindow => "702.85b",
            Self::CascadeMultipleInstances => "702.85c",
            Self::ReboundDefinition => "702.88a",
            Self::ReboundAlternativeCost => "702.88b",
            Self::ReboundRedundancy => "702.88c",
            Self::InfectStatic => "702.90a",
            Self::InfectPlayerDamage => "702.90b",
            Self::InfectCreatureDamage => "702.90c",
            Self::InfectLastKnownInformation => "702.90d",
            Self::InfectAllZones => "702.90e",
            Self::InfectRedundancy => "702.90f",
            Self::FuseDefinition => "702.102a",
            Self::FuseCombinedCharacteristics => "702.102b",
            Self::FuseCombinedManaCost => "702.102c",
            Self::FuseResolutionOrder => "702.102d",
            Self::AftermathDefinition => "702.127a",
            Self::SplitCardSinglePhysicalCard => "709.2",
            Self::SplitCardChooseHalf => "709.3",
            Self::SplitCardOnlyChosenHalfEvaluated => "709.3a",
            Self::SplitSpellChosenHalfCharacteristics => "709.3b",
            Self::AscendSpell => "702.131a",
            Self::AscendPermanent => "702.131b",
            Self::AscendDesignation => "702.131c",
            Self::AscendContinuousEffectRecheck => "702.131d",
            Self::SummoningSickness => "302.6",
            Self::DeclareAttackerTap => "508.1f",
            Self::DeclareBlockerRestrictions => "509.1b",
            Self::ObjectColors => "105.2",
            Self::CombatDamageSteps => "510.4",
            Self::PlayerDamage => "120.3a",
            Self::CreatureDamage => "120.3e",
            Self::LifelinkDamage => "120.3f",
            Self::LethalDamageStateBasedAction => "704.5g",
            Self::DeathtouchStateBasedAction => "704.5h",
            Self::Discard => "701.9a",
            Self::Draw => "121.1",
            Self::EmptyLibraryDraw => "121.4",
            Self::HasteDefinition => "702.10a",
            Self::HasteAttack => "702.10b",
            Self::HasteTapAbility => "702.10c",
            Self::HasteRedundancy => "702.10d",
            Self::VigilanceDefinition => "702.20a",
            Self::VigilanceAttack => "702.20b",
            Self::VigilanceRedundancy => "702.20c",
            Self::TrampleDefinition => "702.19a",
            Self::TrampleAssignment => "702.19b",
            Self::TrampleNoBlockers => "702.19d",
            Self::TrampleRedundancy => "702.19g",
            Self::DeathtouchDefinition => "702.2a",
            Self::DeathtouchDestruction => "702.2b",
            Self::DeathtouchLethalAssignment => "702.2c",
            Self::DeathtouchZoneFunction => "702.2d",
            Self::DeathtouchRedundancy => "702.2f",
            Self::LifelinkDefinition => "702.15a",
            Self::LifelinkLifeGain => "702.15b",
            Self::LifelinkZoneFunction => "702.15d",
            Self::LifelinkSeparateEvents => "702.15e",
            Self::LifelinkRedundancy => "702.15f",
            Self::FirstStrikeDefinition => "702.7a",
            Self::FirstStrikeDamageSteps => "702.7b",
            Self::FirstStrikeChanges => "702.7c",
            Self::FirstStrikeRedundancy => "702.7d",
            Self::DoubleStrikeDefinition => "702.4a",
            Self::DoubleStrikeDamageSteps => "702.4b",
            Self::DoubleStrikeChanges => "702.4c",
            Self::DoubleStrikeGainedAfterFirst => "702.4d",
            Self::DoubleStrikeRedundancy => "702.4e",
            Self::HexproofDefinition => "702.11a",
            Self::HexproofPermanent => "702.11b",
            Self::HexproofPlayer => "702.11c",
            Self::HexproofQuality => "702.11d",
            Self::HexproofMultipleQualities => "702.11f",
            Self::ShroudDefinition => "702.18a",
            Self::ShroudRedundancy => "702.18b",
            Self::IndestructibleStatic => "702.12a",
            Self::IndestructibleDefinition => "702.12b",
            Self::IndestructibleRedundancy => "702.12c",
            Self::ProwessDefinition => "702.108a",
            Self::ProwessMultipleInstances => "702.108b",
            Self::BushidoDefinition => "702.45a",
            Self::BushidoMultipleInstances => "702.45b",
            Self::WitherDefinition => "702.80a",
            Self::WitherLastKnownInformation => "702.80b",
            Self::WitherAllZones => "702.80c",
            Self::WitherRedundancy => "702.80d",
            Self::HorsemanshipDefinition => "702.31a",
            Self::HorsemanshipBlocking => "702.31b",
            Self::HorsemanshipRedundancy => "702.31c",
            Self::FlankingDefinition => "702.25a",
            Self::FlankingMultipleInstances => "702.25b",
            Self::PersistDefinition => "702.79a",
            Self::UndyingDefinition => "702.93a",
            Self::ToxicDefinition => "702.164a",
            Self::ToxicTotalValue => "702.164b",
            Self::ToxicCombatDamage => "702.164c",
            Self::ZoneChangeTriggerFindsNewPublicObject => "400.7e",
            Self::TriggeredAbilityEvent => "603.2",
            Self::TriggeredAbilityOncePerEvent => "603.2c",
            Self::TriggeredAbilityStackLifecycle => "603.3",
            Self::TriggeredAbilityController => "603.3a",
            Self::InterveningIfClause => "603.4",
            Self::EntersBattlefieldTrigger => "603.6a",
            Self::ResolutionObjectInformation => "608.2h",
            Self::LeavesBattlefieldTrigger => "603.6c",
            Self::LeavesBattlefieldLookBack => "603.10a",
            Self::ReplacedEventNeverHappens => "614.6",
            Self::TokenOutsideBattlefieldCeases => "111.7",
            Self::TokenStateBasedActionCeases => "704.5d",
            Self::CounterDefinition => "122.1",
            Self::PowerToughnessCounterEffect => "122.1a",
            Self::CounterPlacedOnBattlefieldObject => "122.6",
            Self::CounterCeasesOnZoneChange => "122.2",
            Self::DamageProcessingSequence => "120.4",
            Self::DamageDealtAfterReplacementAndPrevention => "120.4b",
            Self::DamageProcessedIntoResults => "120.4c",
            Self::PoisonLossStateBasedAction => "104.3d",
            Self::AbilityDefaultZone => "113.6",
            Self::AbilityEntryModificationZone => "113.6h",
            Self::UntapDayNightTransition => "502.2",
            Self::UntapDayNightTeamTransition => "502.2a",
            Self::TransformDefinition => "701.27a",
            Self::TransformRequiresDoubleFaced => "701.27c",
            Self::TransformRejectsInstantSorceryBack => "701.27d",
            Self::InstantCannotEnterBattlefield => "304.4",
            Self::SorceryCannotEnterBattlefield => "307.4",
            Self::DoubleFacedEntryTransformed => "712.13a",
            Self::DoubleFacedNonstackEntry => "712.14",
            Self::DoubleFacedNonstackEntryTransformed => "712.14a",
            Self::TransformPreservesObject => "712.18",
            Self::DayNightFaces => "702.145a",
            Self::DayboundDefinition => "702.145b",
            Self::DayboundImmediateAlignment => "702.145c",
            Self::DayboundInitialDay => "702.145d",
            Self::NightboundDefinition => "702.145e",
            Self::NightboundImmediateAlignment => "702.145f",
            Self::NightboundInitialNight => "702.145g",
            Self::DayNightDesignation => "731.1",
            Self::DayNightUntapCheck => "731.2",
            Self::DayToNightSpellCount => "731.2a",
            Self::NightToDaySpellCount => "731.2b",
            Self::StartYourEnginesDefinition => "702.179a",
            Self::SpeedAbsentUntilSet => "702.179b",
            Self::SpeedIncreaseFromNoSpeed => "702.179c",
            Self::SpeedInherentTrigger => "702.179d",
            Self::SpeedMaximum => "702.179e",
            Self::SpeedNoSpeedAsZero => "702.179f",
            Self::SpeedStateBasedInitialization => "704.5z",
            Self::DeckConstructionAbilityZone => "113.6n",
            Self::CommanderDesignation => "903.3",
            Self::CommanderDeckConstructionAbility => "903.3a",
            Self::CommanderColorIdentity => "903.4",
            Self::CommanderColorIdentityLockedBeforeGame => "903.4a",
            Self::CommanderDeckSize => "903.5a",
            Self::CommanderStartsInCommandZone => "903.6",
            Self::CommanderTax => "903.8",
            Self::CommanderDamageLoss => "903.10a",
            Self::PartnerAbilityDefinition => "702.124a",
            Self::PartnerDeckSize => "702.124b",
            Self::PartnerCombinedColorIdentity => "702.124c",
            Self::PartnerIndependentCommanders => "702.124d",
            Self::PartnerCommanderReference => "702.124e",
            Self::PartnerVariantsDistinct => "702.124f",
            Self::PartnerAtMostTwo => "702.124g",
            Self::ChooseBackgroundDefinition => "702.124k",
            Self::DoctorsCompanionDefinition => "702.124m",
            Self::PartnerNamedReferenceScope => "702.124n",
            Self::ExploitDefinition => "702.110a",
            Self::ExploitEventDefinition => "702.110b",
            Self::SoulbondDefinition => "702.95a",
            Self::SoulbondPairedDesignation => "702.95b",
            Self::SoulbondResolutionRecheck => "702.95c",
            Self::SoulbondOnePartnerLimit => "702.95d",
            Self::SoulbondUnpairEvents => "702.95e",
            Self::EvolveDefinition => "702.100a",
            Self::EvolveEventDefinition => "702.100b",
            Self::EvolveNoncreatureComparison => "702.100c",
            Self::EvolveMultipleInstances => "702.100d",
            Self::ImproviseDefinition => "702.126a",
            Self::ImproviseAfterTotalCost => "702.126b",
            Self::ImproviseRedundancy => "702.126c",
            Self::IntimidateDefinition => "702.13a",
            Self::IntimidateBlocking => "702.13b",
            Self::IntimidateRedundancy => "702.13c",
            Self::SpreeDefinition => "702.172a",
            Self::SpreeSymbols => "702.172b",
            Self::BargainDefinition => "702.166a",
            Self::BargainDeclaredStatus => "702.166b",
            Self::BargainLinkedAbilities => "702.166c",
            Self::BargainConditionalTargets => "702.166d",
            Self::MentorDefinition => "702.134a",
            Self::MentorMultipleInstances => "702.134b",
            Self::MentorEventDefinition => "702.134c",
            Self::ExtortDefinition => "702.101a",
            Self::ExtortMultipleInstances => "702.101b",
            Self::LivingWeaponDefinition => "702.92a",
            Self::MyriadDefinition => "702.116a",
            Self::MyriadMultipleInstances => "702.116b",
            Self::RetraceDefinition => "702.81a",
            Self::BackupDefinition => "702.165a",
            Self::BackupCopyOrdering => "702.165b",
            Self::BackupPrintedAbilitiesOnly => "702.165c",
            Self::BackupAbilitySnapshot => "702.165d",
            Self::UmbraArmorDefinition => "702.89a",
            Self::UmbraArmorOracleTerminology => "702.89b",
            Self::CipherDefinition => "702.99a",
            Self::CipherEncodedRelationship => "702.99b",
            Self::CipherRelationshipDuration => "702.99c",
            Self::RenownDefinition => "702.112a",
            Self::RenownedDesignation => "702.112b",
            Self::RenownMultipleInstances => "702.112c",
            Self::TapPermanent => "701.26a",
            Self::WardDefinition => "702.21a",
            Self::WardVariable => "702.21b",
            Self::ScryProcedure => "701.22a",
            Self::ScryZero => "701.22b",
            Self::ScrySimultaneous => "701.22c",
            Self::ScryTriggerCompletion => "701.22d",
            Self::SurveilProcedure => "701.25a",
            Self::SurveilAdditionalCards => "701.25b",
            Self::SurveilZero => "701.25c",
            Self::SurveilTriggerCompletion => "701.25d",
            Self::CyclingDefinition => "702.29a",
            Self::CyclingZoneExistence => "702.29b",
            Self::CyclingSelfTrigger => "702.29c",
            Self::CyclingOrDiscardTrigger => "702.29d",
        }
    }
}

const MILL_RULES: &[OfficialRule] = &[OfficialRule::MillDefinition, OfficialRule::MillAvailability];
const REGENERATE_SOURCE_RULES: &[OfficialRule] = &[
    OfficialRule::Destroy,
    OfficialRule::RegenerateResolvingEffect,
    OfficialRule::RegenerateCannotBeApplied,
    OfficialRule::ZoneChangeCreatesNewObject,
    OfficialRule::ReplacementEffectChoice,
];
const REGENERATE_TARGET_RULES: &[OfficialRule] = &[
    OfficialRule::Destroy,
    OfficialRule::RegenerateResolvingEffect,
    OfficialRule::RegenerateCannotBeApplied,
    OfficialRule::SpellTargetSelection,
    OfficialRule::ResolveTargetLegality,
    OfficialRule::ZoneChangeCreatesNewObject,
    OfficialRule::ReplacementEffectChoice,
];
const REGENERATE_CONTROLLED_CREATURE_SET_RULES: &[OfficialRule] = &[
    OfficialRule::Destroy,
    OfficialRule::RegenerateResolvingEffect,
    OfficialRule::RegenerateCannotBeApplied,
    OfficialRule::ResolveInstructionsInOrder,
    OfficialRule::ZoneChangeCreatesNewObject,
    OfficialRule::ReplacementEffectChoice,
];
const REGENERATE_STATIC_RULES: &[OfficialRule] = &[
    OfficialRule::Destroy,
    OfficialRule::RegenerateStaticEffect,
    OfficialRule::RegenerateCannotBeApplied,
    OfficialRule::ZoneChangeCreatesNewObject,
    OfficialRule::ReplacementEffectChoice,
];
const PROTECTION_RULES: &[OfficialRule] = &[
    OfficialRule::ProtectionDefinition,
    OfficialRule::ProtectionTargeting,
    OfficialRule::ProtectionAuras,
    OfficialRule::ProtectionEquipmentAndFortifications,
    OfficialRule::ProtectionDamage,
    OfficialRule::ProtectionBlocking,
    OfficialRule::ProtectionMultipleQualities,
    OfficialRule::ProtectionEverything,
    OfficialRule::ProtectionPlayer,
];
const FLYING_RULES: &[OfficialRule] = &[
    OfficialRule::FlyingDefinition,
    OfficialRule::FlyingBlocking,
    OfficialRule::FlyingRedundancy,
];
const FIGHT_RULES: &[OfficialRule] = &[
    OfficialRule::FightDefinition,
    OfficialRule::FightResolutionLegality,
];
const INVESTIGATE_RULES: &[OfficialRule] = &[
    OfficialRule::InvestigateDefinition,
    OfficialRule::ClueTokenDefinition,
];
const KICKER_RULES: &[OfficialRule] = &[
    OfficialRule::CastDeclareAdditionalOrAlternativeCost,
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::KickerDefinition,
    OfficialRule::KickerMultipleCosts,
    OfficialRule::KickerDeclaration,
    OfficialRule::KickerLinkedAbilities,
];
const FLASHBACK_RULES: &[OfficialRule] = &[
    OfficialRule::CastDeclareAdditionalOrAlternativeCost,
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::FlashbackDefinition,
];
const MORPH_RULES: &[OfficialRule] = &[
    OfficialRule::CastDeclareAdditionalOrAlternativeCost,
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::MorphDefinition,
    OfficialRule::MorphCastProcedure,
    OfficialRule::MorphSpecialAction,
    OfficialRule::FaceDownCharacteristics,
];
const FLASH_RULES: &[OfficialRule] =
    &[OfficialRule::FlashDefinition, OfficialRule::FlashRedundancy];
const MENACE_RULES: &[OfficialRule] = &[
    OfficialRule::MenaceDefinition,
    OfficialRule::MenaceBlocking,
    OfficialRule::MenaceRedundancy,
];
const DEFENDER_RULES: &[OfficialRule] = &[
    OfficialRule::DefenderDefinition,
    OfficialRule::DefenderAttackRestriction,
    OfficialRule::DefenderRedundancy,
];
const REACH_RULES: &[OfficialRule] = &[
    OfficialRule::ReachDefinition,
    OfficialRule::ReachBlocking,
    OfficialRule::ReachRedundancy,
];
const DEVOID_RULES: &[OfficialRule] = &[
    OfficialRule::CharacteristicDefiningAbility,
    OfficialRule::DevoidDefinition,
];
const CONVOKE_RULES: &[OfficialRule] = &[
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::ConvokeDefinition,
    OfficialRule::ConvokeAfterTotalCost,
    OfficialRule::ConvokeTappedCreatureDesignation,
    OfficialRule::ConvokeRedundancy,
];
const EQUIP_RULES: &[OfficialRule] = &[
    OfficialRule::Attach,
    OfficialRule::EquipmentDefinition,
    OfficialRule::EquipmentEntryAndEquip,
    OfficialRule::EquipmentLegality,
    OfficialRule::EquipmentControl,
    OfficialRule::EquipDefinition,
    OfficialRule::EquipQuality,
    OfficialRule::EquipMultipleAbilities,
    OfficialRule::EquipPlaneswalker,
];
const ENCHANT_RULES: &[OfficialRule] = &[
    OfficialRule::Attach,
    OfficialRule::AuraSpellTarget,
    OfficialRule::AuraDefinition,
    OfficialRule::AuraTarget,
    OfficialRule::AuraIllegalAttachment,
    OfficialRule::AuraSelfAndCreatureRestriction,
    OfficialRule::AuraNoLegalEntry,
    OfficialRule::AuraBattlefieldAttachFailure,
    OfficialRule::EnchantDefinition,
    OfficialRule::EnchantMultipleRestrictions,
    OfficialRule::EnchantPlayer,
];
const SAGA_RULES: &[OfficialRule] = &[
    OfficialRule::SagaChapterSymbol,
    OfficialRule::SagaChapterTrigger,
    OfficialRule::SagaFinalChapter,
    OfficialRule::SagaEntryLoreCounter,
    OfficialRule::SagaPrecombatLoreCounter,
    OfficialRule::SagaSacrifice,
    OfficialRule::SagaStateBasedSacrifice,
    OfficialRule::Sacrifice,
];
const CUMULATIVE_UPKEEP_RULES: &[OfficialRule] = &[
    OfficialRule::PayCompleteCost,
    OfficialRule::PayLife,
    OfficialRule::CumulativeUpkeepDefinition,
    OfficialRule::CumulativeUpkeepSharedAgeCounters,
    OfficialRule::Sacrifice,
];
const HASTE_RULES: &[OfficialRule] = &[
    OfficialRule::SummoningSickness,
    OfficialRule::HasteDefinition,
    OfficialRule::HasteAttack,
    OfficialRule::HasteTapAbility,
    OfficialRule::HasteRedundancy,
];
const VIGILANCE_RULES: &[OfficialRule] = &[
    OfficialRule::DeclareAttackerTap,
    OfficialRule::VigilanceDefinition,
    OfficialRule::VigilanceAttack,
    OfficialRule::VigilanceRedundancy,
];
const TRAMPLE_RULES: &[OfficialRule] = &[
    OfficialRule::CreatureDamage,
    OfficialRule::PlayerDamage,
    OfficialRule::TrampleDefinition,
    OfficialRule::TrampleAssignment,
    OfficialRule::TrampleNoBlockers,
    OfficialRule::TrampleRedundancy,
];
const DEATHTOUCH_RULES: &[OfficialRule] = &[
    OfficialRule::DeathtouchDefinition,
    OfficialRule::DeathtouchDestruction,
    OfficialRule::DeathtouchLethalAssignment,
    OfficialRule::DeathtouchZoneFunction,
    OfficialRule::DeathtouchRedundancy,
    OfficialRule::DeathtouchStateBasedAction,
];
const LIFELINK_RULES: &[OfficialRule] = &[
    OfficialRule::PlayerDamage,
    OfficialRule::CreatureDamage,
    OfficialRule::LifelinkDamage,
    OfficialRule::LifelinkDefinition,
    OfficialRule::LifelinkLifeGain,
    OfficialRule::LifelinkZoneFunction,
    OfficialRule::LifelinkSeparateEvents,
    OfficialRule::LifelinkRedundancy,
];
const FIRST_STRIKE_RULES: &[OfficialRule] = &[
    OfficialRule::CombatDamageSteps,
    OfficialRule::FirstStrikeDefinition,
    OfficialRule::FirstStrikeDamageSteps,
    OfficialRule::FirstStrikeChanges,
    OfficialRule::FirstStrikeRedundancy,
];
const DOUBLE_STRIKE_RULES: &[OfficialRule] = &[
    OfficialRule::CombatDamageSteps,
    OfficialRule::DoubleStrikeDefinition,
    OfficialRule::DoubleStrikeDamageSteps,
    OfficialRule::DoubleStrikeChanges,
    OfficialRule::DoubleStrikeGainedAfterFirst,
    OfficialRule::DoubleStrikeRedundancy,
];
const HEXPROOF_RULES: &[OfficialRule] = &[
    OfficialRule::HexproofDefinition,
    OfficialRule::HexproofPermanent,
    OfficialRule::HexproofPlayer,
    OfficialRule::HexproofQuality,
    OfficialRule::HexproofMultipleQualities,
];
const SHROUD_RULES: &[OfficialRule] = &[
    OfficialRule::ShroudDefinition,
    OfficialRule::ShroudRedundancy,
];
const INDESTRUCTIBLE_RULES: &[OfficialRule] = &[
    OfficialRule::IndestructibleStatic,
    OfficialRule::IndestructibleDefinition,
    OfficialRule::IndestructibleRedundancy,
    OfficialRule::LethalDamageStateBasedAction,
    OfficialRule::DeathtouchStateBasedAction,
];
const PROWESS_RULES: &[OfficialRule] = &[
    OfficialRule::ProwessDefinition,
    OfficialRule::ProwessMultipleInstances,
];
const BUSHIDO_RULES: &[OfficialRule] = &[
    OfficialRule::BushidoDefinition,
    OfficialRule::BushidoMultipleInstances,
];
const WITHER_RULES: &[OfficialRule] = &[
    OfficialRule::WitherDefinition,
    OfficialRule::WitherLastKnownInformation,
    OfficialRule::WitherAllZones,
    OfficialRule::WitherRedundancy,
];
const HORSEMANSHIP_RULES: &[OfficialRule] = &[
    OfficialRule::HorsemanshipDefinition,
    OfficialRule::HorsemanshipBlocking,
    OfficialRule::HorsemanshipRedundancy,
];
const FLANKING_RULES: &[OfficialRule] = &[
    OfficialRule::FlankingDefinition,
    OfficialRule::FlankingMultipleInstances,
];
const PERSIST_RULES: &[OfficialRule] = &[
    OfficialRule::ZoneChangeCreatesNewObject,
    OfficialRule::ZoneChangeTriggerFindsNewPublicObject,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityOncePerEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::LeavesBattlefieldTrigger,
    OfficialRule::LeavesBattlefieldLookBack,
    OfficialRule::ReplacedEventNeverHappens,
    OfficialRule::TokenOutsideBattlefieldCeases,
    OfficialRule::TokenStateBasedActionCeases,
    OfficialRule::CounterDefinition,
    OfficialRule::PowerToughnessCounterEffect,
    OfficialRule::CounterCeasesOnZoneChange,
    OfficialRule::PersistDefinition,
];
const UNDYING_RULES: &[OfficialRule] = &[
    OfficialRule::ZoneChangeCreatesNewObject,
    OfficialRule::ZoneChangeTriggerFindsNewPublicObject,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityOncePerEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::LeavesBattlefieldTrigger,
    OfficialRule::LeavesBattlefieldLookBack,
    OfficialRule::ReplacedEventNeverHappens,
    OfficialRule::TokenOutsideBattlefieldCeases,
    OfficialRule::TokenStateBasedActionCeases,
    OfficialRule::CounterDefinition,
    OfficialRule::PowerToughnessCounterEffect,
    OfficialRule::CounterCeasesOnZoneChange,
    OfficialRule::UndyingDefinition,
];
const TOXIC_RULES: &[OfficialRule] = &[
    OfficialRule::PoisonLossStateBasedAction,
    OfficialRule::PlayerDamage,
    OfficialRule::DamageProcessingSequence,
    OfficialRule::DamageDealtAfterReplacementAndPrevention,
    OfficialRule::DamageProcessedIntoResults,
    OfficialRule::CounterDefinition,
    OfficialRule::ToxicDefinition,
    OfficialRule::ToxicTotalValue,
    OfficialRule::ToxicCombatDamage,
];
const DAYBOUND_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::AbilityEntryModificationZone,
    OfficialRule::UntapDayNightTransition,
    OfficialRule::UntapDayNightTeamTransition,
    OfficialRule::TransformDefinition,
    OfficialRule::TransformRequiresDoubleFaced,
    OfficialRule::TransformRejectsInstantSorceryBack,
    OfficialRule::InstantCannotEnterBattlefield,
    OfficialRule::SorceryCannotEnterBattlefield,
    OfficialRule::DoubleFacedEntryTransformed,
    OfficialRule::DoubleFacedNonstackEntry,
    OfficialRule::DoubleFacedNonstackEntryTransformed,
    OfficialRule::TransformPreservesObject,
    OfficialRule::DayNightFaces,
    OfficialRule::DayboundDefinition,
    OfficialRule::DayboundImmediateAlignment,
    OfficialRule::DayboundInitialDay,
    OfficialRule::DayNightDesignation,
    OfficialRule::DayNightUntapCheck,
    OfficialRule::DayToNightSpellCount,
    OfficialRule::NightToDaySpellCount,
];
const NIGHTBOUND_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::UntapDayNightTransition,
    OfficialRule::UntapDayNightTeamTransition,
    OfficialRule::TransformDefinition,
    OfficialRule::TransformRequiresDoubleFaced,
    OfficialRule::TransformRejectsInstantSorceryBack,
    OfficialRule::TransformPreservesObject,
    OfficialRule::DayNightFaces,
    OfficialRule::NightboundDefinition,
    OfficialRule::NightboundImmediateAlignment,
    OfficialRule::NightboundInitialNight,
    OfficialRule::DayNightDesignation,
    OfficialRule::DayNightUntapCheck,
    OfficialRule::DayToNightSpellCount,
    OfficialRule::NightToDaySpellCount,
];
const START_YOUR_ENGINES_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::StartYourEnginesDefinition,
    OfficialRule::SpeedAbsentUntilSet,
    OfficialRule::SpeedIncreaseFromNoSpeed,
    OfficialRule::SpeedInherentTrigger,
    OfficialRule::SpeedMaximum,
    OfficialRule::SpeedNoSpeedAsZero,
    OfficialRule::SpeedStateBasedInitialization,
];
const CHOOSE_A_BACKGROUND_RULES: &[OfficialRule] = &[
    OfficialRule::DeckConstructionAbilityZone,
    OfficialRule::CommanderDesignation,
    OfficialRule::CommanderDeckConstructionAbility,
    OfficialRule::CommanderColorIdentity,
    OfficialRule::CommanderColorIdentityLockedBeforeGame,
    OfficialRule::CommanderDeckSize,
    OfficialRule::CommanderStartsInCommandZone,
    OfficialRule::CommanderTax,
    OfficialRule::CommanderDamageLoss,
    OfficialRule::PartnerAbilityDefinition,
    OfficialRule::PartnerDeckSize,
    OfficialRule::PartnerCombinedColorIdentity,
    OfficialRule::PartnerIndependentCommanders,
    OfficialRule::PartnerCommanderReference,
    OfficialRule::PartnerVariantsDistinct,
    OfficialRule::PartnerAtMostTwo,
    OfficialRule::ChooseBackgroundDefinition,
    OfficialRule::PartnerNamedReferenceScope,
];
const DOCTORS_COMPANION_RULES: &[OfficialRule] = &[
    OfficialRule::DeckConstructionAbilityZone,
    OfficialRule::CommanderDesignation,
    OfficialRule::CommanderDeckConstructionAbility,
    OfficialRule::CommanderColorIdentity,
    OfficialRule::CommanderColorIdentityLockedBeforeGame,
    OfficialRule::CommanderDeckSize,
    OfficialRule::CommanderStartsInCommandZone,
    OfficialRule::CommanderTax,
    OfficialRule::CommanderDamageLoss,
    OfficialRule::PartnerAbilityDefinition,
    OfficialRule::PartnerDeckSize,
    OfficialRule::PartnerCombinedColorIdentity,
    OfficialRule::PartnerIndependentCommanders,
    OfficialRule::PartnerCommanderReference,
    OfficialRule::PartnerVariantsDistinct,
    OfficialRule::PartnerAtMostTwo,
    OfficialRule::DoctorsCompanionDefinition,
    OfficialRule::PartnerNamedReferenceScope,
];
const EXPLOIT_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::ReplacementEffectChoice,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::TriggeredAbilityController,
    OfficialRule::EntersBattlefieldTrigger,
    OfficialRule::Sacrifice,
    OfficialRule::ExploitDefinition,
    OfficialRule::ExploitEventDefinition,
];
const SOULBOND_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityOncePerEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::TriggeredAbilityController,
    OfficialRule::InterveningIfClause,
    OfficialRule::EntersBattlefieldTrigger,
    OfficialRule::SoulbondDefinition,
    OfficialRule::SoulbondPairedDesignation,
    OfficialRule::SoulbondResolutionRecheck,
    OfficialRule::SoulbondOnePartnerLimit,
    OfficialRule::SoulbondUnpairEvents,
];
const EVOLVE_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityOncePerEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::TriggeredAbilityController,
    OfficialRule::InterveningIfClause,
    OfficialRule::EntersBattlefieldTrigger,
    OfficialRule::ResolutionObjectInformation,
    OfficialRule::CounterDefinition,
    OfficialRule::PowerToughnessCounterEffect,
    OfficialRule::CounterPlacedOnBattlefieldObject,
    OfficialRule::EvolveDefinition,
    OfficialRule::EvolveEventDefinition,
    OfficialRule::EvolveNoncreatureComparison,
    OfficialRule::EvolveMultipleInstances,
];
const IMPROVISE_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::SummoningSickness,
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastActivateManaAbilities,
    OfficialRule::CastPayTotalCost,
    OfficialRule::TapPermanent,
    OfficialRule::ImproviseDefinition,
    OfficialRule::ImproviseAfterTotalCost,
    OfficialRule::ImproviseRedundancy,
];
const INTIMIDATE_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::DeclareBlockerRestrictions,
    OfficialRule::ObjectColors,
    OfficialRule::IntimidateDefinition,
    OfficialRule::IntimidateBlocking,
    OfficialRule::IntimidateRedundancy,
];
const SPREE_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::ResolveInstructionsInOrder,
    OfficialRule::CastDeclareAdditionalOrAlternativeCost,
    OfficialRule::CastChooseTargets,
    OfficialRule::CastLegalityCheck,
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::AdditionalCostDefinition,
    OfficialRule::AdditionalCostMultiplicity,
    OfficialRule::AdditionalCostOptional,
    OfficialRule::AdditionalCostDoesNotChangeManaCost,
    OfficialRule::ModalSpellDefinition,
    OfficialRule::ModalSpellControllerChooses,
    OfficialRule::ModalTargetsFollowChosenModes,
    OfficialRule::ModalModeNormallyChosenOnce,
    OfficialRule::ModalRetargetingDoesNotChangeMode,
    OfficialRule::ModalCopyRetainsModes,
    OfficialRule::ModalAssociatedAdditionalCosts,
    OfficialRule::SpreeDefinition,
    OfficialRule::SpreeSymbols,
];
const BARGAIN_RULES: &[OfficialRule] = &[
    OfficialRule::CastDeclareAdditionalOrAlternativeCost,
    OfficialRule::CastChooseTargets,
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::AdditionalCostDefinition,
    OfficialRule::AdditionalCostOptional,
    OfficialRule::AdditionalCostDoesNotChangeManaCost,
    OfficialRule::Sacrifice,
    OfficialRule::BargainDefinition,
    OfficialRule::BargainDeclaredStatus,
    OfficialRule::BargainLinkedAbilities,
    OfficialRule::BargainConditionalTargets,
];
const MENTOR_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityOncePerEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::TriggeredAbilityController,
    OfficialRule::CounterDefinition,
    OfficialRule::PowerToughnessCounterEffect,
    OfficialRule::CounterPlacedOnBattlefieldObject,
    OfficialRule::MentorDefinition,
    OfficialRule::MentorMultipleInstances,
    OfficialRule::MentorEventDefinition,
];
const EXTORT_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityOncePerEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::TriggeredAbilityController,
    OfficialRule::PayCompleteCost,
    OfficialRule::ExtortDefinition,
    OfficialRule::ExtortMultipleInstances,
];
const LIVING_WEAPON_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::EntersBattlefieldTrigger,
    OfficialRule::ResolveInstructionsInOrder,
    OfficialRule::Attach,
    OfficialRule::EquipmentLegality,
    OfficialRule::LivingWeaponDefinition,
];
const MYRIAD_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityOncePerEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::TriggeredAbilityController,
    OfficialRule::MyriadDefinition,
    OfficialRule::MyriadMultipleInstances,
];
const RETRACE_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::CastDeclareAdditionalOrAlternativeCost,
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::AdditionalCostDefinition,
    OfficialRule::Discard,
    OfficialRule::RetraceDefinition,
];
const BACKUP_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::TriggeredAbilityController,
    OfficialRule::EntersBattlefieldTrigger,
    OfficialRule::CounterDefinition,
    OfficialRule::PowerToughnessCounterEffect,
    OfficialRule::CounterPlacedOnBattlefieldObject,
    OfficialRule::BackupDefinition,
    OfficialRule::BackupCopyOrdering,
    OfficialRule::BackupPrintedAbilitiesOnly,
    OfficialRule::BackupAbilitySnapshot,
];
const UMBRA_ARMOR_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::Destroy,
    OfficialRule::Attach,
    OfficialRule::AuraIllegalAttachment,
    OfficialRule::UmbraArmorDefinition,
    OfficialRule::UmbraArmorOracleTerminology,
];
const CIPHER_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::CastPayTotalCost,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::TriggeredAbilityController,
    OfficialRule::ZoneChangeCreatesNewObject,
    OfficialRule::CipherDefinition,
    OfficialRule::CipherEncodedRelationship,
    OfficialRule::CipherRelationshipDuration,
];
const RENOWN_RULES: &[OfficialRule] = &[
    OfficialRule::AbilityDefaultZone,
    OfficialRule::TriggeredAbilityEvent,
    OfficialRule::TriggeredAbilityOncePerEvent,
    OfficialRule::TriggeredAbilityStackLifecycle,
    OfficialRule::TriggeredAbilityController,
    OfficialRule::CounterDefinition,
    OfficialRule::PowerToughnessCounterEffect,
    OfficialRule::CounterPlacedOnBattlefieldObject,
    OfficialRule::CounterCeasesOnZoneChange,
    OfficialRule::RenownDefinition,
    OfficialRule::RenownedDesignation,
    OfficialRule::RenownMultipleInstances,
];
const WARD_RULES: &[OfficialRule] = &[
    OfficialRule::WardDefinition,
    OfficialRule::WardVariable,
    OfficialRule::CastPayTotalCost,
];
const SCRY_RULES: &[OfficialRule] = &[
    OfficialRule::ScryProcedure,
    OfficialRule::ScryZero,
    OfficialRule::ScrySimultaneous,
    OfficialRule::ScryTriggerCompletion,
];
const SURVEIL_RULES: &[OfficialRule] = &[
    OfficialRule::SurveilProcedure,
    OfficialRule::SurveilAdditionalCards,
    OfficialRule::SurveilZero,
    OfficialRule::SurveilTriggerCompletion,
];
const CYCLING_RULES: &[OfficialRule] = &[
    OfficialRule::Discard,
    OfficialRule::Draw,
    OfficialRule::EmptyLibraryDraw,
    OfficialRule::CastPayTotalCost,
    OfficialRule::CyclingDefinition,
    OfficialRule::CyclingZoneExistence,
    OfficialRule::CyclingSelfTrigger,
    OfficialRule::CyclingOrDiscardTrigger,
];
const CHANGELING_RULES: &[OfficialRule] = &[
    OfficialRule::CharacteristicDefiningAbility,
    OfficialRule::ChangelingDefinition,
];
const INFECT_RULES: &[OfficialRule] = &[
    OfficialRule::InfectStatic,
    OfficialRule::InfectPlayerDamage,
    OfficialRule::InfectCreatureDamage,
    OfficialRule::InfectLastKnownInformation,
    OfficialRule::InfectAllZones,
    OfficialRule::InfectRedundancy,
];
const FEAR_RULES: &[OfficialRule] = &[
    OfficialRule::FearDefinition,
    OfficialRule::FearBlocking,
    OfficialRule::FearRedundancy,
];
const SHADOW_RULES: &[OfficialRule] = &[
    OfficialRule::ShadowDefinition,
    OfficialRule::ShadowBlocking,
    OfficialRule::ShadowRedundancy,
];
const LANDWALK_RULES: &[OfficialRule] = &[
    OfficialRule::LandwalkDefinition,
    OfficialRule::LandwalkEvasion,
    OfficialRule::LandwalkBlocking,
    OfficialRule::LandwalkNoCancellation,
    OfficialRule::LandwalkRedundancy,
];
const AFFINITY_RULES: &[OfficialRule] = &[
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::AffinityDefinition,
    OfficialRule::AffinityMultipleInstances,
];
const CASCADE_RULES: &[OfficialRule] = &[
    OfficialRule::CastDeclareAdditionalOrAlternativeCost,
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::CascadeDefinition,
    OfficialRule::CascadeActionWindow,
    OfficialRule::CascadeMultipleInstances,
];
const DELVE_RULES: &[OfficialRule] = &[
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::DelveDefinition,
    OfficialRule::DelveAfterTotalCost,
    OfficialRule::DelveRedundancy,
];
const FUSE_RULES: &[OfficialRule] = &[
    OfficialRule::CastDeclareAdditionalOrAlternativeCost,
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::FuseDefinition,
    OfficialRule::FuseCombinedCharacteristics,
    OfficialRule::FuseCombinedManaCost,
    OfficialRule::FuseResolutionOrder,
    OfficialRule::SplitCardSinglePhysicalCard,
    OfficialRule::SplitCardChooseHalf,
    OfficialRule::SplitCardOnlyChosenHalfEvaluated,
    OfficialRule::SplitSpellChosenHalfCharacteristics,
];
const AFTERMATH_RULES: &[OfficialRule] = &[
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::AftermathDefinition,
    OfficialRule::SplitCardSinglePhysicalCard,
    OfficialRule::SplitCardChooseHalf,
    OfficialRule::SplitCardOnlyChosenHalfEvaluated,
    OfficialRule::SplitSpellChosenHalfCharacteristics,
];
const REBOUND_RULES: &[OfficialRule] = &[
    OfficialRule::CastDeclareAdditionalOrAlternativeCost,
    OfficialRule::CastDetermineTotalCost,
    OfficialRule::CastPayTotalCost,
    OfficialRule::ReboundDefinition,
    OfficialRule::ReboundAlternativeCost,
    OfficialRule::ReboundRedundancy,
];
const EXALTED_RULES: &[OfficialRule] = &[
    OfficialRule::ExaltedDefinition,
    OfficialRule::ExaltedAttacksAlone,
];
const ASCEND_RULES: &[OfficialRule] = &[
    OfficialRule::AscendSpell,
    OfficialRule::AscendPermanent,
    OfficialRule::AscendDesignation,
    OfficialRule::AscendContinuousEffectRecheck,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OfficialKeyword {
    Mill,
    Regenerate,
    Protection,
    Flying,
    Fight,
    Investigate,
    Kicker,
    Flashback,
    Morph,
    Flash,
    Menace,
    Defender,
    Reach,
    Changeling,
    Infect,
    Fear,
    Shadow,
    Landwalk,
    Affinity,
    Cascade,
    Delve,
    Fuse,
    Aftermath,
    Rebound,
    Exalted,
    Ascend,
    Devoid,
    Convoke,
    Equip,
    Enchant,
    Saga,
    CumulativeUpkeep,
    Haste,
    Vigilance,
    Trample,
    Deathtouch,
    Lifelink,
    FirstStrike,
    DoubleStrike,
    Hexproof,
    Shroud,
    Indestructible,
    Prowess,
    Bushido,
    Wither,
    Horsemanship,
    Flanking,
    Persist,
    Undying,
    Toxic,
    Daybound,
    Nightbound,
    StartYourEngines,
    ChooseABackground,
    DoctorsCompanion,
    Exploit,
    Soulbond,
    Evolve,
    Improvise,
    Intimidate,
    Spree,
    Bargain,
    Mentor,
    Extort,
    LivingWeapon,
    Myriad,
    Retrace,
    Backup,
    UmbraArmor,
    Cipher,
    Renown,
    Ward,
    Scry,
    Surveil,
    Cycling,
}

impl OfficialKeyword {
    pub const fn printed_label(self) -> &'static str {
        match self {
            Self::Mill => "Mill",
            Self::Regenerate => "Regenerate",
            Self::Protection => "Protection",
            Self::Flying => "Flying",
            Self::Fight => "Fight",
            Self::Investigate => "Investigate",
            Self::Kicker => "Kicker",
            Self::Flashback => "Flashback",
            Self::Morph => "Morph",
            Self::Flash => "Flash",
            Self::Menace => "Menace",
            Self::Defender => "Defender",
            Self::Reach => "Reach",
            Self::Changeling => "Changeling",
            Self::Infect => "Infect",
            Self::Fear => "Fear",
            Self::Shadow => "Shadow",
            Self::Landwalk => "Landwalk",
            Self::Affinity => "Affinity",
            Self::Cascade => "Cascade",
            Self::Delve => "Delve",
            Self::Fuse => "Fuse",
            Self::Aftermath => "Aftermath",
            Self::Rebound => "Rebound",
            Self::Exalted => "Exalted",
            Self::Ascend => "Ascend",
            Self::Devoid => "Devoid",
            Self::Convoke => "Convoke",
            Self::Equip => "Equip",
            Self::Enchant => "Enchant",
            Self::Saga => "Saga",
            Self::CumulativeUpkeep => "Cumulative upkeep",
            Self::Haste => "Haste",
            Self::Vigilance => "Vigilance",
            Self::Trample => "Trample",
            Self::Deathtouch => "Deathtouch",
            Self::Lifelink => "Lifelink",
            Self::FirstStrike => "First strike",
            Self::DoubleStrike => "Double strike",
            Self::Hexproof => "Hexproof",
            Self::Shroud => "Shroud",
            Self::Indestructible => "Indestructible",
            Self::Prowess => "Prowess",
            Self::Bushido => "Bushido",
            Self::Wither => "Wither",
            Self::Horsemanship => "Horsemanship",
            Self::Flanking => "Flanking",
            Self::Persist => "Persist",
            Self::Undying => "Undying",
            Self::Toxic => "Toxic",
            Self::Daybound => "Daybound",
            Self::Nightbound => "Nightbound",
            Self::StartYourEngines => "Start your engines!",
            Self::ChooseABackground => "Choose a Background",
            Self::DoctorsCompanion => "Doctor's companion",
            Self::Exploit => "Exploit",
            Self::Soulbond => "Soulbond",
            Self::Evolve => "Evolve",
            Self::Improvise => "Improvise",
            Self::Intimidate => "Intimidate",
            Self::Spree => "Spree",
            Self::Bargain => "Bargain",
            Self::Mentor => "Mentor",
            Self::Extort => "Extort",
            Self::LivingWeapon => "Living weapon",
            Self::Myriad => "Myriad",
            Self::Retrace => "Retrace",
            Self::Backup => "Backup",
            Self::UmbraArmor => "Umbra armor",
            Self::Cipher => "Cipher",
            Self::Renown => "Renown",
            Self::Ward => "Ward",
            Self::Scry => "Scry",
            Self::Surveil => "Surveil",
            Self::Cycling => "Cycling",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordSourceEvidence {
    pub face_index: u16,
    pub clause_index: u16,
    pub printed_keyword: String,
    pub oracle_fragment: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeywordProgramInput<'a> {
    pub face_index: u16,
    pub clause_index: u16,
    pub printed_keyword: &'a str,
    pub oracle_fragment: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationReplacement {
    NextDestructionThisTurn,
    EveryDestructionWhileStaticEffectApplies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationTargetFilter {
    BattlefieldCreature,
    BattlefieldPermanent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationRecipientCardinality {
    ExactlyOne,
    ZeroOrMore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationRecipientSelectionTime {
    WhenSpellOrAbilityIsPutOnStack,
    OnResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationRecipientScope {
    SourcePermanent,
    SingleTarget {
        filter: RegenerationTargetFilter,
        cardinality: RegenerationRecipientCardinality,
        selection_time: RegenerationRecipientSelectionTime,
    },
    EachCreatureControlledByEffectController {
        cardinality: RegenerationRecipientCardinality,
        selection_time: RegenerationRecipientSelectionTime,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationReminderReferent {
    SelectedTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationProtectionWindow {
    NextDestructionThisTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationReminderEvidence {
    Absent,
    CanonicalTargetCreature {
        referent: RegenerationReminderReferent,
        protection_window: RegenerationProtectionWindow,
        removes_all_damage: bool,
        controller_taps_recipient: bool,
        removes_from_combat_if_attacking_or_blocking_creature: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegenerationProgram {
    pub replacement: RegenerationReplacement,
    pub recipients: RegenerationRecipientScope,
    pub reminder: RegenerationReminderEvidence,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaSymbol {
    Generic(u32),
    Colored(ManaColor),
    Colorless,
    Snow,
    Hybrid(ManaColor, ManaColor),
    Phyrexian(ManaColor),
    VariableX,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaCost {
    pub raw: String,
    pub symbols: Vec<ManaSymbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KickerMultiplicity {
    OncePerCost,
    AnyNumberOfTimes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KickerProgram {
    pub costs: Vec<ManaCost>,
    pub multiplicity: KickerMultiplicity,
    pub effects_require_linked_kicker_ability: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlashbackProgram {
    pub alternative_cost: ManaCost,
    pub cast_from_graveyard: bool,
    pub exile_replaces_every_stack_destination: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MorphProgram {
    pub face_up_cost: ManaCost,
    pub face_down_cast_cost: ManaCost,
    pub face_down_power: i32,
    pub face_down_toughness: i32,
    pub face_down_has_name: bool,
    pub face_down_has_text: bool,
    pub face_down_has_subtypes: bool,
    pub face_down_has_mana_cost: bool,
    pub turn_face_up_is_special_action: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectionQualitySpec {
    Everything,
    Color(ManaColor),
    EachColor,
    ChosenColor,
    Colored,
    Colorless,
    Monocolored,
    Multicolored,
    CardType(String),
    Subtype(String),
    Named(String),
    ChosenPlayer,
    ManaValueAtMost(u32),
    ManaValueAtLeast(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectionProgram {
    pub qualities: Vec<ProtectionQualitySpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativePlayer {
    You,
    Opponent,
    Any,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectPredicate {
    Permanent,
    CardType(CardType),
    Subtype(String),
    Supertype(String),
    Color(ManaColor),
    Commander,
    Tapped,
    Controller(RelativePlayer),
    Zone(Zone),
    Not(Box<ObjectPredicate>),
    All(Vec<ObjectPredicate>),
    Any(Vec<ObjectPredicate>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentFilter {
    Object(ObjectPredicate),
    Player(RelativePlayer),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquipProgram {
    pub activation_cost: ManaCost,
    pub target_filter: ObjectPredicate,
    pub planeswalker_as_creature: bool,
    pub sorcery_timing_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnchantProgram {
    pub target_filter: AttachmentFilter,
    pub aura_spell_targets: bool,
    pub all_enchant_abilities_must_match: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaChapter {
    pub numbers: Vec<u32>,
    pub oracle_effect: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaProgram {
    pub chapters: Vec<SagaChapter>,
    pub final_chapter: u32,
    pub enters_with_one_lore_counter: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CumulativeUpkeepCost {
    ManaAlternatives(Vec<ManaCost>),
    PayLife(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CumulativeUpkeepProgram {
    pub cost_per_age_counter: CumulativeUpkeepCost,
    pub partial_payment_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexproofProgram {
    /// `None` is ordinary hexproof. A quality list is hexproof from those
    /// qualities and applies only to sources controlled by opponents.
    pub qualities: Option<Vec<ProtectionQualitySpec>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WardProgram {
    pub cost: ManaCost,
    pub variable_value_requires_resolution_state: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CyclingProgram {
    pub activation_cost: ManaCost,
    pub activation_zone: Zone,
    pub discard_self_is_cost: bool,
    pub draws: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangelingCharacteristic {
    EveryCreatureType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangelingFunctionScope {
    EverywhereIncludingOutsideTheGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangelingProgram {
    pub is_characteristic_defining_ability: bool,
    pub affected_characteristic: ChangelingCharacteristic,
    pub applies_to_the_object_with_changeling: bool,
    pub function_scope: ChangelingFunctionScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfectPlayerDamageResult {
    SourceControllerGivesPoisonCountersEqualToDamageInsteadOfLifeLoss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfectCreatureDamageResult {
    SourceControllerPutsMinusOneMinusOneCountersEqualToDamageInsteadOfMarkedDamage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InfectProgram {
    pub is_static_ability: bool,
    pub applies_to_combat_and_noncombat_damage: bool,
    pub player_damage_result: InfectPlayerDamageResult,
    pub creature_damage_result: InfectCreatureDamageResult,
    pub uses_damage_after_replacement_and_prevention: bool,
    pub uses_last_known_information_when_source_left_expected_zone: bool,
    pub functions_no_matter_which_zone_source_deals_damage_from: bool,
    pub instances_are_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FearProgram {
    pub artifact_or_black_blockers_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowProgram {
    pub requires_matching_shadow_status: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LandwalkQuality {
    Plains,
    Island,
    Swamp,
    Mountain,
    Forest,
    Desert,
    LegendaryLand,
    NonbasicLand,
    SnowLand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LandwalkProgram {
    pub quality: LandwalkQuality,
    pub checks_defending_player: bool,
    pub same_kind_instances_are_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellStackFunctionScope {
    WhileThisSpellIsOnTheStack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffinityCountedObjects {
    ArtifactPermanentsControlledBySpellController,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffinityProgram {
    pub is_static_ability: bool,
    pub function_scope: SpellStackFunctionScope,
    pub counted_objects: AffinityCountedObjects,
    pub generic_mana_reduction_per_counted_object: u32,
    pub count_uses_current_game_state_during_total_cost_determination: bool,
    pub cannot_reduce_colored_colorless_or_snow_requirements: bool,
    pub cannot_reduce_generic_requirement_below_zero: bool,
    pub multiple_instances_each_apply: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeTriggerTransition {
    ControllerCastsThisSpell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeExileProcedure {
    FromLibraryTopUntilFirstNonlandCardWithLesserManaValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeUncastCardDestination {
    LibraryBottomInRandomOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CascadeProgram {
    pub is_triggered_ability: bool,
    pub function_scope: SpellStackFunctionScope,
    pub trigger_transition: CascadeTriggerTransition,
    pub exile_procedure: CascadeExileProcedure,
    pub source_spell_mana_value_is_strict_upper_bound: bool,
    pub resulting_spell_mana_value_is_rechecked_after_cast_choices: bool,
    pub eligible_card_cast_is_optional: bool,
    pub eligible_card_casts_without_paying_mana_cost: bool,
    pub cast_occurs_during_resolution: bool,
    pub casting_restrictions_and_additional_costs_still_apply: bool,
    pub another_alternative_cost_cannot_be_used: bool,
    pub as_you_cascade_action_window_precedes_cast_choice: bool,
    pub uncast_card_destination: CascadeUncastCardDestination,
    pub instances_trigger_separately: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelvePaymentExchange {
    ExileOneCardFromSpellControllersGraveyardForOneGenericMana,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DelveProgram {
    pub is_static_ability: bool,
    pub function_scope: SpellStackFunctionScope,
    pub payment_exchange: DelvePaymentExchange,
    pub applies_after_total_cost_is_determined: bool,
    pub applies_only_to_generic_mana_in_total_cost: bool,
    pub is_not_an_additional_or_alternative_cost: bool,
    pub is_not_a_cost_reduction: bool,
    pub each_graveyard_card_can_pay_at_most_once: bool,
    pub multiple_instances_are_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuseFunctionScope {
    CardInItsControllersHand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuseCastChoice {
    OneHalfOrBothHalvesChosenBeforeCardIsPutOnStack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuseResolutionOrder {
    LeftHalfThenRightHalf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuseProgram {
    pub is_static_ability: bool,
    pub function_scope: FuseFunctionScope,
    pub requires_one_physical_split_card_with_exactly_two_halves: bool,
    pub requires_cast_origin_hand: bool,
    pub cast_choice: FuseCastChoice,
    pub fused_result_is_one_spell: bool,
    pub fused_spell_has_combined_characteristics_of_both_halves: bool,
    pub total_cost_includes_each_halfs_mana_cost: bool,
    pub resolution_order: FuseResolutionOrder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AftermathProgram {
    pub cast_this_half_from_graveyard: bool,
    pub this_half_cannot_be_cast_from_other_zones: bool,
    pub uses_selected_half_printed_mana_cost: bool,
    pub stack_identity_is_selected_half_only: bool,
    pub exile_replaces_every_stack_destination: bool,
    pub exile_replacement_requires_graveyard_cast_origin: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReboundReplacementEvent {
    CardSpellCastFromHandWouldEnterOwnersGraveyardAsItResolves,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReboundDelayedTrigger {
    BeginningOfSpellControllersNextUpkeep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReboundProgram {
    pub is_static_ability: bool,
    pub function_scope: SpellStackFunctionScope,
    pub replacement_event: ReboundReplacementEvent,
    pub replacement_exiles_the_same_card: bool,
    pub creates_delayed_trigger_only_when_replacement_exiles_card: bool,
    pub delayed_trigger: ReboundDelayedTrigger,
    pub delayed_cast_from_exile_is_optional: bool,
    pub delayed_cast_without_paying_mana_cost: bool,
    pub casting_restrictions_and_additional_costs_still_apply: bool,
    pub another_alternative_cost_cannot_be_used: bool,
    pub no_effect_for_spell_copy_without_card_or_non_graveyard_destination: bool,
    pub instances_are_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExaltedTriggerTransition {
    CreatureControlledByAbilityControllerIsDeclaredAsOnlyAttackerInCombat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExaltedEffectDuration {
    UntilEndOfTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExaltedProgram {
    pub is_triggered_ability: bool,
    pub trigger_transition: ExaltedTriggerTransition,
    pub attacks_alone_counts_only_declared_attackers: bool,
    pub trigger_uses_stack: bool,
    pub affected_creature_is_the_declared_attacker_that_caused_trigger: bool,
    pub uses_targeting: bool,
    pub power_delta: i32,
    pub toughness_delta: i32,
    pub duration: ExaltedEffectDuration,
    pub source_need_not_remain_on_battlefield_for_resolution: bool,
    pub later_creatures_entering_attacking_do_not_undo_trigger: bool,
    pub instances_trigger_separately: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BushidoBlockTransition {
    SourceStartsBlocking {
        excludes_entering_battlefield_blocking: bool,
    },
    SourceAttackerBecomesBlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BushidoTriggerMultiplicity {
    OncePerAbilityOccurrencePerQualifyingTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BushidoResolutionTarget {
    SameBattlefieldCreatureIncarnation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BushidoEffectDuration {
    UntilEndOfTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BushidoProgram {
    pub amount: u32,
    pub trigger_transitions: [BushidoBlockTransition; 2],
    pub trigger_multiplicity: BushidoTriggerMultiplicity,
    pub instances_trigger_separately: bool,
    pub queued_trigger_survives_source_changes: bool,
    pub resolution_target: BushidoResolutionTarget,
    pub power_delta: i32,
    pub toughness_delta: i32,
    pub duration: BushidoEffectDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitherCreatureDamageApplication {
    MinusOneMinusOneCountersEqualToDamage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WitherProgram {
    pub creature_damage: WitherCreatureDamageApplication,
    pub source_controller_places_counters: bool,
    pub uses_last_known_information: bool,
    pub functions_in_all_zones: bool,
    pub instances_are_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorsemanshipBlockRestriction {
    BlockerMustHaveHorsemanship,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HorsemanshipProgram {
    pub block_restriction: HorsemanshipBlockRestriction,
    pub creature_with_horsemanship_may_block_either_kind: bool,
    pub instances_are_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlankingTriggerTransition {
    SourceBecomesBlockedByCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlankingBlockerPredicate {
    BlockingCreatureWithoutFlanking,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlankingTriggerMultiplicity {
    OncePerAbilityOccurrencePerQualifyingBlockingCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlankingEffectRecipient {
    BlockingCreatureIncarnationThatCausedTrigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlankingEffectDuration {
    UntilEndOfTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlankingProgram {
    pub trigger_transition: FlankingTriggerTransition,
    pub blocker_predicate: FlankingBlockerPredicate,
    pub trigger_multiplicity: FlankingTriggerMultiplicity,
    pub instances_trigger_separately: bool,
    pub resolution_recipient: FlankingEffectRecipient,
    pub uses_targeting_system: bool,
    pub power_delta: i32,
    pub toughness_delta: i32,
    pub duration: FlankingEffectDuration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathReturnTriggerTransition {
    BattlefieldPermanentPutIntoGraveyard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathReturnCounterKind {
    MinusOneMinusOne,
    PlusOnePlusOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathReturnCounterCondition {
    NoCounterOfKindImmediatelyBeforeDeathUsingLastKnownInformation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathReturnTriggerMultiplicity {
    OncePerAbilityOccurrencePerQualifyingDeath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathReturnCardIdentity {
    NewPublicGraveyardObjectFromTriggeringZoneChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathReturnResolutionRequirement {
    LinkedCardRemainsInFirstGraveyard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathReturnBattlefieldController {
    Owner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathReturnTokenInteraction {
    TriggerMayExistButTokenCeasesBeforeResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathReturnReplacementInteraction {
    ReplacedGraveyardMoveDoesNotTrigger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeathReturnProgram {
    pub is_triggered_ability: bool,
    pub trigger_transition: DeathReturnTriggerTransition,
    pub prohibited_counter: DeathReturnCounterKind,
    pub counter_condition: DeathReturnCounterCondition,
    pub trigger_multiplicity: DeathReturnTriggerMultiplicity,
    pub trigger_uses_stack_at_next_priority: bool,
    pub instances_trigger_separately: bool,
    pub linked_card: DeathReturnCardIdentity,
    pub resolution_requirement: DeathReturnResolutionRequirement,
    pub return_under: DeathReturnBattlefieldController,
    pub return_counter: DeathReturnCounterKind,
    pub return_creates_new_battlefield_object: bool,
    pub token_interaction: DeathReturnTokenInteraction,
    pub replacement_interaction: DeathReturnReplacementInteraction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToxicDamageEvent {
    CombatDamageDealtToPlayerByCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToxicValueCombination {
    SumAllToxicAbilityValues,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToxicPoisonApplication {
    SourceControllerGivesDamagedPlayerCountersInDamageTransaction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToxicProgram {
    pub amount: u32,
    pub is_static_ability: bool,
    pub damage_event: ToxicDamageEvent,
    pub actual_damage_required: bool,
    pub value_combination: ToxicValueCombination,
    pub poison_application: ToxicPoisonApplication,
    pub poison_counters_equal_total_toxic_value: bool,
    pub poison_is_in_addition_to_other_damage_results: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightFaceRole {
    DayboundFrontFace,
    NightboundBackFace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightGlobalLifecycle {
    SinglePersistentMutuallyExclusiveGameDesignationInitiallyNeither,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightInitialDesignation {
    DayWhenDayboundPermanentIsControlledWhileNeither,
    NightWhenNightboundPermanentIsControlledWhileNeitherAndNoDayboundPermanentExists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightSharedTeamSpellCountRule {
    DayToNightIfTeamCastNoneAndNightToDayIfAnyOneTeamPlayerCastAtLeastTwo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayNightSpellCountTransition {
    pub check_during_second_part_of_untap_step: bool,
    pub inspect_previous_active_player_turn: bool,
    pub day_to_night_when_zero_spells: bool,
    pub night_to_day_minimum_spells: u32,
    pub neither_designation_skips_check: bool,
    pub shared_team_turn_rule: DayNightSharedTeamSpellCountRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightEntryBehavior {
    EnterTransformedAtNightWhenRepresentedByDoubleFacedCard,
    NoEntryModification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightInvalidEntryDestination {
    InstantOrSorceryBackFaceKeepsNonstackCardInPriorZoneOrPutsResolvingSpellIntoOwnersGraveyard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightImmediateAlignment {
    TransformFrontFaceUpPermanentAtNight,
    TransformBackFaceUpPermanentAtDay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightDesignationTransform {
    FrontToBackAsItBecomesNight,
    BackToFrontAsItBecomesDay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightZoneScope {
    EntryModificationWhileEnteringAndOtherAbilitiesOnBattlefield,
    BattlefieldOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayNightTransformBatch {
    AllEligibleBattlefieldPermanentsSimultaneously,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayNightProgram {
    pub is_static_ability: bool,
    pub face_role: DayNightFaceRole,
    pub global_lifecycle: DayNightGlobalLifecycle,
    pub initial_designation: DayNightInitialDesignation,
    pub spell_count_transition: DayNightSpellCountTransition,
    pub entry_behavior: DayNightEntryBehavior,
    pub invalid_entry_destination: Option<DayNightInvalidEntryDestination>,
    pub immediate_alignment: DayNightImmediateAlignment,
    pub designation_transform: DayNightDesignationTransform,
    pub zone_scope: DayNightZoneScope,
    pub transform_batch: DayNightTransformBatch,
    pub transform_requires_double_faced_card_or_token: bool,
    pub transform_instruction_rejects_instant_or_sorcery_destination: bool,
    pub transform_preserves_object_identity: bool,
    pub other_transform_causes_are_prohibited: bool,
    pub instances_are_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedSourceScope {
    ControlledPermanentOnBattlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedInitialization {
    NoSpeedToOneAsStateBasedAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedIncreaseEvent {
    OneOrMoreOpponentsLoseLifeDuringControllersTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedIncreaseLimit {
    OncePerControllerTurn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedPersistence {
    PlayerRetainsDesignationAfterSourceLeaves,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartYourEnginesProgram {
    pub is_static_ability: bool,
    pub source_scope: SpeedSourceScope,
    pub initialization: SpeedInitialization,
    pub initial_speed: u32,
    pub speed_is_absent_until_set: bool,
    pub inherent_trigger_has_no_source: bool,
    pub inherent_trigger_is_controlled_by_player: bool,
    pub inherent_trigger_uses_stack_at_next_priority: bool,
    pub increase_event: SpeedIncreaseEvent,
    pub increase_limit: SpeedIncreaseLimit,
    pub increase_requires_current_speed_below_maximum: bool,
    pub increase_amount: u32,
    pub increase_instruction_from_no_speed_sets_to_requested_value: bool,
    pub maximum_speed: u32,
    pub persistence: SpeedPersistence,
    pub no_speed_reads_as_zero_for_effects: bool,
    pub instances_are_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommanderPartnerVariant {
    ChooseABackground,
    DoctorsCompanion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommanderPartnerSourceRequirement {
    DistinctLegendaryCardWithThisAbility,
    DistinctLegendaryCreatureCardWithThisAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommanderPartnerCounterpartRequirement {
    LegendaryBackgroundEnchantmentCard,
    LegendaryTimeLordDoctorCreatureCardWithNoOtherCreatureTypes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommanderPartnerTracking {
    SeparateCastCountsTaxAndCombatDamagePerCommanderPerDamagedPlayer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommanderPartnerReference {
    EitherCommanderAndAffectedPlayerChoosesOneWhenBothCouldBeAffected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommanderPartnerProgram {
    pub variant: CommanderPartnerVariant,
    pub functions_only_before_game_for_deck_construction: bool,
    pub source_requirement: CommanderPartnerSourceRequirement,
    pub counterpart_requirement: CommanderPartnerCounterpartRequirement,
    pub counterpart_needs_same_partner_ability: bool,
    pub commander_count_when_used: u32,
    pub maximum_commanders_from_partner_abilities: u32,
    pub deck_card_count_including_commanders: u32,
    pub both_commanders_start_in_command_zone: bool,
    pub commander_designation_persists_across_zones: bool,
    pub combined_color_identity_for_deck_construction_and_references: bool,
    pub independent_tracking: CommanderPartnerTracking,
    pub commander_reference: CommanderPartnerReference,
    pub different_partner_variants_cannot_combine: bool,
    pub choose_only_one_when_source_has_multiple_partner_abilities: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploitTriggerTransition {
    SourceCreatureEntersBattlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploitSacrificeChoice {
    OptionalOneCreatureControlledByAbilityControllerOnResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExploitEventDefinition {
    SourceExploitsChosenCreatureWhenControllerSacrificesItDuringThisResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExploitProgram {
    pub trigger_transition: ExploitTriggerTransition,
    pub trigger_uses_stack: bool,
    pub trigger_controller_is_source_controller_at_trigger_time: bool,
    pub sacrifice_choice: ExploitSacrificeChoice,
    pub sacrifice_uses_targeting: bool,
    pub source_may_be_chosen_for_sacrifice: bool,
    pub source_need_not_remain_on_battlefield_for_resolution: bool,
    pub sacrifice_moves_controlled_permanent_from_battlefield_to_owners_graveyard: bool,
    pub sacrifice_destination_is_subject_to_zone_change_replacement: bool,
    pub sacrifice_is_not_destruction: bool,
    pub exploit_event: ExploitEventDefinition,
    pub exploit_event_requires_completed_sacrifice_action: bool,
    pub instances_trigger_separately: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulbondTriggerSet {
    SourceEntersOrAnotherCreatureControlledBySourceControllerEnters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulbondEligibility {
    BothObjectsAreUnpairedCreaturesOnBattlefieldControlledByAbilityControllerAtTriggerAndResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulbondPairChoice {
    OptionalNontargetedChoiceOnResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulbondPairLifecycle {
    SymmetricExclusivePairWhileBothRemainCreaturesOnBattlefieldUnderSameController,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulbondUnpairTransition {
    EitherLeavesBattlefieldStopsBeingCreatureOrChangesController,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoulbondProgram {
    pub represents_two_triggered_abilities: bool,
    pub trigger_set: SoulbondTriggerSet,
    pub trigger_uses_stack: bool,
    pub trigger_controller_is_source_controller_at_trigger_time: bool,
    pub eligibility: SoulbondEligibility,
    pub source_entry_chooses_another_eligible_creature: bool,
    pub other_entry_is_bound_to_that_entering_creature: bool,
    pub simultaneous_other_entries_each_create_their_own_trigger: bool,
    pub pair_choice: SoulbondPairChoice,
    pub pair_lifecycle: SoulbondPairLifecycle,
    pub maximum_partners_per_creature: u32,
    pub unpair_transition: SoulbondUnpairTransition,
    pub teammate_or_opponent_creatures_are_ineligible: bool,
    pub instances_trigger_separately: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolveTriggerTransition {
    CreatureControlledBySourceControllerEntersBattlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolveComparison {
    EnteringPowerGreaterOrEnteringToughnessGreaterThanSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolveInformationRule {
    CurrentInformationOrLastKnownInformationForDepartedEnteringCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolveEventDefinition {
    OneOrMorePlusOnePlusOneCountersPlacedByResolvingEvolveAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvolveProgram {
    pub trigger_transition: EvolveTriggerTransition,
    pub trigger_uses_stack: bool,
    pub trigger_controller_is_source_controller_at_trigger_time: bool,
    pub uses_intervening_if_at_trigger_and_resolution: bool,
    pub comparison: EvolveComparison,
    pub compares_effective_power_and_toughness: bool,
    pub information_rule: EvolveInformationRule,
    pub comparison_is_false_against_noncreature_permanent: bool,
    pub counter_recipient_is_source_incarnation_on_battlefield: bool,
    pub plus_one_plus_one_counters_per_resolution: u32,
    pub evolve_event: EvolveEventDefinition,
    pub uses_targeting: bool,
    pub simultaneous_entries_each_create_their_own_trigger: bool,
    pub instances_trigger_separately: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImproviseFunctionZone {
    SpellStackOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImprovisePaymentTiming {
    AfterTotalCostLockedAndManaAbilitiesActivatedDuringCostPayment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImprovisePaymentExchange {
    TapOneUntappedControlledArtifactForOneGenericMana,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImproviseProgram {
    pub is_static_ability: bool,
    pub function_zone: ImproviseFunctionZone,
    pub payment_timing: ImprovisePaymentTiming,
    pub payment_exchange: ImprovisePaymentExchange,
    pub applies_only_to_generic_mana_in_locked_total_cost: bool,
    pub is_not_additional_or_alternative_cost: bool,
    pub is_not_cost_reduction: bool,
    pub payment_is_optional_for_each_generic_mana: bool,
    pub tapped_or_uncontrolled_artifacts_are_ineligible: bool,
    pub summoning_sickness_does_not_prevent_artifact_payment: bool,
    pub one_artifact_cannot_pay_more_than_once: bool,
    pub instances_are_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntimidateBlockerQualification {
    ArtifactCreatureOrCreatureSharingAtLeastOneCurrentColorWithAttacker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntimidateProgram {
    pub is_static_evasion_ability: bool,
    pub blocker_qualification: IntimidateBlockerQualification,
    pub every_declared_blocker_must_individually_qualify: bool,
    pub colorless_attacker_requires_artifact_blocker: bool,
    pub checks_current_characteristics_during_block_declaration: bool,
    pub gain_or_loss_after_legal_declaration_does_not_change_block: bool,
    pub later_attacker_or_blocker_characteristic_changes_do_not_change_block: bool,
    pub composes_with_other_block_restrictions: bool,
    pub instances_are_redundant: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreeFunctionZone {
    ModalSpellOnStack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreeModeChoice {
    ControllerChoosesOneOrMoreLegalModesWhileCasting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpreeModeCostBinding {
    EveryChosenModeRequiresItsAssociatedPrintedAdditionalCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpreeProgram {
    pub is_static_ability: bool,
    pub function_zone: SpreeFunctionZone,
    pub mode_choice: SpreeModeChoice,
    pub choose_modes_before_targets: bool,
    pub chosen_mode_must_have_legal_required_targets: bool,
    pub same_mode_normally_cannot_be_chosen_more_than_once: bool,
    pub retargeting_does_not_change_modes: bool,
    pub spell_copy_retains_chosen_modes_without_new_choice: bool,
    pub chosen_modes_resolve_in_printed_order: bool,
    pub mode_cost_binding: SpreeModeCostBinding,
    pub all_chosen_mode_costs_are_additional_costs: bool,
    pub all_chosen_mode_costs_must_be_paid_without_partial_payment: bool,
    pub mode_costs_do_not_change_mana_cost: bool,
    pub requires_exact_associated_mode_table_from_source: bool,
    pub plus_sign_icons_have_no_rules_meaning: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BargainSacrificeChoice {
    OneControlledArtifactEnchantmentOrTokenPermanent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BargainProgram {
    pub is_static_ability_on_spell_stack: bool,
    pub is_optional_additional_cost: bool,
    pub sacrifice_choice: BargainSacrificeChoice,
    pub sacrifice_is_declared_before_targets: bool,
    pub sacrifice_is_paid_with_total_cost: bool,
    pub bargain_does_not_change_mana_cost: bool,
    pub bargained_status_is_set_when_intention_is_declared: bool,
    pub casting_must_later_pay_declared_cost_to_complete: bool,
    pub linked_effects_reference_only_this_printed_bargain_ability: bool,
    pub conditional_targets_are_chosen_only_when_bargained: bool,
    pub cost_can_be_paid_at_most_once: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentorTriggerTransition {
    SourceCreatureDeclaredAsAttacker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentorTargetRestriction {
    AttackingCreatureWithCurrentPowerLessThanSourceCurrentPower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MentorProgram {
    pub trigger_transition: MentorTriggerTransition,
    pub trigger_uses_stack: bool,
    pub target_restriction: MentorTargetRestriction,
    pub restriction_checked_on_target_selection_and_resolution: bool,
    pub source_and_target_use_current_power: bool,
    pub plus_one_plus_one_counters: u32,
    pub counter_is_placed_on_legal_target_on_resolution: bool,
    pub mentor_event_occurs_when_ability_resolves: bool,
    pub instances_trigger_separately: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtortTriggerTransition {
    ControllerCastsSpell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtortProgram {
    pub trigger_transition: ExtortTriggerTransition,
    pub trigger_uses_stack: bool,
    pub optional_hybrid_white_black_payment_on_resolution: bool,
    pub payment_may_be_made_at_most_once_per_trigger: bool,
    pub each_opponent_loses_life_simultaneously: u32,
    pub controller_gains_life_equal_to_total_life_actually_lost: bool,
    pub uses_targeting: bool,
    pub instances_trigger_separately: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivingWeaponTokenDefinition {
    ZeroZeroBlackPhyrexianGermCreature,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivingWeaponProgram {
    pub is_enters_battlefield_trigger: bool,
    pub trigger_uses_stack: bool,
    pub token: LivingWeaponTokenDefinition,
    pub token_count: u32,
    pub token_creation_precedes_attachment: bool,
    pub attach_source_equipment_to_created_token: bool,
    pub attachment_does_not_target: bool,
    pub failed_or_illegal_attachment_leaves_equipment_unattached: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MyriadTriggerTransition {
    SourceCreatureDeclaredAsAttacker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MyriadProgram {
    pub trigger_transition: MyriadTriggerTransition,
    pub trigger_uses_stack: bool,
    pub one_optional_copy_for_each_opponent_other_than_defending_player: bool,
    pub copy_is_token_with_source_copiable_values: bool,
    pub token_enters_tapped_and_attacking: bool,
    pub token_controller_chooses_that_opponent_or_their_planeswalker: bool,
    pub entering_attacking_does_not_trigger_declared_attacker_abilities: bool,
    pub creates_delayed_end_of_combat_exile_trigger_when_any_token_was_created: bool,
    pub delayed_trigger_exiles_only_tokens_created_by_this_resolution: bool,
    pub instances_trigger_separately: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetraceFunctionZone {
    OwnersGraveyard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetraceProgram {
    pub is_static_ability: bool,
    pub function_zone: RetraceFunctionZone,
    pub permits_casting_card_from_graveyard: bool,
    pub discard_one_land_card_is_additional_cost: bool,
    pub printed_and_other_costs_are_still_paid: bool,
    pub normal_casting_timing_and_restrictions_still_apply: bool,
    pub does_not_change_mana_cost: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackupGrantedAbilitySet {
    NonBackupAbilitiesPrintedBelowThisBackupAbility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupProgram {
    pub counter_count: u32,
    pub is_enters_battlefield_trigger: bool,
    pub trigger_uses_stack: bool,
    pub targets_one_creature: bool,
    pub places_plus_one_plus_one_counters_on_legal_target: bool,
    pub grants_abilities_only_if_target_is_another_creature: bool,
    pub granted_abilities: BackupGrantedAbilitySet,
    pub granted_abilities_last_until_end_of_turn: bool,
    pub printed_ability_order_is_copiable_and_preserved: bool,
    pub gained_or_created_abilities_are_not_granted: bool,
    pub granted_ability_set_is_fixed_when_trigger_enters_stack: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UmbraArmorProgram {
    pub is_static_replacement_effect: bool,
    pub replaces_destruction_of_enchanted_permanent: bool,
    pub replacement_is_mandatory: bool,
    pub removes_all_damage_marked_on_enchanted_permanent: bool,
    pub destroys_source_aura: bool,
    pub source_aura_is_destroyed_by_replacement_instruction: bool,
    pub does_not_regenerate_enchanted_permanent: bool,
    pub multiple_applicable_replacements_follow_replacement_choice_rules: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CipherEncodeChoice {
    OptionalNontargetedCreatureControlledBySpellController,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CipherProgram {
    pub spell_ability_functions_on_stack: bool,
    pub requires_spell_represented_by_card: bool,
    pub encode_choice_on_resolution: CipherEncodeChoice,
    pub exiles_spell_card_encoded_on_chosen_creature: bool,
    pub static_ability_functions_while_card_is_exiled: bool,
    pub relationship_requires_card_in_exile_and_same_creature_object_on_battlefield: bool,
    pub relationship_survives_creature_control_change_or_loss_of_creature_type: bool,
    pub combat_damage_to_player_triggers_for_current_creature_controller: bool,
    pub trigger_copies_encoded_card: bool,
    pub copied_card_may_be_cast_without_paying_mana_cost: bool,
    pub casting_copy_is_optional_and_obeys_other_casting_restrictions: bool,
    pub casting_copy_still_requires_additional_costs_and_cannot_use_another_alternative_cost: bool,
    pub spell_copy_without_a_card_cannot_be_encoded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenownProgram {
    pub counter_count: u32,
    pub triggers_on_combat_damage_to_player: bool,
    pub uses_intervening_if_not_renowned: bool,
    pub trigger_uses_stack: bool,
    pub puts_plus_one_plus_one_counters_on_source: bool,
    pub source_becomes_renowned_after_counter_instruction: bool,
    pub renowned_is_persistent_battlefield_designation: bool,
    pub renowned_is_not_an_ability_or_copiable_value: bool,
    pub designation_ends_when_permanent_leaves_battlefield: bool,
    pub instances_trigger_separately_but_later_resolutions_do_nothing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AscendSpellApplication {
    SpellAbilityChecksDuringInstantOrSorceryResolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AscendPermanentApplication {
    StaticAbilityContinuouslyChecksWhilePermanentIsOnBattlefield,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AscendProgram {
    pub spell_application: AscendSpellApplication,
    pub permanent_application: AscendPermanentApplication,
    pub permanent_threshold: u32,
    pub requires_player_not_already_have_citys_blessing: bool,
    pub citys_blessing_persists_for_rest_of_game: bool,
    pub citys_blessing_has_no_inherent_rules_effect: bool,
    pub any_number_of_players_may_have_citys_blessing: bool,
    pub continuous_effects_reapply_before_trigger_condition_checks: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvokePaymentTiming {
    AfterTotalCostIsDeterminedAndManaAbilitiesAreActivatedDuringPayment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvokePaymentExchange {
    TapOneUntappedControlledCreatureForOneGenericOrOneMatchingColoredMana,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvokeProgram {
    pub is_static_ability: bool,
    pub function_scope: SpellStackFunctionScope,
    pub payment_timing: ConvokePaymentTiming,
    pub payment_exchange: ConvokePaymentExchange,
    pub cannot_pay_colorless_or_snow_requirements: bool,
    pub is_not_an_additional_or_alternative_cost: bool,
    pub is_not_a_cost_reduction: bool,
    pub summoning_sickness_does_not_prevent_payment: bool,
    pub each_creature_can_pay_at_most_once: bool,
    pub tapped_creature_is_designated_as_having_convoked_spell: bool,
    pub multiple_instances_are_redundant: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordProgramKind {
    Mill,
    Regenerate(RegenerationProgram),
    Protection(ProtectionProgram),
    Flying,
    Fight,
    Investigate,
    Kicker(KickerProgram),
    Flashback(FlashbackProgram),
    Morph(MorphProgram),
    Flash,
    Menace,
    Defender,
    Reach,
    Changeling(ChangelingProgram),
    Infect(InfectProgram),
    Fear(FearProgram),
    Shadow(ShadowProgram),
    Landwalk(LandwalkProgram),
    Affinity(AffinityProgram),
    Cascade(CascadeProgram),
    Delve(DelveProgram),
    Fuse(FuseProgram),
    Aftermath(AftermathProgram),
    Rebound(ReboundProgram),
    Exalted(ExaltedProgram),
    Bushido(BushidoProgram),
    Wither(WitherProgram),
    Horsemanship(HorsemanshipProgram),
    Flanking(FlankingProgram),
    Persist(DeathReturnProgram),
    Undying(DeathReturnProgram),
    Toxic(ToxicProgram),
    Daybound(DayNightProgram),
    Nightbound(DayNightProgram),
    StartYourEngines(StartYourEnginesProgram),
    ChooseABackground(CommanderPartnerProgram),
    DoctorsCompanion(CommanderPartnerProgram),
    Exploit(ExploitProgram),
    Soulbond(SoulbondProgram),
    Evolve(EvolveProgram),
    Improvise(ImproviseProgram),
    Intimidate(IntimidateProgram),
    Spree(SpreeProgram),
    Bargain(BargainProgram),
    Mentor(MentorProgram),
    Extort(ExtortProgram),
    LivingWeapon(LivingWeaponProgram),
    Myriad(MyriadProgram),
    Retrace(RetraceProgram),
    Backup(BackupProgram),
    UmbraArmor(UmbraArmorProgram),
    Cipher(CipherProgram),
    Renown(RenownProgram),
    Ascend(AscendProgram),
    Devoid,
    Convoke(ConvokeProgram),
    Equip(EquipProgram),
    Enchant(EnchantProgram),
    Saga(SagaProgram),
    CumulativeUpkeep(CumulativeUpkeepProgram),
    Haste,
    Vigilance,
    Trample,
    Deathtouch,
    Lifelink,
    FirstStrike,
    DoubleStrike,
    Hexproof(HexproofProgram),
    Shroud,
    Indestructible,
    Prowess,
    Ward(WardProgram),
    Scry,
    Surveil,
    Cycling(CyclingProgram),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordProgram {
    runtime_version: &'static str,
    source: KeywordSourceEvidence,
    kind: KeywordProgramKind,
}

impl KeywordProgram {
    pub fn runtime_version(&self) -> &'static str {
        self.runtime_version
    }

    pub fn source(&self) -> &KeywordSourceEvidence {
        &self.source
    }

    pub fn kind(&self) -> &KeywordProgramKind {
        &self.kind
    }

    pub fn keyword(&self) -> OfficialKeyword {
        match self.kind {
            KeywordProgramKind::Mill => OfficialKeyword::Mill,
            KeywordProgramKind::Regenerate(_) => OfficialKeyword::Regenerate,
            KeywordProgramKind::Protection(_) => OfficialKeyword::Protection,
            KeywordProgramKind::Flying => OfficialKeyword::Flying,
            KeywordProgramKind::Fight => OfficialKeyword::Fight,
            KeywordProgramKind::Investigate => OfficialKeyword::Investigate,
            KeywordProgramKind::Kicker(_) => OfficialKeyword::Kicker,
            KeywordProgramKind::Flashback(_) => OfficialKeyword::Flashback,
            KeywordProgramKind::Morph(_) => OfficialKeyword::Morph,
            KeywordProgramKind::Flash => OfficialKeyword::Flash,
            KeywordProgramKind::Menace => OfficialKeyword::Menace,
            KeywordProgramKind::Defender => OfficialKeyword::Defender,
            KeywordProgramKind::Reach => OfficialKeyword::Reach,
            KeywordProgramKind::Changeling(_) => OfficialKeyword::Changeling,
            KeywordProgramKind::Infect(_) => OfficialKeyword::Infect,
            KeywordProgramKind::Fear(_) => OfficialKeyword::Fear,
            KeywordProgramKind::Shadow(_) => OfficialKeyword::Shadow,
            KeywordProgramKind::Landwalk(_) => OfficialKeyword::Landwalk,
            KeywordProgramKind::Affinity(_) => OfficialKeyword::Affinity,
            KeywordProgramKind::Cascade(_) => OfficialKeyword::Cascade,
            KeywordProgramKind::Delve(_) => OfficialKeyword::Delve,
            KeywordProgramKind::Fuse(_) => OfficialKeyword::Fuse,
            KeywordProgramKind::Aftermath(_) => OfficialKeyword::Aftermath,
            KeywordProgramKind::Rebound(_) => OfficialKeyword::Rebound,
            KeywordProgramKind::Exalted(_) => OfficialKeyword::Exalted,
            KeywordProgramKind::Bushido(_) => OfficialKeyword::Bushido,
            KeywordProgramKind::Wither(_) => OfficialKeyword::Wither,
            KeywordProgramKind::Horsemanship(_) => OfficialKeyword::Horsemanship,
            KeywordProgramKind::Flanking(_) => OfficialKeyword::Flanking,
            KeywordProgramKind::Persist(_) => OfficialKeyword::Persist,
            KeywordProgramKind::Undying(_) => OfficialKeyword::Undying,
            KeywordProgramKind::Toxic(_) => OfficialKeyword::Toxic,
            KeywordProgramKind::Daybound(_) => OfficialKeyword::Daybound,
            KeywordProgramKind::Nightbound(_) => OfficialKeyword::Nightbound,
            KeywordProgramKind::StartYourEngines(_) => OfficialKeyword::StartYourEngines,
            KeywordProgramKind::ChooseABackground(_) => OfficialKeyword::ChooseABackground,
            KeywordProgramKind::DoctorsCompanion(_) => OfficialKeyword::DoctorsCompanion,
            KeywordProgramKind::Exploit(_) => OfficialKeyword::Exploit,
            KeywordProgramKind::Soulbond(_) => OfficialKeyword::Soulbond,
            KeywordProgramKind::Evolve(_) => OfficialKeyword::Evolve,
            KeywordProgramKind::Improvise(_) => OfficialKeyword::Improvise,
            KeywordProgramKind::Intimidate(_) => OfficialKeyword::Intimidate,
            KeywordProgramKind::Spree(_) => OfficialKeyword::Spree,
            KeywordProgramKind::Bargain(_) => OfficialKeyword::Bargain,
            KeywordProgramKind::Mentor(_) => OfficialKeyword::Mentor,
            KeywordProgramKind::Extort(_) => OfficialKeyword::Extort,
            KeywordProgramKind::LivingWeapon(_) => OfficialKeyword::LivingWeapon,
            KeywordProgramKind::Myriad(_) => OfficialKeyword::Myriad,
            KeywordProgramKind::Retrace(_) => OfficialKeyword::Retrace,
            KeywordProgramKind::Backup(_) => OfficialKeyword::Backup,
            KeywordProgramKind::UmbraArmor(_) => OfficialKeyword::UmbraArmor,
            KeywordProgramKind::Cipher(_) => OfficialKeyword::Cipher,
            KeywordProgramKind::Renown(_) => OfficialKeyword::Renown,
            KeywordProgramKind::Ascend(_) => OfficialKeyword::Ascend,
            KeywordProgramKind::Devoid => OfficialKeyword::Devoid,
            KeywordProgramKind::Convoke(_) => OfficialKeyword::Convoke,
            KeywordProgramKind::Equip(_) => OfficialKeyword::Equip,
            KeywordProgramKind::Enchant(_) => OfficialKeyword::Enchant,
            KeywordProgramKind::Saga(_) => OfficialKeyword::Saga,
            KeywordProgramKind::CumulativeUpkeep(_) => OfficialKeyword::CumulativeUpkeep,
            KeywordProgramKind::Haste => OfficialKeyword::Haste,
            KeywordProgramKind::Vigilance => OfficialKeyword::Vigilance,
            KeywordProgramKind::Trample => OfficialKeyword::Trample,
            KeywordProgramKind::Deathtouch => OfficialKeyword::Deathtouch,
            KeywordProgramKind::Lifelink => OfficialKeyword::Lifelink,
            KeywordProgramKind::FirstStrike => OfficialKeyword::FirstStrike,
            KeywordProgramKind::DoubleStrike => OfficialKeyword::DoubleStrike,
            KeywordProgramKind::Hexproof(_) => OfficialKeyword::Hexproof,
            KeywordProgramKind::Shroud => OfficialKeyword::Shroud,
            KeywordProgramKind::Indestructible => OfficialKeyword::Indestructible,
            KeywordProgramKind::Prowess => OfficialKeyword::Prowess,
            KeywordProgramKind::Ward(_) => OfficialKeyword::Ward,
            KeywordProgramKind::Scry => OfficialKeyword::Scry,
            KeywordProgramKind::Surveil => OfficialKeyword::Surveil,
            KeywordProgramKind::Cycling(_) => OfficialKeyword::Cycling,
        }
    }

    pub fn official_rules(&self) -> &'static [OfficialRule] {
        match self.kind {
            KeywordProgramKind::Mill => MILL_RULES,
            KeywordProgramKind::Regenerate(program) => {
                match (program.replacement, program.recipients) {
                    (
                        RegenerationReplacement::NextDestructionThisTurn,
                        RegenerationRecipientScope::SourcePermanent,
                    ) => REGENERATE_SOURCE_RULES,
                    (
                        RegenerationReplacement::NextDestructionThisTurn,
                        RegenerationRecipientScope::SingleTarget { .. },
                    ) => REGENERATE_TARGET_RULES,
                    (
                        RegenerationReplacement::NextDestructionThisTurn,
                        RegenerationRecipientScope::EachCreatureControlledByEffectController {
                            ..
                        },
                    ) => REGENERATE_CONTROLLED_CREATURE_SET_RULES,
                    (
                        RegenerationReplacement::EveryDestructionWhileStaticEffectApplies,
                        RegenerationRecipientScope::SourcePermanent,
                    ) => REGENERATE_STATIC_RULES,
                    (
                        RegenerationReplacement::EveryDestructionWhileStaticEffectApplies,
                        RegenerationRecipientScope::SingleTarget { .. }
                        | RegenerationRecipientScope::EachCreatureControlledByEffectController {
                            ..
                        },
                    ) => REGENERATE_STATIC_RULES,
                }
            }
            KeywordProgramKind::Protection(_) => PROTECTION_RULES,
            KeywordProgramKind::Flying => FLYING_RULES,
            KeywordProgramKind::Fight => FIGHT_RULES,
            KeywordProgramKind::Investigate => INVESTIGATE_RULES,
            KeywordProgramKind::Kicker(_) => KICKER_RULES,
            KeywordProgramKind::Flashback(_) => FLASHBACK_RULES,
            KeywordProgramKind::Morph(_) => MORPH_RULES,
            KeywordProgramKind::Flash => FLASH_RULES,
            KeywordProgramKind::Menace => MENACE_RULES,
            KeywordProgramKind::Defender => DEFENDER_RULES,
            KeywordProgramKind::Reach => REACH_RULES,
            KeywordProgramKind::Changeling(_) => CHANGELING_RULES,
            KeywordProgramKind::Infect(_) => INFECT_RULES,
            KeywordProgramKind::Fear(_) => FEAR_RULES,
            KeywordProgramKind::Shadow(_) => SHADOW_RULES,
            KeywordProgramKind::Landwalk(_) => LANDWALK_RULES,
            KeywordProgramKind::Affinity(_) => AFFINITY_RULES,
            KeywordProgramKind::Cascade(_) => CASCADE_RULES,
            KeywordProgramKind::Delve(_) => DELVE_RULES,
            KeywordProgramKind::Fuse(_) => FUSE_RULES,
            KeywordProgramKind::Aftermath(_) => AFTERMATH_RULES,
            KeywordProgramKind::Rebound(_) => REBOUND_RULES,
            KeywordProgramKind::Exalted(_) => EXALTED_RULES,
            KeywordProgramKind::Bushido(_) => BUSHIDO_RULES,
            KeywordProgramKind::Wither(_) => WITHER_RULES,
            KeywordProgramKind::Horsemanship(_) => HORSEMANSHIP_RULES,
            KeywordProgramKind::Flanking(_) => FLANKING_RULES,
            KeywordProgramKind::Persist(_) => PERSIST_RULES,
            KeywordProgramKind::Undying(_) => UNDYING_RULES,
            KeywordProgramKind::Toxic(_) => TOXIC_RULES,
            KeywordProgramKind::Daybound(_) => DAYBOUND_RULES,
            KeywordProgramKind::Nightbound(_) => NIGHTBOUND_RULES,
            KeywordProgramKind::StartYourEngines(_) => START_YOUR_ENGINES_RULES,
            KeywordProgramKind::ChooseABackground(_) => CHOOSE_A_BACKGROUND_RULES,
            KeywordProgramKind::DoctorsCompanion(_) => DOCTORS_COMPANION_RULES,
            KeywordProgramKind::Exploit(_) => EXPLOIT_RULES,
            KeywordProgramKind::Soulbond(_) => SOULBOND_RULES,
            KeywordProgramKind::Evolve(_) => EVOLVE_RULES,
            KeywordProgramKind::Improvise(_) => IMPROVISE_RULES,
            KeywordProgramKind::Intimidate(_) => INTIMIDATE_RULES,
            KeywordProgramKind::Spree(_) => SPREE_RULES,
            KeywordProgramKind::Bargain(_) => BARGAIN_RULES,
            KeywordProgramKind::Mentor(_) => MENTOR_RULES,
            KeywordProgramKind::Extort(_) => EXTORT_RULES,
            KeywordProgramKind::LivingWeapon(_) => LIVING_WEAPON_RULES,
            KeywordProgramKind::Myriad(_) => MYRIAD_RULES,
            KeywordProgramKind::Retrace(_) => RETRACE_RULES,
            KeywordProgramKind::Backup(_) => BACKUP_RULES,
            KeywordProgramKind::UmbraArmor(_) => UMBRA_ARMOR_RULES,
            KeywordProgramKind::Cipher(_) => CIPHER_RULES,
            KeywordProgramKind::Renown(_) => RENOWN_RULES,
            KeywordProgramKind::Ascend(_) => ASCEND_RULES,
            KeywordProgramKind::Devoid => DEVOID_RULES,
            KeywordProgramKind::Convoke(_) => CONVOKE_RULES,
            KeywordProgramKind::Equip(_) => EQUIP_RULES,
            KeywordProgramKind::Enchant(_) => ENCHANT_RULES,
            KeywordProgramKind::Saga(_) => SAGA_RULES,
            KeywordProgramKind::CumulativeUpkeep(_) => CUMULATIVE_UPKEEP_RULES,
            KeywordProgramKind::Haste => HASTE_RULES,
            KeywordProgramKind::Vigilance => VIGILANCE_RULES,
            KeywordProgramKind::Trample => TRAMPLE_RULES,
            KeywordProgramKind::Deathtouch => DEATHTOUCH_RULES,
            KeywordProgramKind::Lifelink => LIFELINK_RULES,
            KeywordProgramKind::FirstStrike => FIRST_STRIKE_RULES,
            KeywordProgramKind::DoubleStrike => DOUBLE_STRIKE_RULES,
            KeywordProgramKind::Hexproof(_) => HEXPROOF_RULES,
            KeywordProgramKind::Shroud => SHROUD_RULES,
            KeywordProgramKind::Indestructible => INDESTRUCTIBLE_RULES,
            KeywordProgramKind::Prowess => PROWESS_RULES,
            KeywordProgramKind::Ward(_) => WARD_RULES,
            KeywordProgramKind::Scry => SCRY_RULES,
            KeywordProgramKind::Surveil => SURVEIL_RULES,
            KeywordProgramKind::Cycling(_) => CYCLING_RULES,
        }
    }

    pub fn has_exact_contract(&self) -> bool {
        self.runtime_version == KEYWORD_RULES_RUNTIME_VERSION
            && !self.source.printed_keyword.trim().is_empty()
            && !self.official_rules().is_empty()
            && self
                .official_rules()
                .iter()
                .all(|rule| !rule.id().trim().is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordCompileError {
    EmptyPrintedKeyword,
    UnsupportedKeyword(String),
    MissingOracleFragment {
        keyword: OfficialKeyword,
    },
    MultilineOracleFragment,
    MismatchedOracleFragment {
        keyword: OfficialKeyword,
        fragment: String,
    },
    UnsupportedProtectionQuality(String),
    UnsupportedAttachmentFilter(String),
    UnsupportedSagaSyntax(String),
    InsufficientSourceData {
        keyword: OfficialKeyword,
        detail: String,
    },
    UnsupportedCost(String),
    InvalidManaSymbol(String),
    NoAuthoritativeRules {
        keyword: String,
        effective_date: &'static str,
        source_url: &'static str,
    },
}

impl fmt::Display for KeywordCompileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPrintedKeyword => formatter.write_str("the printed keyword is empty"),
            Self::UnsupportedKeyword(keyword) => {
                write!(formatter, "unsupported official keyword {keyword:?}")
            }
            Self::MissingOracleFragment { keyword } => write!(
                formatter,
                "{} requires its exact Oracle ability fragment",
                keyword.printed_label()
            ),
            Self::MultilineOracleFragment => {
                formatter.write_str("a keyword occurrence must be one Oracle fragment")
            }
            Self::MismatchedOracleFragment { keyword, fragment } => write!(
                formatter,
                "Oracle fragment {fragment:?} does not define {}",
                keyword.printed_label()
            ),
            Self::UnsupportedProtectionQuality(quality) => {
                write!(formatter, "unsupported protection quality {quality:?}")
            }
            Self::UnsupportedAttachmentFilter(filter) => {
                write!(formatter, "unsupported attachment filter {filter:?}")
            }
            Self::UnsupportedSagaSyntax(fragment) => {
                write!(formatter, "unsupported Saga chapter syntax {fragment:?}")
            }
            Self::InsufficientSourceData { keyword, detail } => write!(
                formatter,
                "{} lacks exact source data: {detail}",
                keyword.printed_label()
            ),
            Self::UnsupportedCost(cost) => write!(formatter, "unsupported keyword cost {cost:?}"),
            Self::InvalidManaSymbol(symbol) => {
                write!(formatter, "invalid mana symbol {{{symbol}}}")
            }
            Self::NoAuthoritativeRules {
                keyword,
                effective_date,
                source_url,
            } => write!(
                formatter,
                "{keyword} has no executable definition in the official Comprehensive Rules effective {effective_date} from {source_url}"
            ),
        }
    }
}

impl std::error::Error for KeywordCompileError {}

pub fn compile_keyword_program(
    input: KeywordProgramInput<'_>,
) -> Result<KeywordProgram, KeywordCompileError> {
    let label = normalized_label(input.printed_keyword);
    if label.is_empty() {
        return Err(KeywordCompileError::EmptyPrintedKeyword);
    }
    if label == "conjure" {
        return Err(KeywordCompileError::NoAuthoritativeRules {
            keyword: input.printed_keyword.trim().to_owned(),
            effective_date: KEYWORD_RULES_EFFECTIVE_DATE,
            source_url: KEYWORD_RULES_SOURCE_URL,
        });
    }

    let source = KeywordSourceEvidence {
        face_index: input.face_index,
        clause_index: input.clause_index,
        printed_keyword: input.printed_keyword.trim().to_owned(),
        oracle_fragment: input.oracle_fragment.map(str::trim).map(str::to_owned),
    };
    let kind = match label.as_str() {
        "mill" => KeywordProgramKind::Mill,
        "regenerate" => {
            KeywordProgramKind::Regenerate(parse_regeneration_program(input.oracle_fragment)?)
        }
        "protection" => KeywordProgramKind::Protection(parse_protection_program(
            required_fragment(OfficialKeyword::Protection, input.oracle_fragment)?,
        )?),
        "flying" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Flying, input.oracle_fragment)?;
            KeywordProgramKind::Flying
        }
        "fight" => KeywordProgramKind::Fight,
        "investigate" => KeywordProgramKind::Investigate,
        "kicker" => KeywordProgramKind::Kicker(parse_kicker_program(required_fragment(
            OfficialKeyword::Kicker,
            input.oracle_fragment,
        )?)?),
        "flashback" => KeywordProgramKind::Flashback(parse_flashback_program(required_fragment(
            OfficialKeyword::Flashback,
            input.oracle_fragment,
        )?)?),
        "morph" => KeywordProgramKind::Morph(parse_morph_program(required_fragment(
            OfficialKeyword::Morph,
            input.oracle_fragment,
        )?)?),
        "flash" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Flash, input.oracle_fragment)?;
            KeywordProgramKind::Flash
        }
        "menace" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Menace, input.oracle_fragment)?;
            KeywordProgramKind::Menace
        }
        "defender" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Defender, input.oracle_fragment)?;
            KeywordProgramKind::Defender
        }
        "reach" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Reach, input.oracle_fragment)?;
            KeywordProgramKind::Reach
        }
        "changeling" => {
            KeywordProgramKind::Changeling(parse_changeling_program(input.oracle_fragment)?)
        }
        "infect" => KeywordProgramKind::Infect(parse_infect_program(input.oracle_fragment)?),
        "fear" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Fear, input.oracle_fragment)?;
            KeywordProgramKind::Fear(FearProgram {
                artifact_or_black_blockers_only: true,
            })
        }
        "shadow" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Shadow, input.oracle_fragment)?;
            KeywordProgramKind::Shadow(ShadowProgram {
                requires_matching_shadow_status: true,
            })
        }
        "affinity" | "affinity for artifacts" => {
            KeywordProgramKind::Affinity(parse_affinity_program(required_fragment(
                OfficialKeyword::Affinity,
                input.oracle_fragment,
            )?)?)
        }
        "cascade" => KeywordProgramKind::Cascade(parse_cascade_program(input.oracle_fragment)?),
        "delve" => KeywordProgramKind::Delve(parse_delve_program(input.oracle_fragment)?),
        "fuse" => KeywordProgramKind::Fuse(parse_fuse_program(input.oracle_fragment)?),
        "aftermath" => {
            KeywordProgramKind::Aftermath(parse_aftermath_program(input.oracle_fragment)?)
        }
        "rebound" => KeywordProgramKind::Rebound(parse_rebound_program(input.oracle_fragment)?),
        "exalted" => KeywordProgramKind::Exalted(parse_exalted_program(input.oracle_fragment)?),
        "bushido" => KeywordProgramKind::Bushido(parse_bushido_program(required_fragment(
            OfficialKeyword::Bushido,
            input.oracle_fragment,
        )?)?),
        "wither" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Wither, input.oracle_fragment)?;
            KeywordProgramKind::Wither(WitherProgram {
                creature_damage:
                    WitherCreatureDamageApplication::MinusOneMinusOneCountersEqualToDamage,
                source_controller_places_counters: true,
                uses_last_known_information: true,
                functions_in_all_zones: true,
                instances_are_redundant: true,
            })
        }
        "horsemanship" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Horsemanship, input.oracle_fragment)?;
            KeywordProgramKind::Horsemanship(HorsemanshipProgram {
                block_restriction: HorsemanshipBlockRestriction::BlockerMustHaveHorsemanship,
                creature_with_horsemanship_may_block_either_kind: true,
                instances_are_redundant: true,
            })
        }
        "flanking" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Flanking, input.oracle_fragment)?;
            KeywordProgramKind::Flanking(FlankingProgram {
                trigger_transition: FlankingTriggerTransition::SourceBecomesBlockedByCreature,
                blocker_predicate:
                    FlankingBlockerPredicate::BlockingCreatureWithoutFlanking,
                trigger_multiplicity:
                    FlankingTriggerMultiplicity::OncePerAbilityOccurrencePerQualifyingBlockingCreature,
                instances_trigger_separately: true,
                resolution_recipient:
                    FlankingEffectRecipient::BlockingCreatureIncarnationThatCausedTrigger,
                uses_targeting_system: false,
                power_delta: -1,
                toughness_delta: -1,
                duration: FlankingEffectDuration::UntilEndOfTurn,
            })
        }
        "persist" => KeywordProgramKind::Persist(parse_death_return_program(
            OfficialKeyword::Persist,
            input.oracle_fragment,
        )?),
        "undying" => KeywordProgramKind::Undying(parse_death_return_program(
            OfficialKeyword::Undying,
            input.oracle_fragment,
        )?),
        "toxic" => KeywordProgramKind::Toxic(parse_toxic_program(required_fragment(
            OfficialKeyword::Toxic,
            input.oracle_fragment,
        )?)?),
        "daybound" => KeywordProgramKind::Daybound(parse_day_night_program(
            OfficialKeyword::Daybound,
            input.oracle_fragment,
        )?),
        "nightbound" => KeywordProgramKind::Nightbound(parse_day_night_program(
            OfficialKeyword::Nightbound,
            input.oracle_fragment,
        )?),
        "start your engines!" => KeywordProgramKind::StartYourEngines(
            parse_start_your_engines_program(input.oracle_fragment)?,
        ),
        "choose a background" => {
            KeywordProgramKind::ChooseABackground(parse_commander_partner_program(
                OfficialKeyword::ChooseABackground,
                input.oracle_fragment,
            )?)
        }
        "doctor's companion" => {
            KeywordProgramKind::DoctorsCompanion(parse_commander_partner_program(
                OfficialKeyword::DoctorsCompanion,
                input.oracle_fragment,
            )?)
        }
        "exploit" => KeywordProgramKind::Exploit(parse_exploit_program(input.oracle_fragment)?),
        "soulbond" => KeywordProgramKind::Soulbond(parse_soulbond_program(input.oracle_fragment)?),
        "evolve" => KeywordProgramKind::Evolve(parse_evolve_program(input.oracle_fragment)?),
        "improvise" => {
            KeywordProgramKind::Improvise(parse_improvise_program(input.oracle_fragment)?)
        }
        "intimidate" => {
            KeywordProgramKind::Intimidate(parse_intimidate_program(input.oracle_fragment)?)
        }
        "spree" => KeywordProgramKind::Spree(parse_spree_program(input.oracle_fragment)?),
        "bargain" => KeywordProgramKind::Bargain(parse_bargain_program(input.oracle_fragment)?),
        "mentor" => KeywordProgramKind::Mentor(parse_mentor_program(input.oracle_fragment)?),
        "extort" => KeywordProgramKind::Extort(parse_extort_program(input.oracle_fragment)?),
        "living weapon" => {
            KeywordProgramKind::LivingWeapon(parse_living_weapon_program(input.oracle_fragment)?)
        }
        "myriad" => KeywordProgramKind::Myriad(parse_myriad_program(input.oracle_fragment)?),
        "retrace" => KeywordProgramKind::Retrace(parse_retrace_program(input.oracle_fragment)?),
        "backup" => KeywordProgramKind::Backup(parse_backup_program(input.oracle_fragment)?),
        "umbra armor" => {
            KeywordProgramKind::UmbraArmor(parse_umbra_armor_program(input.oracle_fragment)?)
        }
        "cipher" => KeywordProgramKind::Cipher(parse_cipher_program(input.oracle_fragment)?),
        "renown" => KeywordProgramKind::Renown(parse_renown_program(input.oracle_fragment)?),
        "ascend" => KeywordProgramKind::Ascend(parse_ascend_program(input.oracle_fragment)?),
        label if label == "landwalk" || supported_landwalk_quality(label).is_some() => {
            KeywordProgramKind::Landwalk(parse_landwalk_program(
                label,
                required_fragment(OfficialKeyword::Landwalk, input.oracle_fragment)?,
            )?)
        }
        "devoid" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Devoid, input.oracle_fragment)?;
            KeywordProgramKind::Devoid
        }
        "convoke" => KeywordProgramKind::Convoke(parse_convoke_program(input.oracle_fragment)?),
        "equip" => KeywordProgramKind::Equip(parse_equip_program(required_fragment(
            OfficialKeyword::Equip,
            input.oracle_fragment,
        )?)?),
        "enchant" => KeywordProgramKind::Enchant(parse_enchant_program(required_fragment(
            OfficialKeyword::Enchant,
            input.oracle_fragment,
        )?)?),
        "saga" => KeywordProgramKind::Saga(parse_saga_program(required_multiline_fragment(
            OfficialKeyword::Saga,
            input.oracle_fragment,
        )?)?),
        "cumulative upkeep" => {
            KeywordProgramKind::CumulativeUpkeep(parse_cumulative_upkeep_program(
                required_fragment(OfficialKeyword::CumulativeUpkeep, input.oracle_fragment)?,
            )?)
        }
        "haste" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Haste, input.oracle_fragment)?;
            KeywordProgramKind::Haste
        }
        "vigilance" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Vigilance, input.oracle_fragment)?;
            KeywordProgramKind::Vigilance
        }
        "trample" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Trample, input.oracle_fragment)?;
            KeywordProgramKind::Trample
        }
        "deathtouch" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Deathtouch, input.oracle_fragment)?;
            KeywordProgramKind::Deathtouch
        }
        "lifelink" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Lifelink, input.oracle_fragment)?;
            KeywordProgramKind::Lifelink
        }
        "first strike" => {
            validate_fixed_keyword_fragment(OfficialKeyword::FirstStrike, input.oracle_fragment)?;
            KeywordProgramKind::FirstStrike
        }
        "double strike" => {
            validate_fixed_keyword_fragment(OfficialKeyword::DoubleStrike, input.oracle_fragment)?;
            KeywordProgramKind::DoubleStrike
        }
        "hexproof" => KeywordProgramKind::Hexproof(parse_hexproof_program(input.oracle_fragment)?),
        "shroud" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Shroud, input.oracle_fragment)?;
            KeywordProgramKind::Shroud
        }
        "indestructible" => {
            validate_fixed_keyword_fragment(
                OfficialKeyword::Indestructible,
                input.oracle_fragment,
            )?;
            KeywordProgramKind::Indestructible
        }
        "prowess" => {
            validate_fixed_keyword_fragment(OfficialKeyword::Prowess, input.oracle_fragment)?;
            KeywordProgramKind::Prowess
        }
        "ward" => KeywordProgramKind::Ward(parse_ward_program(required_fragment(
            OfficialKeyword::Ward,
            input.oracle_fragment,
        )?)?),
        "scry" => KeywordProgramKind::Scry,
        "surveil" => KeywordProgramKind::Surveil,
        "cycling" => KeywordProgramKind::Cycling(parse_cycling_program(required_fragment(
            OfficialKeyword::Cycling,
            input.oracle_fragment,
        )?)?),
        _ => {
            return Err(KeywordCompileError::UnsupportedKeyword(
                input.printed_keyword.trim().to_owned(),
            ));
        }
    };

    Ok(KeywordProgram {
        runtime_version: KEYWORD_RULES_RUNTIME_VERSION,
        source,
        kind,
    })
}

fn normalized_label(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn required_fragment(
    keyword: OfficialKeyword,
    fragment: Option<&str>,
) -> Result<&str, KeywordCompileError> {
    let fragment = fragment
        .map(str::trim)
        .filter(|fragment| !fragment.is_empty())
        .ok_or(KeywordCompileError::MissingOracleFragment { keyword })?;
    if fragment.contains('\n') || fragment.contains('\r') {
        return Err(KeywordCompileError::MultilineOracleFragment);
    }
    Ok(fragment)
}

fn required_multiline_fragment(
    keyword: OfficialKeyword,
    fragment: Option<&str>,
) -> Result<&str, KeywordCompileError> {
    fragment
        .map(str::trim)
        .filter(|fragment| !fragment.is_empty())
        .ok_or(KeywordCompileError::MissingOracleFragment { keyword })
}

fn strip_reminder_suffix(fragment: &str) -> Result<&str, KeywordCompileError> {
    let fragment = fragment.trim();
    let mut depth = 0u32;
    let mut group_start = None;
    let mut group_end = None;

    for (index, character) in fragment.char_indices() {
        match character {
            '(' => {
                if depth == 0 {
                    if group_start.is_some() {
                        return Err(KeywordCompileError::UnsupportedCost(fragment.to_owned()));
                    }
                    group_start = Some(index);
                }
                depth = depth
                    .checked_add(1)
                    .ok_or_else(|| KeywordCompileError::UnsupportedCost(fragment.to_owned()))?;
            }
            ')' => {
                if depth == 0 {
                    return Err(KeywordCompileError::UnsupportedCost(fragment.to_owned()));
                }
                depth -= 1;
                if depth == 0 {
                    group_end = Some(index + character.len_utf8());
                }
            }
            _ => {}
        }
    }

    if depth != 0 {
        return Err(KeywordCompileError::UnsupportedCost(fragment.to_owned()));
    }
    let Some(start) = group_start else {
        return Ok(fragment);
    };
    if group_end != Some(fragment.len())
        || !fragment[..start]
            .chars()
            .next_back()
            .is_some_and(char::is_whitespace)
    {
        return Err(KeywordCompileError::UnsupportedCost(fragment.to_owned()));
    }
    let core = fragment[..start].trim_end();
    if core.is_empty() {
        return Err(KeywordCompileError::UnsupportedCost(fragment.to_owned()));
    }
    let reminder = &fragment[start + '('.len_utf8()..fragment.len() - ')'.len_utf8()];
    if !is_canonical_keyword_reminder(core, reminder) {
        return Err(KeywordCompileError::UnsupportedCost(fragment.to_owned()));
    }
    Ok(core)
}

fn normalized_oracle_phrase(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\u{2018}' | '\u{2019}' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            _ => character,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn is_canonical_keyword_reminder(core: &str, reminder: &str) -> bool {
    let core = normalized_oracle_phrase(core);
    let reminder = normalized_oracle_phrase(reminder);
    let exact = |expected: &str| reminder == expected;
    let exact_any = |expected: &[&str]| expected.iter().any(|candidate| exact(candidate));

    match core.as_str() {
        "regenerate" => exact_any(&[
            "the next time this creature would be destroyed this turn, it isn't. instead tap it, remove all damage from it, and remove it from combat.",
            "the next time this permanent would be destroyed this turn, it isn't. instead tap it, remove all damage from it, and remove it from combat.",
            "the next time this object would be destroyed this turn, it isn't. instead tap it, remove all damage from it, and remove it from combat.",
        ]),
        "regenerate target creature." => exact(
            "the next time that creature would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it.",
        ),
        "flying" => exact_any(&[
            "this creature can't be blocked except by creatures with flying or reach.",
            "this object can't be blocked except by creatures with flying or reach.",
        ]),
        "flash" => exact_any(&[
            "you may cast this spell any time you could cast an instant.",
            "you may cast this card any time you could cast an instant.",
            "you may cast this object spell any time you could cast an instant.",
        ]),
        "menace" => exact_any(&[
            "this creature can't be blocked except by two or more creatures.",
            "this object can't be blocked except by two or more creatures.",
        ]),
        "defender" => exact_any(&["this creature can't attack.", "this object can't attack."]),
        "reach" => exact_any(&[
            "this creature can block creatures with flying.",
            "this object can block creatures with flying.",
        ]),
        "changeling" => exact("this card is every creature type."),
        "infect" => exact_any(&[
            "this creature deals damage to creatures in the form of -1/-1 counters and to players in the form of poison counters.",
            "a creature with infect deals damage to creatures in the form of -1/-1 counters and to players in the form of poison counters.",
        ]),
        "fear" => exact(
            "this creature can't be blocked except by artifact creatures and/or black creatures.",
        ),
        "shadow" => exact("this creature can block or be blocked by only creatures with shadow."),
        "wither" => exact("this deals damage to creatures in the form of -1/-1 counters."),
        "horsemanship" => {
            exact("this creature can't be blocked except by creatures with horsemanship.")
        }
        "flanking" => exact(
            "whenever a creature without flanking blocks this creature, the blocking creature gets -1/-1 until end of turn.",
        ),
        "persist" => exact(
            "when this creature dies, if it had no -1/-1 counters on it, return it to the battlefield under its owner's control with a -1/-1 counter on it.",
        ),
        "undying" => exact(
            "when this creature dies, if it had no +1/+1 counters on it, return it to the battlefield under its owner's control with a +1/+1 counter on it.",
        ),
        "daybound" => {
            exact("if a player casts no spells during their own turn, it becomes night next turn.")
        }
        "nightbound" => exact(
            "if a player casts at least two spells during their own turn, it becomes day next turn.",
        ),
        "start your engines!" => exact(
            "if you have no speed, it starts at 1. it increases once on each of your turns when an opponent loses life. max speed is 4.",
        ),
        "choose a background" => exact("you can have a background as a second commander."),
        "doctor's companion" => exact("you can have two commanders if the other is the doctor."),
        "exploit" => exact("when this creature enters, you may sacrifice a creature."),
        "soulbond" => exact(
            "you may pair this creature with another unpaired creature when either enters. they remain paired for as long as you control both of them.",
        ),
        "evolve" => exact(
            "whenever a creature you control enters, if that creature has greater power or toughness than this creature, put a +1/+1 counter on this creature.",
        ),
        "improvise" => exact(
            "your artifacts can help cast this spell. each artifact you tap after you're done activating mana abilities pays for {1}.",
        ),
        "intimidate" => exact(
            "this creature can't be blocked except by artifact creatures and/or creatures that share a color with it.",
        ),
        "spree" => exact("choose one or more additional costs."),
        "cascade" => exact_any(&[
            "when you cast this spell, exile cards from the top of your library until you exile a nonland card that costs less. you may cast it without paying its mana cost. put the exiled cards on the bottom in a random order.",
            "when you cast this spell, exile cards from the top of your library until you exile a nonland card with lesser mana value. you may cast it without paying its mana cost. put the exiled cards on the bottom in a random order.",
        ]),
        "delve" => exact_any(&[
            "each card you exile from your graveyard while casting this spell pays for {1}.",
            "each card you exile from your graveyard while casting this spell pays for 1 generic mana.",
        ]),
        "fuse" => exact("you may cast one or both halves of this card from your hand."),
        "aftermath" => exact("cast this spell only from your graveyard. then exile it."),
        "rebound" => exact(
            "if you cast this spell from your hand, exile it as it resolves. at the beginning of your next upkeep, you may cast this card from exile without paying its mana cost.",
        ),
        "exalted" => exact(
            "whenever a creature you control attacks alone, that creature gets +1/+1 until end of turn.",
        ),
        "ascend" => exact(
            "if you control ten or more permanents, you get the city's blessing for the rest of the game.",
        ),
        "devoid" => exact_any(&["this card has no color.", "this object has no color."]),
        "convoke" => exact_any(&[
            "your creatures can help cast this spell. each creature you tap while casting this spell pays for {1} or one mana of that creature's color.",
            "your creatures can help cast this object spell. each creature you tap while casting this object spell pays for {1} or one mana of that creature's color.",
        ]),
        "haste" => exact_any(&[
            "this creature can attack as soon as it comes under your control.",
            "this creature can attack and {t} as soon as it comes under your control.",
            "this object can attack and {t} as soon as it comes under your control.",
        ]),
        "vigilance" => exact_any(&[
            "attacking doesn't cause this creature to tap.",
            "attacking doesn't cause this object to tap.",
        ]),
        "trample" => exact_any(&[
            "this creature can deal excess combat damage to the player or planeswalker it's attacking.",
            "this creature can deal excess combat damage to the player, planeswalker, or battle it's attacking.",
            "this object can deal excess combat damage to the player or planeswalker it's attacking.",
            "this object can deal excess combat damage to the player, planeswalker, or battle it's attacking.",
        ]),
        "deathtouch" => {
            exact("any amount of damage this deals to a creature is enough to destroy it.")
        }
        "lifelink" => exact_any(&[
            "damage dealt by this creature also causes you to gain that much life.",
            "damage dealt by this object also causes you to gain that much life.",
        ]),
        "first strike" => exact_any(&[
            "this creature deals combat damage before creatures without first strike.",
            "this object deals combat damage before creatures without first strike.",
        ]),
        "double strike" => exact_any(&[
            "this creature deals both first-strike and regular combat damage.",
            "this object deals both first-strike and regular combat damage.",
        ]),
        "shroud" => exact_any(&[
            "a permanent with shroud can't be the target of spells or abilities.",
            "this creature can't be the target of spells or abilities.",
            "this enchantment can't be the target of spells or abilities.",
            "this permanent can't be the target of spells or abilities.",
            "this object can't be the target of spells or abilities.",
        ]),
        "indestructible" => exact_any(&[
            "damage and effects that say \"destroy\" don't destroy this creature.",
            "damage and effects that say \"destroy\" don't destroy this creature. if its toughness is 0 or less, it still dies.",
            "damage and effects that say \"destroy\" don't destroy this creature. if its toughness is 0 or less, it's still put into its owner's graveyard.",
            "effects that say \"destroy\" don't destroy this creature. a creature with indestructible can't be destroyed by damage.",
            "effects that say \"destroy\" don't destroy this equipment.",
            "damage and effects that say \"destroy\" don't destroy this object.",
            "effects that say \"destroy\" don't destroy this object. a creature with indestructible can't be destroyed by damage.",
        ]),
        "prowess" => exact_any(&[
            "whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.",
            "whenever you cast a noncreature spell, this object gets +1/+1 until end of turn.",
        ]),
        "cumulative upkeep" => exact_any(&[
            "at the beginning of your upkeep, put an age counter on this permanent, then sacrifice it unless you pay its upkeep cost for each age counter on it.",
            "at the beginning of your upkeep, put an age counter on this object, then sacrifice it unless you pay its upkeep cost for each age counter on it.",
        ]),
        _ => {
            if let Some(amount_text) = core.strip_prefix("toxic ") {
                let Ok(amount) = amount_text.parse::<u32>() else {
                    return false;
                };
                return toxic_canonical_reminders(amount)
                    .iter()
                    .any(|candidate| normalized_oracle_phrase(candidate) == reminder);
            }
            if let Some(cost) = core.strip_prefix("kicker ") {
                return ["this spell", "this object spell"].iter().any(|source| {
                    exact(&format!(
                        "you may pay an additional {cost} as you cast {source}."
                    ))
                });
            }
            if let Some(cost) = core.strip_prefix("multikicker ") {
                return ["this spell", "this object spell"].iter().any(|source| {
                    exact(&format!(
                        "you may pay an additional {cost} any number of times as you cast {source}."
                    ))
                });
            }
            if core.starts_with("flashback ") {
                return exact_any(&[
                    "you may cast this card from your graveyard for its flashback cost. then exile it.",
                    "you may cast this object from your graveyard for its flashback cost. then exile it.",
                ]);
            }
            if core.starts_with("morph ") {
                return exact_any(&[
                    "you may cast this card face down as a 2/2 creature for {3}. turn it face up any time for its morph cost.",
                    "you may cast this object face down as a 2/2 creature for {3}. turn it face up any time for its morph cost.",
                ]);
            }
            if let Some(body) = core.strip_prefix("equip ") {
                let Some(cost_start) = body.find('{') else {
                    return false;
                };
                let quality = body[..cost_start].trim();
                let cost = body[cost_start..].trim();
                if cost.is_empty() {
                    return false;
                }
                let instruction = if quality.is_empty() {
                    "attach to target creature you control. equip only as a sorcery.".to_owned()
                } else {
                    format!(
                        "attach to target {quality} creature you control. equip only as a sorcery."
                    )
                };
                return exact(&instruction) || exact(&format!("{cost}: {instruction}"));
            }
            if let Some(filter) = core.strip_prefix("enchant ") {
                return match filter {
                    "creature" => exact(
                        "target a creature as you cast this. this card enters attached to that creature.",
                    ),
                    "land" => exact(
                        "target a land as you cast this. this card enters attached to that land.",
                    ),
                    _ => false,
                };
            }
            if core.starts_with("cumulative upkeep ")
                || core.starts_with("cumulative upkeep\u{2014}")
            {
                let generic = exact_any(&[
                    "at the beginning of your upkeep, put an age counter on this permanent, then sacrifice it unless you pay its upkeep cost for each age counter on it.",
                    "at the beginning of your upkeep, put an age counter on this object, then sacrifice it unless you pay its upkeep cost for each age counter on it.",
                ]);
                if generic {
                    return true;
                }
                if core == "cumulative upkeep {s}" {
                    return exact(
                        "at the beginning of your upkeep, put an age counter on this permanent, then sacrifice it unless you pay its upkeep cost for each age counter on it. {s} can be paid with one mana from a snow source.",
                    );
                }
                if let Some(cost) = core.strip_prefix("cumulative upkeep ") {
                    let cost = cost.strip_suffix('.').unwrap_or(cost);
                    return exact(&format!(
                        "at the beginning of your upkeep, put an age counter on this permanent, then sacrifice it unless you pay {cost} for each age counter on it."
                    ));
                }
                return false;
            }
            if let Some(cost) = core.strip_prefix("ward ") {
                return ["creature", "permanent", "object"].iter().any(|noun| {
                    exact(&format!(
                        "whenever this {noun} becomes the target of a spell or ability an opponent controls, counter it unless that player pays {cost}."
                    ))
                });
            }
            if let Some(cost) = core.strip_prefix("cycling ") {
                return ["card", "object"]
                    .iter()
                    .any(|noun| exact(&format!("{cost}, discard this {noun}: draw a card.")));
            }
            if core == "affinity for artifacts" {
                return exact("this spell costs {1} less to cast for each artifact you control.");
            }
            if let Some(quality) = supported_landwalk_quality(&core) {
                return exact(&format!(
                    "this creature can't be blocked as long as defending player controls {}.",
                    quality.reminder_object()
                ));
            }
            if let Some(quality) = core.strip_prefix("protection from ") {
                return ["creature", "permanent", "object"].iter().any(|noun| {
                    exact(&format!(
                        "this {noun} can't be blocked, targeted, dealt damage, enchanted, or equipped by anything {quality}."
                    )) || exact(&format!(
                        "this {noun} can't be blocked, targeted, dealt damage, or enchanted by anything {quality}."
                    ))
                });
            }
            if core == "hexproof" {
                return ["creature", "permanent", "object"].iter().any(|noun| {
                    exact(&format!(
                        "this {noun} can't be the target of spells or abilities your opponents control."
                    ))
                });
            }
            if let Some(quality) = core.strip_prefix("hexproof from ") {
                return ["creature", "permanent", "object"].iter().any(|noun| {
                    exact(&format!(
                        "this {noun} can't be the target of {quality} spells or abilities your opponents control."
                    ))
                });
            }
            false
        }
    }
}

impl LandwalkQuality {
    pub const fn printed_label(self) -> &'static str {
        match self {
            Self::Plains => "Plainswalk",
            Self::Island => "Islandwalk",
            Self::Swamp => "Swampwalk",
            Self::Mountain => "Mountainwalk",
            Self::Forest => "Forestwalk",
            Self::Desert => "Desertwalk",
            Self::LegendaryLand => "Legendary landwalk",
            Self::NonbasicLand => "Nonbasic landwalk",
            Self::SnowLand => "Snow landwalk",
        }
    }

    fn reminder_object(self) -> &'static str {
        match self {
            Self::Plains => "a plains",
            Self::Island => "an island",
            Self::Swamp => "a swamp",
            Self::Mountain => "a mountain",
            Self::Forest => "a forest",
            Self::Desert => "a desert",
            Self::LegendaryLand => "a legendary land",
            Self::NonbasicLand => "a nonbasic land",
            Self::SnowLand => "a snow land",
        }
    }
}

fn supported_landwalk_quality(label: &str) -> Option<LandwalkQuality> {
    match label.trim().to_ascii_lowercase().as_str() {
        "plainswalk" => Some(LandwalkQuality::Plains),
        "islandwalk" => Some(LandwalkQuality::Island),
        "swampwalk" => Some(LandwalkQuality::Swamp),
        "mountainwalk" => Some(LandwalkQuality::Mountain),
        "forestwalk" => Some(LandwalkQuality::Forest),
        "desertwalk" => Some(LandwalkQuality::Desert),
        "legendary landwalk" => Some(LandwalkQuality::LegendaryLand),
        "nonbasic landwalk" => Some(LandwalkQuality::NonbasicLand),
        "snow landwalk" => Some(LandwalkQuality::SnowLand),
        _ => None,
    }
}

fn parse_landwalk_program(
    printed_label: &str,
    fragment: &str,
) -> Result<LandwalkProgram, KeywordCompileError> {
    let core = strip_reminder_suffix(fragment)?;
    let normalized_core = normalized_label(core);
    let Some(quality) = supported_landwalk_quality(&normalized_core) else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Landwalk,
            fragment: fragment.to_owned(),
        });
    };
    let normalized_printed = normalized_label(printed_label);
    if normalized_printed != "landwalk"
        && supported_landwalk_quality(&normalized_printed) != Some(quality)
    {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Landwalk,
            fragment: fragment.to_owned(),
        });
    }
    Ok(LandwalkProgram {
        quality,
        checks_defending_player: true,
        same_kind_instances_are_redundant: true,
    })
}

fn require_exact_oracle_clause_family<'a>(
    keyword: OfficialKeyword,
    fragment: Option<&'a str>,
    accepted: &[&str],
) -> Result<&'a str, KeywordCompileError> {
    let fragment = required_fragment(keyword, fragment)?;
    if !accepted.contains(&fragment) {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword,
            fragment: fragment.to_owned(),
        });
    }
    Ok(fragment)
}

fn parse_changeling_program(
    fragment: Option<&str>,
) -> Result<ChangelingProgram, KeywordCompileError> {
    require_exact_oracle_clause_family(
        OfficialKeyword::Changeling,
        fragment,
        &["Changeling", CHANGELING_CANONICAL_ORACLE_CLAUSE],
    )?;
    Ok(ChangelingProgram {
        is_characteristic_defining_ability: true,
        affected_characteristic: ChangelingCharacteristic::EveryCreatureType,
        applies_to_the_object_with_changeling: true,
        function_scope: ChangelingFunctionScope::EverywhereIncludingOutsideTheGame,
    })
}

fn parse_infect_program(fragment: Option<&str>) -> Result<InfectProgram, KeywordCompileError> {
    const ALTERNATE_ORACLE_REMINDER: &str = "Infect (A creature with infect deals damage to creatures in the form of -1/-1 counters and to players in the form of poison counters.)";
    require_exact_oracle_clause_family(
        OfficialKeyword::Infect,
        fragment,
        &[
            "Infect",
            INFECT_CANONICAL_ORACLE_CLAUSE,
            ALTERNATE_ORACLE_REMINDER,
        ],
    )?;
    Ok(InfectProgram {
        is_static_ability: true,
        applies_to_combat_and_noncombat_damage: true,
        player_damage_result:
            InfectPlayerDamageResult::SourceControllerGivesPoisonCountersEqualToDamageInsteadOfLifeLoss,
        creature_damage_result:
            InfectCreatureDamageResult::SourceControllerPutsMinusOneMinusOneCountersEqualToDamageInsteadOfMarkedDamage,
        uses_damage_after_replacement_and_prevention: true,
        uses_last_known_information_when_source_left_expected_zone: true,
        functions_no_matter_which_zone_source_deals_damage_from: true,
        instances_are_redundant: true,
    })
}

fn parse_affinity_program(fragment: &str) -> Result<AffinityProgram, KeywordCompileError> {
    require_exact_oracle_clause_family(
        OfficialKeyword::Affinity,
        Some(fragment),
        &[
            "Affinity for artifacts",
            AFFINITY_FOR_ARTIFACTS_CANONICAL_ORACLE_CLAUSE,
        ],
    )?;
    Ok(AffinityProgram {
        is_static_ability: true,
        function_scope: SpellStackFunctionScope::WhileThisSpellIsOnTheStack,
        counted_objects: AffinityCountedObjects::ArtifactPermanentsControlledBySpellController,
        generic_mana_reduction_per_counted_object: 1,
        count_uses_current_game_state_during_total_cost_determination: true,
        cannot_reduce_colored_colorless_or_snow_requirements: true,
        cannot_reduce_generic_requirement_below_zero: true,
        multiple_instances_each_apply: true,
    })
}

fn parse_cascade_program(fragment: Option<&str>) -> Result<CascadeProgram, KeywordCompileError> {
    const LESSER_MANA_VALUE_REMINDER: &str = "Cascade (When you cast this spell, exile cards from the top of your library until you exile a nonland card with lesser mana value. You may cast it without paying its mana cost. Put the exiled cards on the bottom in a random order.)";
    require_exact_oracle_clause_family(
        OfficialKeyword::Cascade,
        fragment,
        &[
            "Cascade",
            CASCADE_CANONICAL_ORACLE_CLAUSE,
            LESSER_MANA_VALUE_REMINDER,
        ],
    )?;
    Ok(CascadeProgram {
        is_triggered_ability: true,
        function_scope: SpellStackFunctionScope::WhileThisSpellIsOnTheStack,
        trigger_transition: CascadeTriggerTransition::ControllerCastsThisSpell,
        exile_procedure:
            CascadeExileProcedure::FromLibraryTopUntilFirstNonlandCardWithLesserManaValue,
        source_spell_mana_value_is_strict_upper_bound: true,
        resulting_spell_mana_value_is_rechecked_after_cast_choices: true,
        eligible_card_cast_is_optional: true,
        eligible_card_casts_without_paying_mana_cost: true,
        cast_occurs_during_resolution: true,
        casting_restrictions_and_additional_costs_still_apply: true,
        another_alternative_cost_cannot_be_used: true,
        as_you_cascade_action_window_precedes_cast_choice: true,
        uncast_card_destination: CascadeUncastCardDestination::LibraryBottomInRandomOrder,
        instances_trigger_separately: true,
    })
}

fn parse_delve_program(fragment: Option<&str>) -> Result<DelveProgram, KeywordCompileError> {
    const GENERIC_MANA_REMINDER: &str = "Delve (Each card you exile from your graveyard while casting this spell pays for 1 generic mana.)";
    require_exact_oracle_clause_family(
        OfficialKeyword::Delve,
        fragment,
        &[
            "Delve",
            DELVE_CANONICAL_ORACLE_CLAUSE,
            GENERIC_MANA_REMINDER,
        ],
    )?;
    Ok(DelveProgram {
        is_static_ability: true,
        function_scope: SpellStackFunctionScope::WhileThisSpellIsOnTheStack,
        payment_exchange:
            DelvePaymentExchange::ExileOneCardFromSpellControllersGraveyardForOneGenericMana,
        applies_after_total_cost_is_determined: true,
        applies_only_to_generic_mana_in_total_cost: true,
        is_not_an_additional_or_alternative_cost: true,
        is_not_a_cost_reduction: true,
        each_graveyard_card_can_pay_at_most_once: true,
        multiple_instances_are_redundant: true,
    })
}

fn parse_fuse_program(fragment: Option<&str>) -> Result<FuseProgram, KeywordCompileError> {
    require_exact_oracle_clause_family(
        OfficialKeyword::Fuse,
        fragment,
        &["Fuse", FUSE_CANONICAL_ORACLE_CLAUSE],
    )?;
    Ok(FuseProgram {
        is_static_ability: true,
        function_scope: FuseFunctionScope::CardInItsControllersHand,
        requires_one_physical_split_card_with_exactly_two_halves: true,
        requires_cast_origin_hand: true,
        cast_choice: FuseCastChoice::OneHalfOrBothHalvesChosenBeforeCardIsPutOnStack,
        fused_result_is_one_spell: true,
        fused_spell_has_combined_characteristics_of_both_halves: true,
        total_cost_includes_each_halfs_mana_cost: true,
        resolution_order: FuseResolutionOrder::LeftHalfThenRightHalf,
    })
}

fn parse_rebound_program(fragment: Option<&str>) -> Result<ReboundProgram, KeywordCompileError> {
    require_exact_oracle_clause_family(
        OfficialKeyword::Rebound,
        fragment,
        &["Rebound", REBOUND_CANONICAL_ORACLE_CLAUSE],
    )?;
    Ok(ReboundProgram {
        is_static_ability: true,
        function_scope: SpellStackFunctionScope::WhileThisSpellIsOnTheStack,
        replacement_event:
            ReboundReplacementEvent::CardSpellCastFromHandWouldEnterOwnersGraveyardAsItResolves,
        replacement_exiles_the_same_card: true,
        creates_delayed_trigger_only_when_replacement_exiles_card: true,
        delayed_trigger: ReboundDelayedTrigger::BeginningOfSpellControllersNextUpkeep,
        delayed_cast_from_exile_is_optional: true,
        delayed_cast_without_paying_mana_cost: true,
        casting_restrictions_and_additional_costs_still_apply: true,
        another_alternative_cost_cannot_be_used: true,
        no_effect_for_spell_copy_without_card_or_non_graveyard_destination: true,
        instances_are_redundant: true,
    })
}

fn parse_exalted_program(fragment: Option<&str>) -> Result<ExaltedProgram, KeywordCompileError> {
    require_exact_oracle_clause_family(
        OfficialKeyword::Exalted,
        fragment,
        &["Exalted", EXALTED_CANONICAL_ORACLE_CLAUSE],
    )?;
    Ok(ExaltedProgram {
        is_triggered_ability: true,
        trigger_transition:
            ExaltedTriggerTransition::CreatureControlledByAbilityControllerIsDeclaredAsOnlyAttackerInCombat,
        attacks_alone_counts_only_declared_attackers: true,
        trigger_uses_stack: true,
        affected_creature_is_the_declared_attacker_that_caused_trigger: true,
        uses_targeting: false,
        power_delta: 1,
        toughness_delta: 1,
        duration: ExaltedEffectDuration::UntilEndOfTurn,
        source_need_not_remain_on_battlefield_for_resolution: true,
        later_creatures_entering_attacking_do_not_undo_trigger: true,
        instances_trigger_separately: true,
    })
}

fn parse_ascend_program(fragment: Option<&str>) -> Result<AscendProgram, KeywordCompileError> {
    require_exact_oracle_clause_family(
        OfficialKeyword::Ascend,
        fragment,
        &["Ascend", ASCEND_CANONICAL_ORACLE_CLAUSE],
    )?;
    Ok(AscendProgram {
        spell_application:
            AscendSpellApplication::SpellAbilityChecksDuringInstantOrSorceryResolution,
        permanent_application:
            AscendPermanentApplication::StaticAbilityContinuouslyChecksWhilePermanentIsOnBattlefield,
        permanent_threshold: 10,
        requires_player_not_already_have_citys_blessing: true,
        citys_blessing_persists_for_rest_of_game: true,
        citys_blessing_has_no_inherent_rules_effect: true,
        any_number_of_players_may_have_citys_blessing: true,
        continuous_effects_reapply_before_trigger_condition_checks: true,
    })
}

fn parse_convoke_program(fragment: Option<&str>) -> Result<ConvokeProgram, KeywordCompileError> {
    const OBJECT_SELF_REFERENCE: &str = "Convoke (Your creatures can help cast this object spell. Each creature you tap while casting this object spell pays for {1} or one mana of that creature's color.)";
    require_exact_oracle_clause_family(
        OfficialKeyword::Convoke,
        fragment,
        &[
            "Convoke",
            CONVOKE_CANONICAL_ORACLE_CLAUSE,
            OBJECT_SELF_REFERENCE,
        ],
    )?;
    Ok(ConvokeProgram {
        is_static_ability: true,
        function_scope: SpellStackFunctionScope::WhileThisSpellIsOnTheStack,
        payment_timing:
            ConvokePaymentTiming::AfterTotalCostIsDeterminedAndManaAbilitiesAreActivatedDuringPayment,
        payment_exchange:
            ConvokePaymentExchange::TapOneUntappedControlledCreatureForOneGenericOrOneMatchingColoredMana,
        cannot_pay_colorless_or_snow_requirements: true,
        is_not_an_additional_or_alternative_cost: true,
        is_not_a_cost_reduction: true,
        summoning_sickness_does_not_prevent_payment: true,
        each_creature_can_pay_at_most_once: true,
        tapped_creature_is_designated_as_having_convoked_spell: true,
        multiple_instances_are_redundant: true,
    })
}

fn validate_fixed_keyword_fragment(
    keyword: OfficialKeyword,
    fragment: Option<&str>,
) -> Result<(), KeywordCompileError> {
    let Some(fragment) = fragment else {
        return Ok(());
    };
    let fragment = strip_reminder_suffix(required_fragment(keyword, Some(fragment))?)?;
    if normalized_label(fragment) != normalized_label(keyword.printed_label()) {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword,
            fragment: fragment.to_owned(),
        });
    }
    Ok(())
}

fn parse_regeneration_program(
    fragment: Option<&str>,
) -> Result<RegenerationProgram, KeywordCompileError> {
    let Some(fragment) = fragment else {
        return Ok(RegenerationProgram {
            replacement: RegenerationReplacement::NextDestructionThisTurn,
            recipients: RegenerationRecipientScope::SourcePermanent,
            reminder: RegenerationReminderEvidence::Absent,
        });
    };
    let fragment = required_fragment(OfficialKeyword::Regenerate, Some(fragment))?;
    let normalized = normalized_oracle_phrase(fragment);
    let one_shot = |recipients, reminder| RegenerationProgram {
        replacement: RegenerationReplacement::NextDestructionThisTurn,
        recipients,
        reminder,
    };
    let single_target = |filter| RegenerationRecipientScope::SingleTarget {
        filter,
        cardinality: RegenerationRecipientCardinality::ExactlyOne,
        selection_time: RegenerationRecipientSelectionTime::WhenSpellOrAbilityIsPutOnStack,
    };
    let absent = RegenerationReminderEvidence::Absent;

    match normalized.as_str() {
        "regenerate" => Ok(one_shot(
            RegenerationRecipientScope::SourcePermanent,
            absent,
        )),
        "regenerate target creature." => Ok(one_shot(
            single_target(RegenerationTargetFilter::BattlefieldCreature),
            absent,
        )),
        "regenerate target permanent." => Ok(one_shot(
            single_target(RegenerationTargetFilter::BattlefieldPermanent),
            absent,
        )),
        "regenerate each creature you control." => Ok(one_shot(
            RegenerationRecipientScope::EachCreatureControlledByEffectController {
                cardinality: RegenerationRecipientCardinality::ZeroOrMore,
                selection_time: RegenerationRecipientSelectionTime::OnResolution,
            },
            absent,
        )),
        "regenerate target creature. (the next time that creature would be destroyed this turn, instead tap it, remove it from combat, and heal all damage on it.)" => {
            Ok(one_shot(
                single_target(RegenerationTargetFilter::BattlefieldCreature),
                RegenerationReminderEvidence::CanonicalTargetCreature {
                    referent: RegenerationReminderReferent::SelectedTarget,
                    protection_window: RegenerationProtectionWindow::NextDestructionThisTurn,
                    removes_all_damage: true,
                    controller_taps_recipient: true,
                    removes_from_combat_if_attacking_or_blocking_creature: true,
                },
            ))
        }
        "if this permanent would be destroyed, regenerate it instead" => Ok(RegenerationProgram {
            replacement: RegenerationReplacement::EveryDestructionWhileStaticEffectApplies,
            recipients: RegenerationRecipientScope::SourcePermanent,
            reminder: absent,
        }),
        _ => Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Regenerate,
            fragment: fragment.to_owned(),
        }),
    }
}

fn parse_protection_program(fragment: &str) -> Result<ProtectionProgram, KeywordCompileError> {
    let fragment = strip_reminder_suffix(fragment)?;
    let normalized = normalized_label(fragment);
    let Some(qualities) = normalized.strip_prefix("protection from ") else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Protection,
            fragment: fragment.to_owned(),
        });
    };
    if qualities.contains(" and ") && !qualities.contains(" and from ") {
        return Err(KeywordCompileError::UnsupportedProtectionQuality(
            qualities.to_owned(),
        ));
    }
    let qualities = qualities
        .replace(", and from ", "\u{0}")
        .replace(", from ", "\u{0}")
        .replace(" and from ", "\u{0}");
    let parsed = qualities
        .split('\u{0}')
        .map(str::trim)
        .map(parse_protection_quality)
        .collect::<Result<Vec<_>, _>>()?;
    if parsed.is_empty() {
        return Err(KeywordCompileError::UnsupportedProtectionQuality(
            fragment.to_owned(),
        ));
    }
    Ok(ProtectionProgram { qualities: parsed })
}

fn parse_protection_quality(quality: &str) -> Result<ProtectionQualitySpec, KeywordCompileError> {
    let color = match quality {
        "white" => Some(ManaColor::White),
        "blue" => Some(ManaColor::Blue),
        "black" => Some(ManaColor::Black),
        "red" => Some(ManaColor::Red),
        "green" => Some(ManaColor::Green),
        _ => None,
    };
    if let Some(color) = color {
        return Ok(ProtectionQualitySpec::Color(color));
    }
    match quality {
        "everything" => Ok(ProtectionQualitySpec::Everything),
        "each color" => Ok(ProtectionQualitySpec::EachColor),
        "the color of your choice" | "a color of your choice" | "the chosen color" => {
            Ok(ProtectionQualitySpec::ChosenColor)
        }
        "colored" => Ok(ProtectionQualitySpec::Colored),
        "colorless" => Ok(ProtectionQualitySpec::Colorless),
        "monocolored" => Ok(ProtectionQualitySpec::Monocolored),
        "multicolored" => Ok(ProtectionQualitySpec::Multicolored),
        "artifacts" | "artifact" => Ok(ProtectionQualitySpec::CardType("artifact".into())),
        "battles" | "battle" => Ok(ProtectionQualitySpec::CardType("battle".into())),
        "creatures" | "creature" => Ok(ProtectionQualitySpec::CardType("creature".into())),
        "enchantments" | "enchantment" => Ok(ProtectionQualitySpec::CardType("enchantment".into())),
        "instants" | "instant" => Ok(ProtectionQualitySpec::CardType("instant".into())),
        "kindreds" | "kindred" => Ok(ProtectionQualitySpec::CardType("kindred".into())),
        "lands" | "land" => Ok(ProtectionQualitySpec::CardType("land".into())),
        "planeswalkers" | "planeswalker" => {
            Ok(ProtectionQualitySpec::CardType("planeswalker".into()))
        }
        "sorceries" | "sorcery" => Ok(ProtectionQualitySpec::CardType("sorcery".into())),
        "the chosen player" | "a player" => Ok(ProtectionQualitySpec::ChosenPlayer),
        _ => {
            if let Some(name) = quality.strip_prefix("cards named ")
                && !name.trim().is_empty()
            {
                return Ok(ProtectionQualitySpec::Named(name.trim().to_owned()));
            }
            if let Some(value) = quality
                .strip_prefix("mana value ")
                .and_then(|suffix| suffix.strip_suffix(" or less"))
                .and_then(|value| value.parse::<u32>().ok())
            {
                return Ok(ProtectionQualitySpec::ManaValueAtMost(value));
            }
            if let Some(value) = quality
                .strip_prefix("mana value ")
                .and_then(|suffix| suffix.strip_suffix(" or greater"))
                .and_then(|value| value.parse::<u32>().ok())
            {
                return Ok(ProtectionQualitySpec::ManaValueAtLeast(value));
            }
            Err(KeywordCompileError::UnsupportedProtectionQuality(
                quality.to_owned(),
            ))
        }
    }
}

fn parse_kicker_program(fragment: &str) -> Result<KickerProgram, KeywordCompileError> {
    let fragment = strip_reminder_suffix(fragment)?;
    let normalized = fragment.trim();
    let (multiplicity, costs) = if let Some(cost) = case_insensitive_prefix(normalized, "Kicker ") {
        let costs = cost
            .split(" and/or ")
            .map(parse_mana_cost)
            .collect::<Result<Vec<_>, _>>()?;
        (KickerMultiplicity::OncePerCost, costs)
    } else if let Some(cost) = case_insensitive_prefix(normalized, "Multikicker ") {
        (
            KickerMultiplicity::AnyNumberOfTimes,
            vec![parse_mana_cost(cost)?],
        )
    } else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Kicker,
            fragment: fragment.to_owned(),
        });
    };
    if costs.is_empty() {
        return Err(KeywordCompileError::UnsupportedCost(fragment.to_owned()));
    }
    Ok(KickerProgram {
        costs,
        multiplicity,
        effects_require_linked_kicker_ability: true,
    })
}

fn parse_flashback_program(fragment: &str) -> Result<FlashbackProgram, KeywordCompileError> {
    let fragment = strip_reminder_suffix(fragment)?;
    let Some(cost) = case_insensitive_prefix(fragment, "Flashback ") else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Flashback,
            fragment: fragment.to_owned(),
        });
    };
    Ok(FlashbackProgram {
        alternative_cost: parse_mana_cost(cost)?,
        cast_from_graveyard: true,
        exile_replaces_every_stack_destination: true,
    })
}

fn parse_aftermath_program(
    fragment: Option<&str>,
) -> Result<AftermathProgram, KeywordCompileError> {
    let fragment = required_fragment(OfficialKeyword::Aftermath, fragment)?;
    if fragment != AFTERMATH_CANONICAL_ORACLE_CLAUSE {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Aftermath,
            fragment: fragment.to_owned(),
        });
    }
    Ok(AftermathProgram {
        cast_this_half_from_graveyard: true,
        this_half_cannot_be_cast_from_other_zones: true,
        uses_selected_half_printed_mana_cost: true,
        stack_identity_is_selected_half_only: true,
        exile_replaces_every_stack_destination: true,
        exile_replacement_requires_graveyard_cast_origin: true,
    })
}

fn parse_bushido_program(fragment: &str) -> Result<BushidoProgram, KeywordCompileError> {
    let mismatch = || KeywordCompileError::MismatchedOracleFragment {
        keyword: OfficialKeyword::Bushido,
        fragment: fragment.to_owned(),
    };
    let Some(remainder) = fragment.strip_prefix("Bushido ") else {
        return Err(mismatch());
    };
    let Some((amount_text, _)) = remainder.split_once(" (") else {
        return Err(mismatch());
    };
    if amount_text.is_empty()
        || !amount_text.bytes().all(|byte| byte.is_ascii_digit())
        || amount_text.starts_with('0')
    {
        return Err(mismatch());
    }
    let amount = amount_text.parse::<u32>().map_err(|_| mismatch())?;
    let delta = i32::try_from(amount).map_err(|_| mismatch())?;
    let expected = format!(
        "Bushido {amount} (Whenever this creature blocks or becomes blocked, it gets +{amount}/+{amount} until end of turn.)"
    );
    if fragment != expected {
        return Err(mismatch());
    }

    Ok(BushidoProgram {
        amount,
        trigger_transitions: [
            BushidoBlockTransition::SourceStartsBlocking {
                excludes_entering_battlefield_blocking: true,
            },
            BushidoBlockTransition::SourceAttackerBecomesBlocked,
        ],
        trigger_multiplicity:
            BushidoTriggerMultiplicity::OncePerAbilityOccurrencePerQualifyingTransition,
        instances_trigger_separately: true,
        queued_trigger_survives_source_changes: true,
        resolution_target: BushidoResolutionTarget::SameBattlefieldCreatureIncarnation,
        power_delta: delta,
        toughness_delta: delta,
        duration: BushidoEffectDuration::UntilEndOfTurn,
    })
}

fn parse_death_return_program(
    keyword: OfficialKeyword,
    fragment: Option<&str>,
) -> Result<DeathReturnProgram, KeywordCompileError> {
    let fragment = required_fragment(keyword, fragment)?;
    let (bare, reminded, counter) = match keyword {
        OfficialKeyword::Persist => (
            "Persist",
            PERSIST_CANONICAL_ORACLE_CLAUSE,
            DeathReturnCounterKind::MinusOneMinusOne,
        ),
        OfficialKeyword::Undying => (
            "Undying",
            UNDYING_CANONICAL_ORACLE_CLAUSE,
            DeathReturnCounterKind::PlusOnePlusOne,
        ),
        _ => {
            return Err(KeywordCompileError::MismatchedOracleFragment {
                keyword,
                fragment: fragment.to_owned(),
            });
        }
    };
    if fragment != bare && fragment != reminded {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword,
            fragment: fragment.to_owned(),
        });
    }

    Ok(DeathReturnProgram {
        is_triggered_ability: true,
        trigger_transition: DeathReturnTriggerTransition::BattlefieldPermanentPutIntoGraveyard,
        prohibited_counter: counter,
        counter_condition:
            DeathReturnCounterCondition::NoCounterOfKindImmediatelyBeforeDeathUsingLastKnownInformation,
        trigger_multiplicity:
            DeathReturnTriggerMultiplicity::OncePerAbilityOccurrencePerQualifyingDeath,
        trigger_uses_stack_at_next_priority: true,
        instances_trigger_separately: true,
        linked_card: DeathReturnCardIdentity::NewPublicGraveyardObjectFromTriggeringZoneChange,
        resolution_requirement: DeathReturnResolutionRequirement::LinkedCardRemainsInFirstGraveyard,
        return_under: DeathReturnBattlefieldController::Owner,
        return_counter: counter,
        return_creates_new_battlefield_object: true,
        token_interaction:
            DeathReturnTokenInteraction::TriggerMayExistButTokenCeasesBeforeResolution,
        replacement_interaction:
            DeathReturnReplacementInteraction::ReplacedGraveyardMoveDoesNotTrigger,
    })
}

fn parse_toxic_program(fragment: &str) -> Result<ToxicProgram, KeywordCompileError> {
    let mismatch = || KeywordCompileError::MismatchedOracleFragment {
        keyword: OfficialKeyword::Toxic,
        fragment: fragment.to_owned(),
    };
    let core_end = fragment.find(" (").unwrap_or(fragment.len());
    let core = &fragment[..core_end];
    let Some(amount_text) = core.strip_prefix("Toxic ") else {
        return Err(mismatch());
    };
    if amount_text.is_empty()
        || !amount_text.bytes().all(|byte| byte.is_ascii_digit())
        || amount_text.starts_with('0')
    {
        return Err(mismatch());
    }
    let amount = amount_text.parse::<u32>().map_err(|_| mismatch())?;
    let bare = format!("Toxic {amount}");
    let canonical = if fragment == bare {
        true
    } else {
        toxic_canonical_reminders(amount)
            .into_iter()
            .any(|reminder| fragment == format!("{bare} ({reminder})"))
    };
    if !canonical {
        return Err(mismatch());
    }

    Ok(ToxicProgram {
        amount,
        is_static_ability: true,
        damage_event: ToxicDamageEvent::CombatDamageDealtToPlayerByCreature,
        actual_damage_required: true,
        value_combination: ToxicValueCombination::SumAllToxicAbilityValues,
        poison_application:
            ToxicPoisonApplication::SourceControllerGivesDamagedPlayerCountersInDamageTransaction,
        poison_counters_equal_total_toxic_value: true,
        poison_is_in_addition_to_other_damage_results: true,
    })
}

fn toxic_canonical_reminders(amount: u32) -> Vec<String> {
    let base = if amount == 1 {
        Some("Players dealt combat damage by this creature also get a poison counter.".to_owned())
    } else {
        toxic_number_word(amount).map(|word| {
            format!("Players dealt combat damage by this creature also get {word} poison counters.")
        })
    };
    let Some(base) = base else {
        return Vec::new();
    };
    let mut reminders = vec![base.clone()];
    if amount == 2 {
        reminders.push(format!(
            "{base} A player with ten or more poison counters loses the game."
        ));
    }
    reminders
}

fn toxic_number_word(amount: u32) -> Option<&'static str> {
    match amount {
        2 => Some("two"),
        3 => Some("three"),
        4 => Some("four"),
        5 => Some("five"),
        6 => Some("six"),
        7 => Some("seven"),
        8 => Some("eight"),
        9 => Some("nine"),
        10 => Some("ten"),
        11 => Some("eleven"),
        12 => Some("twelve"),
        13 => Some("thirteen"),
        14 => Some("fourteen"),
        15 => Some("fifteen"),
        16 => Some("sixteen"),
        17 => Some("seventeen"),
        18 => Some("eighteen"),
        19 => Some("nineteen"),
        20 => Some("twenty"),
        _ => None,
    }
}

fn parse_day_night_program(
    keyword: OfficialKeyword,
    fragment: Option<&str>,
) -> Result<DayNightProgram, KeywordCompileError> {
    let fragment = required_fragment(keyword, fragment)?;
    let (
        bare,
        reminded,
        face_role,
        initial_designation,
        entry_behavior,
        invalid_entry_destination,
        immediate_alignment,
        designation_transform,
        zone_scope,
    ) = match keyword {
        OfficialKeyword::Daybound => (
            "Daybound",
            DAYBOUND_CANONICAL_ORACLE_CLAUSE,
            DayNightFaceRole::DayboundFrontFace,
            DayNightInitialDesignation::DayWhenDayboundPermanentIsControlledWhileNeither,
            DayNightEntryBehavior::EnterTransformedAtNightWhenRepresentedByDoubleFacedCard,
            Some(
                DayNightInvalidEntryDestination::InstantOrSorceryBackFaceKeepsNonstackCardInPriorZoneOrPutsResolvingSpellIntoOwnersGraveyard,
            ),
            DayNightImmediateAlignment::TransformFrontFaceUpPermanentAtNight,
            DayNightDesignationTransform::FrontToBackAsItBecomesNight,
            DayNightZoneScope::EntryModificationWhileEnteringAndOtherAbilitiesOnBattlefield,
        ),
        OfficialKeyword::Nightbound => (
            "Nightbound",
            NIGHTBOUND_CANONICAL_ORACLE_CLAUSE,
            DayNightFaceRole::NightboundBackFace,
            DayNightInitialDesignation::NightWhenNightboundPermanentIsControlledWhileNeitherAndNoDayboundPermanentExists,
            DayNightEntryBehavior::NoEntryModification,
            None,
            DayNightImmediateAlignment::TransformBackFaceUpPermanentAtDay,
            DayNightDesignationTransform::BackToFrontAsItBecomesDay,
            DayNightZoneScope::BattlefieldOnly,
        ),
        _ => {
            return Err(KeywordCompileError::MismatchedOracleFragment {
                keyword,
                fragment: fragment.to_owned(),
            });
        }
    };
    if fragment != bare && fragment != reminded {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword,
            fragment: fragment.to_owned(),
        });
    }

    Ok(DayNightProgram {
        is_static_ability: true,
        face_role,
        global_lifecycle:
            DayNightGlobalLifecycle::SinglePersistentMutuallyExclusiveGameDesignationInitiallyNeither,
        initial_designation,
        spell_count_transition: DayNightSpellCountTransition {
            check_during_second_part_of_untap_step: true,
            inspect_previous_active_player_turn: true,
            day_to_night_when_zero_spells: true,
            night_to_day_minimum_spells: 2,
            neither_designation_skips_check: true,
            shared_team_turn_rule:
                DayNightSharedTeamSpellCountRule::DayToNightIfTeamCastNoneAndNightToDayIfAnyOneTeamPlayerCastAtLeastTwo,
        },
        entry_behavior,
        invalid_entry_destination,
        immediate_alignment,
        designation_transform,
        zone_scope,
        transform_batch: DayNightTransformBatch::AllEligibleBattlefieldPermanentsSimultaneously,
        transform_requires_double_faced_card_or_token: true,
        transform_instruction_rejects_instant_or_sorcery_destination: true,
        transform_preserves_object_identity: true,
        other_transform_causes_are_prohibited: true,
        instances_are_redundant: true,
    })
}

fn parse_start_your_engines_program(
    fragment: Option<&str>,
) -> Result<StartYourEnginesProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::StartYourEngines;
    let fragment = required_fragment(keyword, fragment)?;
    if fragment != "Start your engines!" && fragment != START_YOUR_ENGINES_CANONICAL_ORACLE_CLAUSE {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword,
            fragment: fragment.to_owned(),
        });
    }

    Ok(StartYourEnginesProgram {
        is_static_ability: true,
        source_scope: SpeedSourceScope::ControlledPermanentOnBattlefield,
        initialization: SpeedInitialization::NoSpeedToOneAsStateBasedAction,
        initial_speed: 1,
        speed_is_absent_until_set: true,
        inherent_trigger_has_no_source: true,
        inherent_trigger_is_controlled_by_player: true,
        inherent_trigger_uses_stack_at_next_priority: true,
        increase_event: SpeedIncreaseEvent::OneOrMoreOpponentsLoseLifeDuringControllersTurn,
        increase_limit: SpeedIncreaseLimit::OncePerControllerTurn,
        increase_requires_current_speed_below_maximum: true,
        increase_amount: 1,
        increase_instruction_from_no_speed_sets_to_requested_value: true,
        maximum_speed: 4,
        persistence: SpeedPersistence::PlayerRetainsDesignationAfterSourceLeaves,
        no_speed_reads_as_zero_for_effects: true,
        instances_are_redundant: true,
    })
}

fn require_exact_canonical_keyword_clause<'a>(
    keyword: OfficialKeyword,
    fragment: Option<&'a str>,
    canonical_clause: &str,
) -> Result<&'a str, KeywordCompileError> {
    let fragment = required_fragment(keyword, fragment)?;
    if fragment != canonical_clause {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword,
            fragment: fragment.to_owned(),
        });
    }
    Ok(fragment)
}

fn parse_commander_partner_program(
    keyword: OfficialKeyword,
    fragment: Option<&str>,
) -> Result<CommanderPartnerProgram, KeywordCompileError> {
    let (canonical_clause, variant, source_requirement, counterpart_requirement) = match keyword {
        OfficialKeyword::ChooseABackground => (
            CHOOSE_A_BACKGROUND_CANONICAL_ORACLE_CLAUSE,
            CommanderPartnerVariant::ChooseABackground,
            CommanderPartnerSourceRequirement::DistinctLegendaryCardWithThisAbility,
            CommanderPartnerCounterpartRequirement::LegendaryBackgroundEnchantmentCard,
        ),
        OfficialKeyword::DoctorsCompanion => (
            DOCTORS_COMPANION_CANONICAL_ORACLE_CLAUSE,
            CommanderPartnerVariant::DoctorsCompanion,
            CommanderPartnerSourceRequirement::DistinctLegendaryCreatureCardWithThisAbility,
            CommanderPartnerCounterpartRequirement::LegendaryTimeLordDoctorCreatureCardWithNoOtherCreatureTypes,
        ),
        _ => {
            return Err(KeywordCompileError::MismatchedOracleFragment {
                keyword,
                fragment: fragment.unwrap_or_default().to_owned(),
            });
        }
    };
    require_exact_canonical_keyword_clause(keyword, fragment, canonical_clause)?;
    Ok(CommanderPartnerProgram {
        variant,
        functions_only_before_game_for_deck_construction: true,
        source_requirement,
        counterpart_requirement,
        counterpart_needs_same_partner_ability: false,
        commander_count_when_used: 2,
        maximum_commanders_from_partner_abilities: 2,
        deck_card_count_including_commanders: 100,
        both_commanders_start_in_command_zone: true,
        commander_designation_persists_across_zones: true,
        combined_color_identity_for_deck_construction_and_references: true,
        independent_tracking:
            CommanderPartnerTracking::SeparateCastCountsTaxAndCombatDamagePerCommanderPerDamagedPlayer,
        commander_reference:
            CommanderPartnerReference::EitherCommanderAndAffectedPlayerChoosesOneWhenBothCouldBeAffected,
        different_partner_variants_cannot_combine: true,
        choose_only_one_when_source_has_multiple_partner_abilities: true,
    })
}

fn parse_exploit_program(fragment: Option<&str>) -> Result<ExploitProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Exploit;
    require_exact_canonical_keyword_clause(keyword, fragment, EXPLOIT_CANONICAL_ORACLE_CLAUSE)?;
    Ok(ExploitProgram {
        trigger_transition: ExploitTriggerTransition::SourceCreatureEntersBattlefield,
        trigger_uses_stack: true,
        trigger_controller_is_source_controller_at_trigger_time: true,
        sacrifice_choice:
            ExploitSacrificeChoice::OptionalOneCreatureControlledByAbilityControllerOnResolution,
        sacrifice_uses_targeting: false,
        source_may_be_chosen_for_sacrifice: true,
        source_need_not_remain_on_battlefield_for_resolution: true,
        sacrifice_moves_controlled_permanent_from_battlefield_to_owners_graveyard: true,
        sacrifice_destination_is_subject_to_zone_change_replacement: true,
        sacrifice_is_not_destruction: true,
        exploit_event:
            ExploitEventDefinition::SourceExploitsChosenCreatureWhenControllerSacrificesItDuringThisResolution,
        exploit_event_requires_completed_sacrifice_action: true,
        instances_trigger_separately: true,
    })
}

fn parse_soulbond_program(fragment: Option<&str>) -> Result<SoulbondProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Soulbond;
    require_exact_canonical_keyword_clause(keyword, fragment, SOULBOND_CANONICAL_ORACLE_CLAUSE)?;
    Ok(SoulbondProgram {
        represents_two_triggered_abilities: true,
        trigger_set:
            SoulbondTriggerSet::SourceEntersOrAnotherCreatureControlledBySourceControllerEnters,
        trigger_uses_stack: true,
        trigger_controller_is_source_controller_at_trigger_time: true,
        eligibility:
            SoulbondEligibility::BothObjectsAreUnpairedCreaturesOnBattlefieldControlledByAbilityControllerAtTriggerAndResolution,
        source_entry_chooses_another_eligible_creature: true,
        other_entry_is_bound_to_that_entering_creature: true,
        simultaneous_other_entries_each_create_their_own_trigger: true,
        pair_choice: SoulbondPairChoice::OptionalNontargetedChoiceOnResolution,
        pair_lifecycle:
            SoulbondPairLifecycle::SymmetricExclusivePairWhileBothRemainCreaturesOnBattlefieldUnderSameController,
        maximum_partners_per_creature: 1,
        unpair_transition:
            SoulbondUnpairTransition::EitherLeavesBattlefieldStopsBeingCreatureOrChangesController,
        teammate_or_opponent_creatures_are_ineligible: true,
        instances_trigger_separately: true,
    })
}

fn parse_evolve_program(fragment: Option<&str>) -> Result<EvolveProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Evolve;
    require_exact_canonical_keyword_clause(keyword, fragment, EVOLVE_CANONICAL_ORACLE_CLAUSE)?;
    Ok(EvolveProgram {
        trigger_transition:
            EvolveTriggerTransition::CreatureControlledBySourceControllerEntersBattlefield,
        trigger_uses_stack: true,
        trigger_controller_is_source_controller_at_trigger_time: true,
        uses_intervening_if_at_trigger_and_resolution: true,
        comparison: EvolveComparison::EnteringPowerGreaterOrEnteringToughnessGreaterThanSource,
        compares_effective_power_and_toughness: true,
        information_rule:
            EvolveInformationRule::CurrentInformationOrLastKnownInformationForDepartedEnteringCreature,
        comparison_is_false_against_noncreature_permanent: true,
        counter_recipient_is_source_incarnation_on_battlefield: true,
        plus_one_plus_one_counters_per_resolution: 1,
        evolve_event:
            EvolveEventDefinition::OneOrMorePlusOnePlusOneCountersPlacedByResolvingEvolveAbility,
        uses_targeting: false,
        simultaneous_entries_each_create_their_own_trigger: true,
        instances_trigger_separately: true,
    })
}

fn parse_improvise_program(
    fragment: Option<&str>,
) -> Result<ImproviseProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Improvise;
    require_exact_canonical_keyword_clause(keyword, fragment, IMPROVISE_CANONICAL_ORACLE_CLAUSE)?;
    Ok(ImproviseProgram {
        is_static_ability: true,
        function_zone: ImproviseFunctionZone::SpellStackOnly,
        payment_timing:
            ImprovisePaymentTiming::AfterTotalCostLockedAndManaAbilitiesActivatedDuringCostPayment,
        payment_exchange:
            ImprovisePaymentExchange::TapOneUntappedControlledArtifactForOneGenericMana,
        applies_only_to_generic_mana_in_locked_total_cost: true,
        is_not_additional_or_alternative_cost: true,
        is_not_cost_reduction: true,
        payment_is_optional_for_each_generic_mana: true,
        tapped_or_uncontrolled_artifacts_are_ineligible: true,
        summoning_sickness_does_not_prevent_artifact_payment: true,
        one_artifact_cannot_pay_more_than_once: true,
        instances_are_redundant: true,
    })
}

fn parse_intimidate_program(
    fragment: Option<&str>,
) -> Result<IntimidateProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Intimidate;
    require_exact_canonical_keyword_clause(keyword, fragment, INTIMIDATE_CANONICAL_ORACLE_CLAUSE)?;
    Ok(IntimidateProgram {
        is_static_evasion_ability: true,
        blocker_qualification:
            IntimidateBlockerQualification::ArtifactCreatureOrCreatureSharingAtLeastOneCurrentColorWithAttacker,
        every_declared_blocker_must_individually_qualify: true,
        colorless_attacker_requires_artifact_blocker: true,
        checks_current_characteristics_during_block_declaration: true,
        gain_or_loss_after_legal_declaration_does_not_change_block: true,
        later_attacker_or_blocker_characteristic_changes_do_not_change_block: true,
        composes_with_other_block_restrictions: true,
        instances_are_redundant: true,
    })
}

fn parse_spree_program(fragment: Option<&str>) -> Result<SpreeProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Spree;
    require_exact_canonical_keyword_clause(keyword, fragment, SPREE_CANONICAL_ORACLE_CLAUSE)?;
    Ok(SpreeProgram {
        is_static_ability: true,
        function_zone: SpreeFunctionZone::ModalSpellOnStack,
        mode_choice: SpreeModeChoice::ControllerChoosesOneOrMoreLegalModesWhileCasting,
        choose_modes_before_targets: true,
        chosen_mode_must_have_legal_required_targets: true,
        same_mode_normally_cannot_be_chosen_more_than_once: true,
        retargeting_does_not_change_modes: true,
        spell_copy_retains_chosen_modes_without_new_choice: true,
        chosen_modes_resolve_in_printed_order: true,
        mode_cost_binding:
            SpreeModeCostBinding::EveryChosenModeRequiresItsAssociatedPrintedAdditionalCost,
        all_chosen_mode_costs_are_additional_costs: true,
        all_chosen_mode_costs_must_be_paid_without_partial_payment: true,
        mode_costs_do_not_change_mana_cost: true,
        requires_exact_associated_mode_table_from_source: true,
        plus_sign_icons_have_no_rules_meaning: true,
    })
}

fn parse_bargain_program(fragment: Option<&str>) -> Result<BargainProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Bargain;
    require_exact_canonical_keyword_clause(keyword, fragment, BARGAIN_CANONICAL_ORACLE_CLAUSE)?;
    Ok(BargainProgram {
        is_static_ability_on_spell_stack: true,
        is_optional_additional_cost: true,
        sacrifice_choice: BargainSacrificeChoice::OneControlledArtifactEnchantmentOrTokenPermanent,
        sacrifice_is_declared_before_targets: true,
        sacrifice_is_paid_with_total_cost: true,
        bargain_does_not_change_mana_cost: true,
        bargained_status_is_set_when_intention_is_declared: true,
        casting_must_later_pay_declared_cost_to_complete: true,
        linked_effects_reference_only_this_printed_bargain_ability: true,
        conditional_targets_are_chosen_only_when_bargained: true,
        cost_can_be_paid_at_most_once: true,
    })
}

fn parse_mentor_program(fragment: Option<&str>) -> Result<MentorProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Mentor;
    require_exact_canonical_keyword_clause(keyword, fragment, MENTOR_CANONICAL_ORACLE_CLAUSE)?;
    Ok(MentorProgram {
        trigger_transition: MentorTriggerTransition::SourceCreatureDeclaredAsAttacker,
        trigger_uses_stack: true,
        target_restriction:
            MentorTargetRestriction::AttackingCreatureWithCurrentPowerLessThanSourceCurrentPower,
        restriction_checked_on_target_selection_and_resolution: true,
        source_and_target_use_current_power: true,
        plus_one_plus_one_counters: 1,
        counter_is_placed_on_legal_target_on_resolution: true,
        mentor_event_occurs_when_ability_resolves: true,
        instances_trigger_separately: true,
    })
}

fn parse_extort_program(fragment: Option<&str>) -> Result<ExtortProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Extort;
    require_exact_canonical_keyword_clause(keyword, fragment, EXTORT_CANONICAL_ORACLE_CLAUSE)?;
    Ok(ExtortProgram {
        trigger_transition: ExtortTriggerTransition::ControllerCastsSpell,
        trigger_uses_stack: true,
        optional_hybrid_white_black_payment_on_resolution: true,
        payment_may_be_made_at_most_once_per_trigger: true,
        each_opponent_loses_life_simultaneously: 1,
        controller_gains_life_equal_to_total_life_actually_lost: true,
        uses_targeting: false,
        instances_trigger_separately: true,
    })
}

fn parse_living_weapon_program(
    fragment: Option<&str>,
) -> Result<LivingWeaponProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::LivingWeapon;
    require_exact_canonical_keyword_clause(
        keyword,
        fragment,
        LIVING_WEAPON_CANONICAL_ORACLE_CLAUSE,
    )?;
    Ok(LivingWeaponProgram {
        is_enters_battlefield_trigger: true,
        trigger_uses_stack: true,
        token: LivingWeaponTokenDefinition::ZeroZeroBlackPhyrexianGermCreature,
        token_count: 1,
        token_creation_precedes_attachment: true,
        attach_source_equipment_to_created_token: true,
        attachment_does_not_target: true,
        failed_or_illegal_attachment_leaves_equipment_unattached: true,
    })
}

fn parse_myriad_program(fragment: Option<&str>) -> Result<MyriadProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Myriad;
    require_exact_canonical_keyword_clause(keyword, fragment, MYRIAD_CANONICAL_ORACLE_CLAUSE)?;
    Ok(MyriadProgram {
        trigger_transition: MyriadTriggerTransition::SourceCreatureDeclaredAsAttacker,
        trigger_uses_stack: true,
        one_optional_copy_for_each_opponent_other_than_defending_player: true,
        copy_is_token_with_source_copiable_values: true,
        token_enters_tapped_and_attacking: true,
        token_controller_chooses_that_opponent_or_their_planeswalker: true,
        entering_attacking_does_not_trigger_declared_attacker_abilities: true,
        creates_delayed_end_of_combat_exile_trigger_when_any_token_was_created: true,
        delayed_trigger_exiles_only_tokens_created_by_this_resolution: true,
        instances_trigger_separately: true,
    })
}

fn parse_retrace_program(fragment: Option<&str>) -> Result<RetraceProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Retrace;
    require_exact_canonical_keyword_clause(keyword, fragment, RETRACE_CANONICAL_ORACLE_CLAUSE)?;
    Ok(RetraceProgram {
        is_static_ability: true,
        function_zone: RetraceFunctionZone::OwnersGraveyard,
        permits_casting_card_from_graveyard: true,
        discard_one_land_card_is_additional_cost: true,
        printed_and_other_costs_are_still_paid: true,
        normal_casting_timing_and_restrictions_still_apply: true,
        does_not_change_mana_cost: true,
    })
}

fn parse_backup_program(fragment: Option<&str>) -> Result<BackupProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Backup;
    require_exact_canonical_keyword_clause(keyword, fragment, BACKUP_ONE_CANONICAL_ORACLE_CLAUSE)?;
    Ok(BackupProgram {
        counter_count: 1,
        is_enters_battlefield_trigger: true,
        trigger_uses_stack: true,
        targets_one_creature: true,
        places_plus_one_plus_one_counters_on_legal_target: true,
        grants_abilities_only_if_target_is_another_creature: true,
        granted_abilities: BackupGrantedAbilitySet::NonBackupAbilitiesPrintedBelowThisBackupAbility,
        granted_abilities_last_until_end_of_turn: true,
        printed_ability_order_is_copiable_and_preserved: true,
        gained_or_created_abilities_are_not_granted: true,
        granted_ability_set_is_fixed_when_trigger_enters_stack: true,
    })
}

fn parse_umbra_armor_program(
    fragment: Option<&str>,
) -> Result<UmbraArmorProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::UmbraArmor;
    require_exact_canonical_keyword_clause(keyword, fragment, UMBRA_ARMOR_CANONICAL_ORACLE_CLAUSE)?;
    Ok(UmbraArmorProgram {
        is_static_replacement_effect: true,
        replaces_destruction_of_enchanted_permanent: true,
        replacement_is_mandatory: true,
        removes_all_damage_marked_on_enchanted_permanent: true,
        destroys_source_aura: true,
        source_aura_is_destroyed_by_replacement_instruction: true,
        does_not_regenerate_enchanted_permanent: true,
        multiple_applicable_replacements_follow_replacement_choice_rules: true,
    })
}

fn parse_cipher_program(fragment: Option<&str>) -> Result<CipherProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Cipher;
    require_exact_canonical_keyword_clause(keyword, fragment, CIPHER_CANONICAL_ORACLE_CLAUSE)?;
    Ok(CipherProgram {
        spell_ability_functions_on_stack: true,
        requires_spell_represented_by_card: true,
        encode_choice_on_resolution:
            CipherEncodeChoice::OptionalNontargetedCreatureControlledBySpellController,
        exiles_spell_card_encoded_on_chosen_creature: true,
        static_ability_functions_while_card_is_exiled: true,
        relationship_requires_card_in_exile_and_same_creature_object_on_battlefield: true,
        relationship_survives_creature_control_change_or_loss_of_creature_type: true,
        combat_damage_to_player_triggers_for_current_creature_controller: true,
        trigger_copies_encoded_card: true,
        copied_card_may_be_cast_without_paying_mana_cost: true,
        casting_copy_is_optional_and_obeys_other_casting_restrictions: true,
        casting_copy_still_requires_additional_costs_and_cannot_use_another_alternative_cost: true,
        spell_copy_without_a_card_cannot_be_encoded: true,
    })
}

fn parse_renown_program(fragment: Option<&str>) -> Result<RenownProgram, KeywordCompileError> {
    let keyword = OfficialKeyword::Renown;
    require_exact_canonical_keyword_clause(keyword, fragment, RENOWN_ONE_CANONICAL_ORACLE_CLAUSE)?;
    Ok(RenownProgram {
        counter_count: 1,
        triggers_on_combat_damage_to_player: true,
        uses_intervening_if_not_renowned: true,
        trigger_uses_stack: true,
        puts_plus_one_plus_one_counters_on_source: true,
        source_becomes_renowned_after_counter_instruction: true,
        renowned_is_persistent_battlefield_designation: true,
        renowned_is_not_an_ability_or_copiable_value: true,
        designation_ends_when_permanent_leaves_battlefield: true,
        instances_trigger_separately_but_later_resolutions_do_nothing: true,
    })
}

fn parse_morph_program(fragment: &str) -> Result<MorphProgram, KeywordCompileError> {
    let fragment = strip_reminder_suffix(fragment)?;
    let Some(cost) = case_insensitive_prefix(fragment, "Morph ") else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Morph,
            fragment: fragment.to_owned(),
        });
    };
    Ok(MorphProgram {
        face_up_cost: parse_mana_cost(cost)?,
        face_down_cast_cost: parse_mana_cost("{3}")?,
        face_down_power: 2,
        face_down_toughness: 2,
        face_down_has_name: false,
        face_down_has_text: false,
        face_down_has_subtypes: false,
        face_down_has_mana_cost: false,
        turn_face_up_is_special_action: true,
    })
}

fn parse_equip_program(fragment: &str) -> Result<EquipProgram, KeywordCompileError> {
    let fragment = strip_reminder_suffix(fragment)?;
    let Some(body) = case_insensitive_prefix(fragment, "Equip ") else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Equip,
            fragment: fragment.to_owned(),
        });
    };
    let Some(cost_start) = body.find('{') else {
        return Err(KeywordCompileError::UnsupportedCost(body.to_owned()));
    };
    let quality = body[..cost_start].trim();
    let activation_cost = parse_mana_cost(&body[cost_start..])?;
    let (target_filter, planeswalker_as_creature) = if quality.eq_ignore_ascii_case("planeswalker")
    {
        (
            ObjectPredicate::All(vec![
                ObjectPredicate::CardType(CardType::Planeswalker),
                ObjectPredicate::Controller(RelativePlayer::You),
                ObjectPredicate::Zone(Zone::Battlefield),
            ]),
            true,
        )
    } else {
        let quality = quality
            .strip_suffix(" creature")
            .or_else(|| quality.strip_suffix(" Creature"))
            .unwrap_or(quality)
            .trim();
        let mut predicates = vec![
            ObjectPredicate::CardType(CardType::Creature),
            ObjectPredicate::Controller(RelativePlayer::You),
            ObjectPredicate::Zone(Zone::Battlefield),
        ];
        if !quality.is_empty() {
            predicates.push(parse_attachment_quality(quality)?);
        }
        (ObjectPredicate::All(predicates), false)
    };
    Ok(EquipProgram {
        activation_cost,
        target_filter,
        planeswalker_as_creature,
        sorcery_timing_only: true,
    })
}

fn parse_attachment_quality(quality: &str) -> Result<ObjectPredicate, KeywordCompileError> {
    let quality = quality.trim();
    let normalized = normalized_label(quality);
    match normalized.as_str() {
        "legendary" => Ok(ObjectPredicate::Supertype("legendary".into())),
        "artifact" => Ok(ObjectPredicate::CardType(CardType::Artifact)),
        "white" => Ok(ObjectPredicate::Color(ManaColor::White)),
        "blue" => Ok(ObjectPredicate::Color(ManaColor::Blue)),
        "black" => Ok(ObjectPredicate::Color(ManaColor::Black)),
        "red" => Ok(ObjectPredicate::Color(ManaColor::Red)),
        "green" => Ok(ObjectPredicate::Color(ManaColor::Green)),
        "commander" => Ok(ObjectPredicate::Commander),
        _ => Err(KeywordCompileError::UnsupportedAttachmentFilter(
            quality.to_owned(),
        )),
    }
}

fn parse_enchant_program(fragment: &str) -> Result<EnchantProgram, KeywordCompileError> {
    let fragment = strip_reminder_suffix(fragment)?;
    let Some(filter) = case_insensitive_prefix(fragment, "Enchant ") else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Enchant,
            fragment: fragment.to_owned(),
        });
    };
    let target_filter = parse_enchant_filter(filter)?;
    Ok(EnchantProgram {
        target_filter,
        aura_spell_targets: true,
        all_enchant_abilities_must_match: true,
    })
}

fn parse_enchant_filter(filter: &str) -> Result<AttachmentFilter, KeywordCompileError> {
    let normalized = normalized_label(filter);
    match normalized.as_str() {
        "player" => return Ok(AttachmentFilter::Player(RelativePlayer::Any)),
        "you" => return Ok(AttachmentFilter::Player(RelativePlayer::You)),
        "opponent" => return Ok(AttachmentFilter::Player(RelativePlayer::Opponent)),
        _ => {}
    }

    let (without_controller, controller) =
        if let Some(base) = normalized.strip_suffix(" you control") {
            (base.trim(), Some(RelativePlayer::You))
        } else if let Some(base) = normalized.strip_suffix(" an opponent controls") {
            (base.trim(), Some(RelativePlayer::Opponent))
        } else {
            (normalized.as_str(), None)
        };
    let (without_zone, zone) =
        if let Some(base) = without_controller.strip_suffix(" card in a graveyard") {
            (base.trim(), Zone::Graveyard)
        } else {
            (without_controller, Zone::Battlefield)
        };

    let mut predicates = vec![ObjectPredicate::Zone(zone)];
    if let Some(controller) = controller {
        predicates.push(ObjectPredicate::Controller(controller));
    }
    let base = if let Some(base) = without_zone.strip_prefix("tapped ") {
        predicates.push(ObjectPredicate::Tapped);
        base
    } else {
        without_zone
    };
    let predicate = match base {
        "permanent" => ObjectPredicate::Permanent,
        "nonland permanent" => ObjectPredicate::All(vec![
            ObjectPredicate::Permanent,
            ObjectPredicate::Not(Box::new(ObjectPredicate::CardType(CardType::Land))),
        ]),
        "artifact" => ObjectPredicate::CardType(CardType::Artifact),
        "battle" => ObjectPredicate::CardType(CardType::Battle),
        "creature" => ObjectPredicate::CardType(CardType::Creature),
        "enchantment" => ObjectPredicate::CardType(CardType::Enchantment),
        "land" => ObjectPredicate::CardType(CardType::Land),
        "planeswalker" => ObjectPredicate::CardType(CardType::Planeswalker),
        "artifact or creature" => ObjectPredicate::Any(vec![
            ObjectPredicate::CardType(CardType::Artifact),
            ObjectPredicate::CardType(CardType::Creature),
        ]),
        _ => {
            if base.contains(" or ") || base.contains(" and ") || base.contains(" with ") {
                return Err(KeywordCompileError::UnsupportedAttachmentFilter(
                    filter.to_owned(),
                ));
            }
            parse_attachment_quality(base)?
        }
    };
    predicates.push(predicate);
    Ok(AttachmentFilter::Object(ObjectPredicate::All(predicates)))
}

fn parse_saga_program(fragment: &str) -> Result<SagaProgram, KeywordCompileError> {
    if fragment.lines().any(|line| {
        normalized_label(line.trim_start_matches('(').trim_end_matches(')')) == "read ahead"
    }) {
        return Err(KeywordCompileError::InsufficientSourceData {
            keyword: OfficialKeyword::Saga,
            detail: "Read ahead entry replacement is outside this Saga lifecycle contract".into(),
        });
    }
    let mut chapters = Vec::new();
    for line in fragment
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        if line.starts_with('(') && line.ends_with(')') {
            continue;
        }
        let split = line
            .split_once('\u{2014}')
            .or_else(|| line.split_once(" - "));
        let Some((numbers, effect)) = split else {
            return Err(KeywordCompileError::UnsupportedSagaSyntax(line.to_owned()));
        };
        let numbers = numbers
            .split(',')
            .map(str::trim)
            .map(parse_roman_chapter)
            .collect::<Result<Vec<_>, _>>()?;
        if numbers.is_empty() || effect.trim().is_empty() {
            return Err(KeywordCompileError::UnsupportedSagaSyntax(line.to_owned()));
        }
        chapters.push(SagaChapter {
            numbers,
            oracle_effect: effect.trim().to_owned(),
        });
    }
    let final_chapter = chapters
        .iter()
        .flat_map(|chapter| chapter.numbers.iter().copied())
        .max()
        .ok_or_else(|| KeywordCompileError::InsufficientSourceData {
            keyword: OfficialKeyword::Saga,
            detail: "no chapter symbols were supplied".into(),
        })?;
    Ok(SagaProgram {
        chapters,
        final_chapter,
        enters_with_one_lore_counter: true,
    })
}

fn parse_roman_chapter(value: &str) -> Result<u32, KeywordCompileError> {
    let value = value.trim().to_ascii_uppercase();
    if value.is_empty()
        || !value
            .chars()
            .all(|character| matches!(character, 'I' | 'V' | 'X'))
    {
        return Err(KeywordCompileError::UnsupportedSagaSyntax(value));
    }
    let mut total = 0u32;
    let mut previous = 0u32;
    for character in value.chars().rev() {
        let current = match character {
            'I' => 1,
            'V' => 5,
            'X' => 10,
            _ => unreachable!(),
        };
        if current < previous {
            total = total
                .checked_sub(current)
                .ok_or_else(|| KeywordCompileError::UnsupportedSagaSyntax(value.clone()))?;
        } else {
            total = total
                .checked_add(current)
                .ok_or_else(|| KeywordCompileError::UnsupportedSagaSyntax(value.clone()))?;
            previous = current;
        }
    }
    if total == 0 || total > 20 {
        return Err(KeywordCompileError::UnsupportedSagaSyntax(value));
    }
    if canonical_roman_chapter(total) != value {
        return Err(KeywordCompileError::UnsupportedSagaSyntax(value));
    }
    Ok(total)
}

fn canonical_roman_chapter(mut value: u32) -> String {
    let mut roman = String::new();
    for (amount, symbol) in [(10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")] {
        while value >= amount {
            roman.push_str(symbol);
            value -= amount;
        }
    }
    roman
}

fn parse_cumulative_upkeep_program(
    fragment: &str,
) -> Result<CumulativeUpkeepProgram, KeywordCompileError> {
    let fragment = strip_reminder_suffix(fragment)?;
    let Some(body) = case_insensitive_prefix(fragment, "Cumulative upkeep") else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::CumulativeUpkeep,
            fragment: fragment.to_owned(),
        });
    };
    let body = body.trim_start_matches(|character: char| {
        character.is_whitespace()
            || character == ':'
            || character == '-'
            || character == '\u{2013}'
            || character == '\u{2014}'
    });
    let body = body.strip_suffix('.').unwrap_or(body).trim_end();
    if body.is_empty() {
        return Err(KeywordCompileError::UnsupportedCost(fragment.to_owned()));
    }
    let cost_per_age_counter = if let Some(life) =
        case_insensitive_prefix(body, "Pay ").and_then(|payment| {
            payment
                .strip_suffix(" life")
                .and_then(|value| value.trim().parse::<u32>().ok())
        }) {
        CumulativeUpkeepCost::PayLife(life)
    } else {
        let alternatives = body
            .split(" or ")
            .map(parse_mana_cost)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| KeywordCompileError::InsufficientSourceData {
                keyword: OfficialKeyword::CumulativeUpkeep,
                detail: format!("cost {body:?} is not an executable mana or life cost"),
            })?;
        CumulativeUpkeepCost::ManaAlternatives(alternatives)
    };
    Ok(CumulativeUpkeepProgram {
        cost_per_age_counter,
        partial_payment_allowed: false,
    })
}

fn parse_hexproof_program(fragment: Option<&str>) -> Result<HexproofProgram, KeywordCompileError> {
    let Some(fragment) = fragment else {
        return Ok(HexproofProgram { qualities: None });
    };
    let fragment = strip_reminder_suffix(required_fragment(
        OfficialKeyword::Hexproof,
        Some(fragment),
    )?)?;
    if fragment.eq_ignore_ascii_case("Hexproof") {
        return Ok(HexproofProgram { qualities: None });
    }
    let Some(qualities) = case_insensitive_prefix(fragment, "Hexproof from ") else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Hexproof,
            fragment: fragment.to_owned(),
        });
    };
    let protection = parse_protection_program(&format!("Protection from {qualities}"))?;
    Ok(HexproofProgram {
        qualities: Some(protection.qualities),
    })
}

fn parse_ward_program(fragment: &str) -> Result<WardProgram, KeywordCompileError> {
    let fragment = strip_reminder_suffix(fragment)?;
    let Some(cost) = case_insensitive_prefix(fragment, "Ward ") else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Ward,
            fragment: fragment.to_owned(),
        });
    };
    let cost = parse_mana_cost(cost)?;
    if cost
        .symbols
        .iter()
        .any(|symbol| matches!(symbol, ManaSymbol::VariableX))
    {
        return Err(KeywordCompileError::InsufficientSourceData {
            keyword: OfficialKeyword::Ward,
            detail: "Ward X requires the exact Oracle expression that defines X".into(),
        });
    }
    Ok(WardProgram {
        cost,
        variable_value_requires_resolution_state: false,
    })
}

fn parse_cycling_program(fragment: &str) -> Result<CyclingProgram, KeywordCompileError> {
    let fragment = strip_reminder_suffix(fragment)?;
    let Some(cost) = case_insensitive_prefix(fragment, "Cycling ") else {
        return Err(KeywordCompileError::MismatchedOracleFragment {
            keyword: OfficialKeyword::Cycling,
            fragment: fragment.to_owned(),
        });
    };
    Ok(CyclingProgram {
        activation_cost: parse_mana_cost(cost)?,
        activation_zone: Zone::Hand,
        discard_self_is_cost: true,
        draws: 1,
    })
}

fn case_insensitive_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let prefix_length = prefix.len();
    if value.len() < prefix_length || !value.is_char_boundary(prefix_length) {
        return None;
    }
    value[..prefix_length]
        .eq_ignore_ascii_case(prefix)
        .then_some(value[prefix_length..].trim())
}

fn parse_mana_cost(cost: &str) -> Result<ManaCost, KeywordCompileError> {
    let cost = cost.trim();
    if cost.is_empty() {
        return Err(KeywordCompileError::UnsupportedCost(cost.to_owned()));
    }
    let mut symbols = Vec::new();
    let mut offset = 0usize;
    while offset < cost.len() {
        let remaining = &cost[offset..];
        let Some(character) = remaining.chars().next() else {
            break;
        };
        if character.is_whitespace() {
            offset += character.len_utf8();
            continue;
        }
        if character != '{' {
            return Err(KeywordCompileError::UnsupportedCost(cost.to_owned()));
        }
        let start = offset + 1;
        let Some(relative_end) = cost[start..].find('}') else {
            return Err(KeywordCompileError::UnsupportedCost(cost.to_owned()));
        };
        let end = start + relative_end;
        if cost[start..end].contains('{') {
            return Err(KeywordCompileError::UnsupportedCost(cost.to_owned()));
        }
        let token = cost[start..end].trim().to_ascii_uppercase();
        symbols.push(parse_mana_symbol(&token)?);
        offset = end + 1;
    }
    if symbols.is_empty() {
        return Err(KeywordCompileError::UnsupportedCost(cost.to_owned()));
    }
    Ok(ManaCost {
        raw: cost.to_owned(),
        symbols,
    })
}

fn parse_mana_symbol(symbol: &str) -> Result<ManaSymbol, KeywordCompileError> {
    if symbol.chars().all(|character| character.is_ascii_digit()) {
        return symbol
            .parse::<u32>()
            .map(ManaSymbol::Generic)
            .map_err(|_| KeywordCompileError::InvalidManaSymbol(symbol.to_owned()));
    }
    if let Some(color) = parse_color_symbol(symbol) {
        return Ok(if color == ManaColor::Colorless {
            ManaSymbol::Colorless
        } else {
            ManaSymbol::Colored(color)
        });
    }
    match symbol {
        "S" => return Ok(ManaSymbol::Snow),
        "X" => return Ok(ManaSymbol::VariableX),
        _ => {}
    }
    let alternatives = symbol.split('/').collect::<Vec<_>>();
    match alternatives.as_slice() {
        [color, "P"] => parse_color_symbol(color)
            .filter(|color| *color != ManaColor::Colorless)
            .map(ManaSymbol::Phyrexian)
            .ok_or_else(|| KeywordCompileError::InvalidManaSymbol(symbol.to_owned())),
        [first, second] => {
            let first = parse_color_symbol(first)
                .filter(|color| *color != ManaColor::Colorless)
                .ok_or_else(|| KeywordCompileError::InvalidManaSymbol(symbol.to_owned()))?;
            let second = parse_color_symbol(second)
                .filter(|color| *color != ManaColor::Colorless)
                .ok_or_else(|| KeywordCompileError::InvalidManaSymbol(symbol.to_owned()))?;
            Ok(ManaSymbol::Hybrid(first, second))
        }
        _ => Err(KeywordCompileError::InvalidManaSymbol(symbol.to_owned())),
    }
}

fn parse_color_symbol(symbol: &str) -> Option<ManaColor> {
    match symbol {
        "W" => Some(ManaColor::White),
        "U" => Some(ManaColor::Blue),
        "B" => Some(ManaColor::Black),
        "R" => Some(ManaColor::Red),
        "G" => Some(ManaColor::Green),
        "C" => Some(ManaColor::Colorless),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ManaUnitId(pub u64);

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
    fn normalized_name(self) -> &'static str {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CombatKeyword {
    Flying,
    Reach,
    Menace,
    Fear,
    Shadow,
    Defender,
    Haste,
    Vigilance,
    Trample,
    Deathtouch,
    Lifelink,
    FirstStrike,
    DoubleStrike,
    Indestructible,
    Prowess,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObjectCharacteristics {
    pub name: Option<String>,
    pub card_types: BTreeSet<CardType>,
    pub supertypes: BTreeSet<String>,
    pub subtypes: BTreeSet<String>,
    pub colors: BTreeSet<ManaColor>,
    pub mana_value: u32,
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub oracle_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtectionQuality {
    Everything,
    Color(ManaColor),
    Colored,
    Colorless,
    Monocolored,
    Multicolored,
    CardType(String),
    Subtype(String),
    Named(String),
    Player(PlayerId),
    ManaValueAtMost(u32),
    ManaValueAtLeast(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastMethod {
    Ordinary,
    Flashback,
    MorphFaceDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordObject {
    pub id: ObjectId,
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub zone: Zone,
    pub printed: ObjectCharacteristics,
    pub is_token: bool,
    pub is_commander: bool,
    pub has_reconfigure: bool,
    pub controlled_since_turn_began: bool,
    pub tapped: bool,
    pub attacking: bool,
    pub blocking: bool,
    pub damage_marked: u32,
    pub damaged_by_deathtouch_since_state_check: bool,
    pub temporary_power_delta: i32,
    pub temporary_toughness_delta: i32,
    pub combat_keywords: BTreeSet<CombatKeyword>,
    pub landwalk_instances: BTreeMap<LandwalkQuality, u16>,
    pub rules_keywords: BTreeSet<OfficialKeyword>,
    pub keyword_instances: BTreeMap<OfficialKeyword, u16>,
    pub protections: Vec<ProtectionQuality>,
    pub has_hexproof: bool,
    pub hexproof_qualities: Vec<ProtectionQuality>,
    pub has_shroud: bool,
    pub ward_costs: Vec<ManaCost>,
    pub counters: BTreeMap<String, u32>,
    pub attached_to: Option<ProtectionTarget>,
    pub regeneration_shields: u16,
    pub static_regeneration: bool,
    pub face_down: bool,
    pub cast_method: Option<CastMethod>,
    pub kicker_payments: Vec<usize>,
    pub morph_face_up_cost: Option<ManaCost>,
}

impl KeywordObject {
    pub fn new(
        id: ObjectId,
        owner: PlayerId,
        controller: PlayerId,
        zone: Zone,
        printed: ObjectCharacteristics,
    ) -> Self {
        Self {
            id,
            owner,
            controller,
            zone,
            printed,
            is_token: false,
            is_commander: false,
            has_reconfigure: false,
            controlled_since_turn_began: true,
            tapped: false,
            attacking: false,
            blocking: false,
            damage_marked: 0,
            damaged_by_deathtouch_since_state_check: false,
            temporary_power_delta: 0,
            temporary_toughness_delta: 0,
            combat_keywords: BTreeSet::new(),
            landwalk_instances: BTreeMap::new(),
            rules_keywords: BTreeSet::new(),
            keyword_instances: BTreeMap::new(),
            protections: Vec::new(),
            has_hexproof: false,
            hexproof_qualities: Vec::new(),
            has_shroud: false,
            ward_costs: Vec::new(),
            counters: BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            static_regeneration: false,
            face_down: false,
            cast_method: None,
            kicker_payments: Vec::new(),
            morph_face_up_cost: None,
        }
    }

    pub fn effective_characteristics(&self) -> ObjectCharacteristics {
        let mut characteristics = if !self.face_down {
            let mut characteristics = self.printed.clone();
            if self.rules_keywords.contains(&OfficialKeyword::Devoid) {
                characteristics.colors.clear();
            }
            characteristics
        } else {
            ObjectCharacteristics {
                name: None,
                card_types: BTreeSet::from([CardType::Creature]),
                supertypes: BTreeSet::new(),
                subtypes: BTreeSet::new(),
                colors: BTreeSet::new(),
                mana_value: 0,
                power: Some(2),
                toughness: Some(2),
                oracle_text: None,
            }
        };
        if let Some(power) = characteristics.power.as_mut() {
            *power = power.saturating_add(self.temporary_power_delta);
        }
        if let Some(toughness) = characteristics.toughness.as_mut() {
            *toughness = toughness.saturating_add(self.temporary_toughness_delta);
        }
        characteristics
    }

    fn is_creature(&self) -> bool {
        self.effective_characteristics()
            .card_types
            .contains(&CardType::Creature)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManaUnit {
    pub id: ManaUnitId,
    pub color: ManaColor,
    pub from_snow_source: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordPlayerState {
    pub id: PlayerId,
    pub life: i32,
    /// The front of the deque is the top of the library.
    pub library: VecDeque<ObjectId>,
    pub hand: Vec<ObjectId>,
    pub graveyard: Vec<ObjectId>,
    pub exile: Vec<ObjectId>,
    pub command: Vec<ObjectId>,
    pub mana_pool: Vec<ManaUnit>,
    pub protections: Vec<ProtectionQuality>,
    pub has_hexproof: bool,
    pub hexproof_qualities: Vec<ProtectionQuality>,
    pub has_shroud: bool,
    pub failed_draw_attempts: u32,
}

impl KeywordPlayerState {
    pub fn new(id: PlayerId, life: i32) -> Self {
        Self {
            id,
            life,
            library: VecDeque::new(),
            hand: Vec::new(),
            graveyard: Vec::new(),
            exile: Vec::new(),
            command: Vec::new(),
            mana_pool: Vec::new(),
            protections: Vec::new(),
            has_hexproof: false,
            hexproof_qualities: Vec::new(),
            has_shroud: false,
            failed_draw_attempts: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEquipActivation {
    pub equipment: ObjectId,
    pub target: ObjectId,
    pub activating_player: PlayerId,
    pub target_filter: ObjectPredicate,
    pub planeswalker_as_creature: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSagaChapter {
    pub saga: ObjectId,
    pub chapter_number: u32,
    pub oracle_effect: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeywordGameState {
    pub players: BTreeMap<PlayerId, KeywordPlayerState>,
    pub objects: BTreeMap<ObjectId, KeywordObject>,
    pub next_object_id: u64,
    pub pending_equip: BTreeMap<ObjectId, PendingEquipActivation>,
    pub pending_saga_chapters: Vec<PendingSagaChapter>,
    pub countered_abilities: BTreeSet<u64>,
}

impl KeywordGameState {
    pub fn add_player(&mut self, player: KeywordPlayerState) -> Result<(), KeywordExecutionError> {
        if self.players.insert(player.id, player).is_some() {
            return Err(KeywordExecutionError::DuplicatePlayer);
        }
        Ok(())
    }

    pub fn insert_object(&mut self, object: KeywordObject) -> Result<(), KeywordExecutionError> {
        if !self.players.contains_key(&object.owner)
            || !self.players.contains_key(&object.controller)
        {
            return Err(KeywordExecutionError::MissingPlayer);
        }
        if self.objects.contains_key(&object.id) {
            return Err(KeywordExecutionError::DuplicateObject);
        }
        self.place_in_owned_zone(object.owner, object.id, object.zone)?;
        self.next_object_id = self.next_object_id.max(object.id.0.saturating_add(1));
        self.objects.insert(object.id, object);
        Ok(())
    }

    pub fn object(&self, object: ObjectId) -> Result<&KeywordObject, KeywordExecutionError> {
        self.objects
            .get(&object)
            .ok_or(KeywordExecutionError::MissingObject(object))
    }

    fn object_mut(
        &mut self,
        object: ObjectId,
    ) -> Result<&mut KeywordObject, KeywordExecutionError> {
        self.objects
            .get_mut(&object)
            .ok_or(KeywordExecutionError::MissingObject(object))
    }

    fn move_object(
        &mut self,
        object: ObjectId,
        destination: Zone,
    ) -> Result<(), KeywordExecutionError> {
        let (owner, current_zone) = {
            let object = self.object(object)?;
            (object.owner, object.zone)
        };
        self.remove_from_owned_zone(owner, object, current_zone)?;
        self.place_in_owned_zone(owner, object, destination)?;
        self.object_mut(object)?.zone = destination;
        Ok(())
    }

    fn remove_from_owned_zone(
        &mut self,
        owner: PlayerId,
        object: ObjectId,
        zone: Zone,
    ) -> Result<(), KeywordExecutionError> {
        let Some(player) = self.players.get_mut(&owner) else {
            return Err(KeywordExecutionError::MissingPlayer);
        };
        let removed = match zone {
            Zone::Library => remove_from_deque(&mut player.library, object),
            Zone::Hand => remove_from_vec(&mut player.hand, object),
            Zone::Graveyard => remove_from_vec(&mut player.graveyard, object),
            Zone::Exile => remove_from_vec(&mut player.exile, object),
            Zone::Command => remove_from_vec(&mut player.command, object),
            Zone::Battlefield | Zone::Stack => true,
        };
        if removed {
            Ok(())
        } else {
            Err(KeywordExecutionError::InconsistentZoneMembership { object, zone })
        }
    }

    fn place_in_owned_zone(
        &mut self,
        owner: PlayerId,
        object: ObjectId,
        zone: Zone,
    ) -> Result<(), KeywordExecutionError> {
        let Some(player) = self.players.get_mut(&owner) else {
            return Err(KeywordExecutionError::MissingPlayer);
        };
        match zone {
            Zone::Library => player.library.push_front(object),
            Zone::Hand => player.hand.push(object),
            Zone::Graveyard => player.graveyard.push(object),
            Zone::Exile => player.exile.push(object),
            Zone::Command => player.command.push(object),
            Zone::Battlefield | Zone::Stack => {}
        }
        Ok(())
    }
}

fn remove_from_vec(objects: &mut Vec<ObjectId>, object: ObjectId) -> bool {
    let Some(index) = objects.iter().position(|candidate| *candidate == object) else {
        return false;
    };
    objects.remove(index);
    true
}

fn remove_from_deque(objects: &mut VecDeque<ObjectId>, object: ObjectId) -> bool {
    let Some(index) = objects.iter().position(|candidate| *candidate == object) else {
        return false;
    };
    objects.remove(index);
    true
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolPayment {
    Mana(Vec<ManaUnitId>),
    Life(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ManaPayment {
    pub symbols: BTreeMap<usize, SymbolPayment>,
    pub x_value: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MillMode {
    Instruction,
    Choice,
    Cost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionTarget {
    Object(ObjectId),
    Player(PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetStatus {
    NotTargeted,
    LegalTarget,
    IllegalTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CumulativeUpkeepPayment {
    pub alternative_index: usize,
    pub mana_payment: ManaPayment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageRecipient {
    Object(ObjectId),
    Player(PlayerId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrampleBlockerAssignment {
    pub blocker: ObjectId,
    pub assigned_damage: u32,
    /// Combat damage simultaneously assigned to this blocker by other
    /// attackers in the same combat damage step.
    pub damage_assigned_by_other_attackers: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatDamageStep {
    First,
    Second,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstStepAbilitySnapshot {
    Neither,
    FirstStrike,
    DoubleStrike,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WardSource {
    Spell(ObjectId),
    Ability {
        stack_item_id: u64,
        source: SourceProfile,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionInteraction {
    Target,
    Enchant,
    Equip,
    Fortify,
    Damage,
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegenerationChoice {
    OneShotReplacement,
    StaticReplacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceProfile {
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub name: Option<String>,
    pub card_types: BTreeSet<CardType>,
    pub subtypes: BTreeSet<String>,
    pub colors: BTreeSet<ManaColor>,
    pub mana_value: u32,
}

impl SourceProfile {
    pub fn from_object(object: &KeywordObject) -> Self {
        let characteristics = object.effective_characteristics();
        Self {
            owner: object.owner,
            controller: object.controller,
            name: characteristics.name,
            card_types: characteristics.card_types,
            subtypes: characteristics.subtypes,
            colors: characteristics.colors,
            mana_value: characteristics.mana_value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordAction {
    InstallStaticKeyword {
        object: ObjectId,
    },
    InstallTargetingRestriction {
        target: ProtectionTarget,
        chosen_color: Option<ManaColor>,
        chosen_player: Option<PlayerId>,
    },
    InstallWard {
        permanent: ObjectId,
    },
    Mill {
        player: PlayerId,
        amount: u32,
        mode: MillMode,
    },
    CreateRegenerationReplacement {
        permanent: ObjectId,
    },
    InstallProtection {
        target: ProtectionTarget,
        chosen_color: Option<ManaColor>,
        chosen_player: Option<PlayerId>,
    },
    InstallFlying {
        creature: ObjectId,
    },
    Fight {
        first: ObjectId,
        second: ObjectId,
        first_target_status: TargetStatus,
        second_target_status: TargetStatus,
    },
    Investigate {
        player: PlayerId,
    },
    PayKicker {
        player: PlayerId,
        spell: ObjectId,
        cost_index: usize,
        payment: ManaPayment,
    },
    CastWithFlashback {
        player: PlayerId,
        card: ObjectId,
        payment: ManaPayment,
    },
    LeaveStackAfterFlashback {
        card: ObjectId,
        requested_destination: Zone,
    },
    CastFaceDownWithMorph {
        player: PlayerId,
        card: ObjectId,
        can_cast_from_current_zone: bool,
        payment: ManaPayment,
    },
    ResolveFaceDownMorphSpell {
        card: ObjectId,
    },
    TurnMorphFaceUp {
        player: PlayerId,
        permanent: ObjectId,
        payment: ManaPayment,
    },
    PayConvokeCost {
        player: PlayerId,
        spell: ObjectId,
        total_cost: ManaCost,
        convoking_creatures: BTreeMap<usize, Vec<ObjectId>>,
        mana_payment: ManaPayment,
    },
    ActivateEquip {
        player: PlayerId,
        equipment: ObjectId,
        target: ObjectId,
        sorcery_timing_legal: bool,
        payment: ManaPayment,
    },
    ResolveEquip {
        equipment: ObjectId,
    },
    ResolveAuraSpell {
        player: PlayerId,
        aura: ObjectId,
        target: ProtectionTarget,
    },
    CheckAuraState {
        aura: ObjectId,
    },
    EnterSaga {
        saga: ObjectId,
    },
    AdvanceSagaPrecombatMain {
        saga: ObjectId,
        active_player: PlayerId,
    },
    ResolveSagaChapter {
        saga: ObjectId,
        chapter_number: u32,
    },
    CheckSagaSacrifice {
        saga: ObjectId,
    },
    ResolveCumulativeUpkeep {
        permanent: ObjectId,
        player: PlayerId,
        payments: Option<Vec<CumulativeUpkeepPayment>>,
    },
    DeclareAttackerWithVigilance {
        creature: ObjectId,
    },
    AssignTrampleDamage {
        attacker: ObjectId,
        blockers: Vec<TrampleBlockerAssignment>,
        defending_player: PlayerId,
        player_damage: u32,
    },
    RecordDeathtouchDamage {
        source: ObjectId,
        creature: ObjectId,
        damage: u32,
    },
    ApplyLifelink {
        source: ObjectId,
        damage_dealt: u32,
    },
    RecordCombatDamageEligibility {
        creature: ObjectId,
        step: CombatDamageStep,
        first_step_exists: bool,
        first_step_snapshot: FirstStepAbilitySnapshot,
    },
    AttemptIndestructibleDestruction {
        permanent: ObjectId,
    },
    ResolveProwessTrigger {
        creature: ObjectId,
        spell: ObjectId,
    },
    ResolveWard {
        permanent: ObjectId,
        source: WardSource,
        payer: PlayerId,
        payment: Option<ManaPayment>,
    },
    Scry {
        player: PlayerId,
        amount: u32,
        top_order: Vec<ObjectId>,
        bottom_order: Vec<ObjectId>,
    },
    Surveil {
        player: PlayerId,
        amount: u32,
        additional_cards: u32,
        top_order: Vec<ObjectId>,
        graveyard_order: Vec<ObjectId>,
    },
    Cycle {
        player: PlayerId,
        card: ObjectId,
        payment: ManaPayment,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FightNoActionReason {
    ObjectLeftBattlefield,
    ObjectIsNotCreature,
    IllegalTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordEvidenceEvent {
    StaticKeywordInstalled {
        object: ObjectId,
        keyword: OfficialKeyword,
    },
    TargetingRestrictionInstalled {
        target: ProtectionTarget,
        keyword: OfficialKeyword,
        qualities: Vec<ProtectionQuality>,
    },
    WardInstalled {
        permanent: ObjectId,
        cost: ManaCost,
    },
    CardsMilled {
        player: PlayerId,
        requested: u32,
        moved: Vec<ObjectId>,
        library_exhausted: bool,
    },
    RegenerationReplacementCreated {
        permanent: ObjectId,
        replacement: RegenerationReplacement,
        available_replacements: Option<u16>,
    },
    RegenerationReplacedDestruction {
        permanent: ObjectId,
        removed_damage: u32,
        tapped: bool,
        removed_from_combat: bool,
        remaining_one_shot_replacements: u16,
    },
    DestroyedWithoutRegeneration {
        permanent: ObjectId,
    },
    ProtectionInstalled {
        target: ProtectionTarget,
        qualities: Vec<ProtectionQuality>,
    },
    FlyingInstalled {
        creature: ObjectId,
    },
    FightResolved {
        first: ObjectId,
        second: ObjectId,
        first_dealt: u32,
        second_dealt: u32,
    },
    FightDidNotHappen {
        first: ObjectId,
        second: ObjectId,
        reason: FightNoActionReason,
    },
    ClueCreated {
        player: PlayerId,
        token: ObjectId,
    },
    KickerPaid {
        player: PlayerId,
        spell: ObjectId,
        cost_index: usize,
        payment_number: usize,
        mana_spent: Vec<ManaUnitId>,
        life_paid: u32,
    },
    FlashbackCast {
        player: PlayerId,
        card: ObjectId,
        mana_spent: Vec<ManaUnitId>,
        life_paid: u32,
    },
    FlashbackDestinationReplaced {
        card: ObjectId,
        requested_destination: Zone,
        actual_destination: Zone,
    },
    MorphCastFaceDown {
        player: PlayerId,
        card: ObjectId,
        mana_spent: Vec<ManaUnitId>,
        life_paid: u32,
    },
    MorphResolvedFaceDown {
        card: ObjectId,
    },
    MorphTurnedFaceUp {
        player: PlayerId,
        permanent: ObjectId,
        mana_spent: Vec<ManaUnitId>,
        life_paid: u32,
        used_stack: bool,
        caused_enter_trigger: bool,
    },
    ConvokeCostPaid {
        player: PlayerId,
        spell: ObjectId,
        convoking_creatures: Vec<ObjectId>,
        mana_spent: Vec<ManaUnitId>,
        life_paid: u32,
    },
    EquipActivated {
        player: PlayerId,
        equipment: ObjectId,
        target: ObjectId,
        mana_spent: Vec<ManaUnitId>,
        life_paid: u32,
    },
    EquipmentAttached {
        equipment: ObjectId,
        target: ObjectId,
        previous_target: Option<ObjectId>,
    },
    EquipResolutionFailed {
        equipment: ObjectId,
        target: ObjectId,
    },
    AuraAttached {
        aura: ObjectId,
        target: ProtectionTarget,
    },
    AuraMovedToGraveyard {
        aura: ObjectId,
    },
    SagaLoreCountersAdded {
        saga: ObjectId,
        before: u32,
        after: u32,
        triggered_chapters: Vec<u32>,
    },
    SagaChapterLeftStack {
        saga: ObjectId,
        chapter_number: u32,
    },
    SagaSacrificed {
        saga: ObjectId,
    },
    SagaSacrificeDeferred {
        saga: ObjectId,
        pending_chapters: Vec<u32>,
    },
    CumulativeUpkeepAgeCounterAdded {
        permanent: ObjectId,
        age_counters: u32,
    },
    CumulativeUpkeepPaid {
        permanent: ObjectId,
        age_counters: u32,
        mana_spent: Vec<ManaUnitId>,
        life_paid: u32,
    },
    CumulativeUpkeepDeclinedAndSacrificed {
        permanent: ObjectId,
        age_counters: u32,
    },
    AttackerDeclared {
        creature: ObjectId,
        tapped: bool,
    },
    TrampleDamageAssigned {
        attacker: ObjectId,
        blockers: Vec<TrampleBlockerAssignment>,
        defending_player: PlayerId,
        assigned_to_player: u32,
        actual_damage_dealt: u32,
    },
    DeathtouchDamageRecorded {
        source: ObjectId,
        creature: ObjectId,
        damage: u32,
    },
    LifelinkLifeGained {
        source: ObjectId,
        player: PlayerId,
        amount: u32,
    },
    CombatDamageEligibilityRecorded {
        creature: ObjectId,
        step: CombatDamageStep,
        eligible: bool,
    },
    DestructionIgnoredByIndestructible {
        permanent: ObjectId,
    },
    ProwessResolved {
        creature: ObjectId,
        spell: ObjectId,
        power_delta: i32,
        toughness_delta: i32,
    },
    WardPaid {
        permanent: ObjectId,
        payer: PlayerId,
        mana_spent: Vec<ManaUnitId>,
        life_paid: u32,
    },
    WardCounteredSpell {
        permanent: ObjectId,
        spell: ObjectId,
    },
    WardCounteredAbility {
        permanent: ObjectId,
        stack_item_id: u64,
    },
    LibraryReordered {
        player: PlayerId,
        keyword: OfficialKeyword,
        looked: Vec<ObjectId>,
        top_order: Vec<ObjectId>,
        other_destination: Vec<ObjectId>,
        event_occurred: bool,
    },
    Cycled {
        player: PlayerId,
        card: ObjectId,
        drawn: Option<ObjectId>,
        failed_draw: bool,
        mana_spent: Vec<ManaUnitId>,
        life_paid: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeywordReceipt {
    pub evidence_version: &'static str,
    pub runtime_version: &'static str,
    pub rules_effective_date: &'static str,
    pub rules_source_url: &'static str,
    pub keyword: OfficialKeyword,
    pub source: KeywordSourceEvidence,
    pub official_rules: Vec<OfficialRule>,
    pub events: Vec<KeywordEvidenceEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionalRollbackContract {
    RestoreCompleteStateOnEveryError,
}

pub const KEYWORD_TRANSACTION_CONTRACT: TransactionalRollbackContract =
    TransactionalRollbackContract::RestoreCompleteStateOnEveryError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatureDestructionStateBasedOutcome {
    NoDestruction,
    Destroyed,
    Regenerated,
    IgnoredByIndestructible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeywordExecutionError {
    ProgramVersionMismatch,
    ActionProgramMismatch,
    MissingPlayer,
    DuplicatePlayer,
    MissingObject(ObjectId),
    DuplicateObject,
    InconsistentZoneMembership {
        object: ObjectId,
        zone: Zone,
    },
    WrongZone {
        object: ObjectId,
        expected: Zone,
        actual: Zone,
    },
    WrongController,
    WrongOwner,
    NotCreature(ObjectId),
    NotInstantOrSorcery(ObjectId),
    LibraryInvariant(ObjectId),
    MillUnavailable {
        requested: u32,
        available: usize,
        mode: MillMode,
    },
    MissingProtectionChoice,
    UnexpectedProtectionChoice,
    MissingManaPayment {
        symbol_index: usize,
    },
    UnexpectedManaPayment {
        symbol_index: usize,
    },
    DuplicateManaUnit(ManaUnitId),
    MissingManaUnit(ManaUnitId),
    InvalidManaPayment {
        symbol_index: usize,
    },
    MissingVariableValue,
    UnexpectedVariableValue,
    InsufficientLife,
    KickerCostOutOfRange,
    KickerCostAlreadyPaid,
    CastPermissionMissing,
    NotFaceDownMorph,
    InvalidStackDestination,
    ObjectIdOverflow,
    InvalidRegenerationChoice,
    InvalidTiming,
    InvalidConvokeTotalCost,
    InvalidConvokeCreature(ObjectId),
    DuplicateConvokeCreature(ObjectId),
    PendingEquipAlreadyExists,
    MissingPendingEquip,
    IllegalAttachment,
    MissingSagaChapter,
    InvalidSagaState,
    InvalidCumulativePaymentCount {
        expected: usize,
        actual: usize,
    },
    CumulativeAlternativeOutOfRange,
    KeywordNotInstalled {
        object: ObjectId,
        keyword: OfficialKeyword,
    },
    InvalidCombatParticipant(ObjectId),
    IllegalCombatAssignment,
    InvalidDamageSource,
    TargetingRestrictionMismatch,
    WardDidNotTrigger,
    WardCostNotInstalled,
    InvalidWardSource,
    InvalidLibraryDecision,
    InvalidProwessTrigger,
    IndestructibleRequiresOwnContract,
}

impl fmt::Display for KeywordExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for KeywordExecutionError {}

pub fn execute_keyword_action(
    state: &mut KeywordGameState,
    program: &KeywordProgram,
    action: KeywordAction,
) -> Result<KeywordReceipt, KeywordExecutionError> {
    if program.runtime_version != KEYWORD_RULES_RUNTIME_VERSION || !program.has_exact_contract() {
        return Err(KeywordExecutionError::ProgramVersionMismatch);
    }
    let before = state.clone();
    match execute_keyword_action_inner(state, program, action) {
        Ok(events) => Ok(receipt(program, events)),
        Err(error) => {
            *state = before;
            Err(error)
        }
    }
}

fn receipt(program: &KeywordProgram, events: Vec<KeywordEvidenceEvent>) -> KeywordReceipt {
    KeywordReceipt {
        evidence_version: KEYWORD_RULES_EVIDENCE_VERSION,
        runtime_version: KEYWORD_RULES_RUNTIME_VERSION,
        rules_effective_date: KEYWORD_RULES_EFFECTIVE_DATE,
        rules_source_url: KEYWORD_RULES_SOURCE_URL,
        keyword: program.keyword(),
        source: program.source.clone(),
        official_rules: program.official_rules().to_vec(),
        events,
    }
}

fn execute_keyword_action_inner(
    state: &mut KeywordGameState,
    program: &KeywordProgram,
    action: KeywordAction,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    match (&program.kind, action) {
        (
            KeywordProgramKind::Flash
            | KeywordProgramKind::Menace
            | KeywordProgramKind::Defender
            | KeywordProgramKind::Reach
            | KeywordProgramKind::Fear(_)
            | KeywordProgramKind::Shadow(_)
            | KeywordProgramKind::Landwalk(_)
            | KeywordProgramKind::Devoid
            | KeywordProgramKind::Haste
            | KeywordProgramKind::Vigilance
            | KeywordProgramKind::Trample
            | KeywordProgramKind::Deathtouch
            | KeywordProgramKind::Lifelink
            | KeywordProgramKind::FirstStrike
            | KeywordProgramKind::DoubleStrike
            | KeywordProgramKind::Indestructible
            | KeywordProgramKind::Prowess,
            KeywordAction::InstallStaticKeyword { object },
        ) => execute_install_static_keyword(state, object, program),
        (
            KeywordProgramKind::Hexproof(hexproof),
            KeywordAction::InstallTargetingRestriction {
                target,
                chosen_color,
                chosen_player,
            },
        ) => execute_install_targeting_restriction(
            state,
            target,
            OfficialKeyword::Hexproof,
            hexproof.qualities.as_deref(),
            chosen_color,
            chosen_player,
        ),
        (
            KeywordProgramKind::Shroud,
            KeywordAction::InstallTargetingRestriction {
                target,
                chosen_color,
                chosen_player,
            },
        ) => execute_install_targeting_restriction(
            state,
            target,
            OfficialKeyword::Shroud,
            None,
            chosen_color,
            chosen_player,
        ),
        (KeywordProgramKind::Ward(ward), KeywordAction::InstallWard { permanent }) => {
            execute_install_ward(state, permanent, ward)
        }
        (
            KeywordProgramKind::Mill,
            KeywordAction::Mill {
                player,
                amount,
                mode,
            },
        ) => execute_mill(state, player, amount, mode),
        (
            KeywordProgramKind::Regenerate(program),
            KeywordAction::CreateRegenerationReplacement { permanent },
        ) if program.recipients == RegenerationRecipientScope::SourcePermanent => {
            execute_regenerate(state, permanent, program.replacement)
        }
        (
            KeywordProgramKind::Protection(protection),
            KeywordAction::InstallProtection {
                target,
                chosen_color,
                chosen_player,
            },
        ) => execute_install_protection(state, target, protection, chosen_color, chosen_player),
        (KeywordProgramKind::Flying, KeywordAction::InstallFlying { creature }) => {
            execute_install_flying(state, creature)
        }
        (
            KeywordProgramKind::Fight,
            KeywordAction::Fight {
                first,
                second,
                first_target_status,
                second_target_status,
            },
        ) => execute_fight(
            state,
            first,
            second,
            first_target_status,
            second_target_status,
        ),
        (KeywordProgramKind::Investigate, KeywordAction::Investigate { player }) => {
            execute_investigate(state, player)
        }
        (
            KeywordProgramKind::Kicker(kicker),
            KeywordAction::PayKicker {
                player,
                spell,
                cost_index,
                payment,
            },
        ) => execute_kicker(state, player, spell, kicker, cost_index, &payment),
        (
            KeywordProgramKind::Flashback(flashback),
            KeywordAction::CastWithFlashback {
                player,
                card,
                payment,
            },
        ) => execute_flashback_cast(state, player, card, flashback, &payment),
        (
            KeywordProgramKind::Flashback(flashback),
            KeywordAction::LeaveStackAfterFlashback {
                card,
                requested_destination,
            },
        ) => execute_flashback_stack_exit(state, card, requested_destination, flashback),
        (
            KeywordProgramKind::Morph(morph),
            KeywordAction::CastFaceDownWithMorph {
                player,
                card,
                can_cast_from_current_zone,
                payment,
            },
        ) => execute_morph_cast(
            state,
            player,
            card,
            can_cast_from_current_zone,
            morph,
            &payment,
        ),
        (KeywordProgramKind::Morph(_), KeywordAction::ResolveFaceDownMorphSpell { card }) => {
            execute_morph_resolution(state, card)
        }
        (
            KeywordProgramKind::Morph(morph),
            KeywordAction::TurnMorphFaceUp {
                player,
                permanent,
                payment,
            },
        ) => execute_turn_morph_face_up(state, player, permanent, morph, &payment),
        (
            KeywordProgramKind::Convoke(_),
            KeywordAction::PayConvokeCost {
                player,
                spell,
                total_cost,
                convoking_creatures,
                mana_payment,
            },
        ) => execute_convoke(
            state,
            player,
            spell,
            &total_cost,
            &convoking_creatures,
            &mana_payment,
        ),
        (
            KeywordProgramKind::Equip(equip),
            KeywordAction::ActivateEquip {
                player,
                equipment,
                target,
                sorcery_timing_legal,
                payment,
            },
        ) => execute_equip_activation(
            state,
            player,
            equipment,
            target,
            sorcery_timing_legal,
            equip,
            &payment,
        ),
        (KeywordProgramKind::Equip(equip), KeywordAction::ResolveEquip { equipment }) => {
            execute_equip_resolution(state, equipment, equip)
        }
        (
            KeywordProgramKind::Enchant(enchant),
            KeywordAction::ResolveAuraSpell {
                player,
                aura,
                target,
            },
        ) => execute_aura_resolution(state, player, aura, target, enchant),
        (KeywordProgramKind::Enchant(enchant), KeywordAction::CheckAuraState { aura }) => {
            execute_aura_state_check(state, aura, enchant)
        }
        (KeywordProgramKind::Saga(saga), KeywordAction::EnterSaga { saga: permanent }) => {
            execute_saga_entry(state, permanent, saga)
        }
        (
            KeywordProgramKind::Saga(saga),
            KeywordAction::AdvanceSagaPrecombatMain {
                saga: permanent,
                active_player,
            },
        ) => execute_saga_advance(state, permanent, active_player, saga),
        (
            KeywordProgramKind::Saga(_),
            KeywordAction::ResolveSagaChapter {
                saga,
                chapter_number,
            },
        ) => execute_saga_chapter_exit(state, saga, chapter_number),
        (KeywordProgramKind::Saga(saga), KeywordAction::CheckSagaSacrifice { saga: permanent }) => {
            execute_saga_sacrifice_check(state, permanent, saga)
        }
        (
            KeywordProgramKind::CumulativeUpkeep(upkeep),
            KeywordAction::ResolveCumulativeUpkeep {
                permanent,
                player,
                payments,
            },
        ) => execute_cumulative_upkeep(state, permanent, player, upkeep, payments.as_deref()),
        (
            KeywordProgramKind::Vigilance,
            KeywordAction::DeclareAttackerWithVigilance { creature },
        ) => execute_declare_attacker_with_vigilance(state, creature),
        (
            KeywordProgramKind::Trample,
            KeywordAction::AssignTrampleDamage {
                attacker,
                blockers,
                defending_player,
                player_damage,
            },
        ) => execute_assign_trample_damage(
            state,
            attacker,
            &blockers,
            defending_player,
            player_damage,
        ),
        (
            KeywordProgramKind::Deathtouch,
            KeywordAction::RecordDeathtouchDamage {
                source,
                creature,
                damage,
            },
        ) => execute_record_deathtouch_damage(state, source, creature, damage),
        (
            KeywordProgramKind::Lifelink,
            KeywordAction::ApplyLifelink {
                source,
                damage_dealt,
            },
        ) => execute_apply_lifelink(state, source, damage_dealt),
        (
            KeywordProgramKind::FirstStrike | KeywordProgramKind::DoubleStrike,
            KeywordAction::RecordCombatDamageEligibility {
                creature,
                step,
                first_step_exists,
                first_step_snapshot,
            },
        ) => execute_record_combat_damage_eligibility(
            state,
            creature,
            step,
            first_step_exists,
            first_step_snapshot,
        ),
        (
            KeywordProgramKind::Indestructible,
            KeywordAction::AttemptIndestructibleDestruction { permanent },
        ) => execute_attempt_indestructible_destruction(state, permanent),
        (KeywordProgramKind::Prowess, KeywordAction::ResolveProwessTrigger { creature, spell }) => {
            execute_resolve_prowess_trigger(state, creature, spell)
        }
        (
            KeywordProgramKind::Ward(ward),
            KeywordAction::ResolveWard {
                permanent,
                source,
                payer,
                payment,
            },
        ) => execute_resolve_ward(state, permanent, source, payer, payment.as_ref(), ward),
        (
            KeywordProgramKind::Scry,
            KeywordAction::Scry {
                player,
                amount,
                top_order,
                bottom_order,
            },
        ) => execute_scry(state, player, amount, &top_order, &bottom_order),
        (
            KeywordProgramKind::Surveil,
            KeywordAction::Surveil {
                player,
                amount,
                additional_cards,
                top_order,
                graveyard_order,
            },
        ) => execute_surveil(
            state,
            player,
            amount,
            additional_cards,
            &top_order,
            &graveyard_order,
        ),
        (
            KeywordProgramKind::Cycling(cycling),
            KeywordAction::Cycle {
                player,
                card,
                payment,
            },
        ) => execute_cycle(state, player, card, &payment, cycling),
        _ => Err(KeywordExecutionError::ActionProgramMismatch),
    }
}

fn execute_mill(
    state: &mut KeywordGameState,
    player: PlayerId,
    amount: u32,
    mode: MillMode,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let available = state
        .players
        .get(&player)
        .ok_or(KeywordExecutionError::MissingPlayer)?
        .library
        .len();
    if usize::try_from(amount).unwrap_or(usize::MAX) > available && mode != MillMode::Instruction {
        return Err(KeywordExecutionError::MillUnavailable {
            requested: amount,
            available,
            mode,
        });
    }
    let move_count = available.min(usize::try_from(amount).unwrap_or(usize::MAX));
    let moved = state
        .players
        .get(&player)
        .ok_or(KeywordExecutionError::MissingPlayer)?
        .library
        .iter()
        .take(move_count)
        .copied()
        .collect::<Vec<_>>();
    for object_id in &moved {
        let object = state.object(*object_id)?;
        if object.owner != player || object.zone != Zone::Library {
            return Err(KeywordExecutionError::LibraryInvariant(*object_id));
        }
    }
    for object_id in &moved {
        state.move_object(*object_id, Zone::Graveyard)?;
    }
    Ok(vec![KeywordEvidenceEvent::CardsMilled {
        player,
        requested: amount,
        moved,
        library_exhausted: move_count < usize::try_from(amount).unwrap_or(usize::MAX),
    }])
}

fn execute_regenerate(
    state: &mut KeywordGameState,
    permanent: ObjectId,
    replacement: RegenerationReplacement,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object_mut(permanent)?;
    if object.zone != Zone::Battlefield {
        return Err(KeywordExecutionError::WrongZone {
            object: permanent,
            expected: Zone::Battlefield,
            actual: object.zone,
        });
    }
    let available_replacements = match replacement {
        RegenerationReplacement::NextDestructionThisTurn => {
            object.regeneration_shields = object.regeneration_shields.saturating_add(1);
            Some(object.regeneration_shields)
        }
        RegenerationReplacement::EveryDestructionWhileStaticEffectApplies => {
            object.static_regeneration = true;
            None
        }
    };
    Ok(vec![KeywordEvidenceEvent::RegenerationReplacementCreated {
        permanent,
        replacement,
        available_replacements,
    }])
}

fn execute_install_protection(
    state: &mut KeywordGameState,
    target: ProtectionTarget,
    program: &ProtectionProgram,
    chosen_color: Option<ManaColor>,
    chosen_player: Option<PlayerId>,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let qualities = resolve_quality_specs(state, &program.qualities, chosen_color, chosen_player)?;

    match target {
        ProtectionTarget::Object(object_id) => {
            let object = state.object_mut(object_id)?;
            if object.zone != Zone::Battlefield {
                return Err(KeywordExecutionError::WrongZone {
                    object: object_id,
                    expected: Zone::Battlefield,
                    actual: object.zone,
                });
            }
            for quality in &qualities {
                if !object.protections.contains(quality) {
                    object.protections.push(quality.clone());
                }
            }
        }
        ProtectionTarget::Player(player) => {
            let player = state
                .players
                .get_mut(&player)
                .ok_or(KeywordExecutionError::MissingPlayer)?;
            for quality in &qualities {
                if !player.protections.contains(quality) {
                    player.protections.push(quality.clone());
                }
            }
        }
    }
    Ok(vec![KeywordEvidenceEvent::ProtectionInstalled {
        target,
        qualities,
    }])
}

fn resolve_quality_specs(
    state: &KeywordGameState,
    specs: &[ProtectionQualitySpec],
    chosen_color: Option<ManaColor>,
    chosen_player: Option<PlayerId>,
) -> Result<Vec<ProtectionQuality>, KeywordExecutionError> {
    let expects_color = specs.contains(&ProtectionQualitySpec::ChosenColor);
    let expects_player = specs.contains(&ProtectionQualitySpec::ChosenPlayer);
    if (expects_color && chosen_color.is_none()) || (expects_player && chosen_player.is_none()) {
        return Err(KeywordExecutionError::MissingProtectionChoice);
    }
    if (!expects_color && chosen_color.is_some()) || (!expects_player && chosen_player.is_some()) {
        return Err(KeywordExecutionError::UnexpectedProtectionChoice);
    }
    if let Some(player) = chosen_player
        && !state.players.contains_key(&player)
    {
        return Err(KeywordExecutionError::MissingPlayer);
    }

    let mut qualities = Vec::new();
    for quality in specs {
        match quality {
            ProtectionQualitySpec::Everything => {
                qualities.push(ProtectionQuality::Everything);
            }
            ProtectionQualitySpec::Color(color) => {
                qualities.push(ProtectionQuality::Color(*color));
            }
            ProtectionQualitySpec::EachColor => {
                qualities.extend([
                    ProtectionQuality::Color(ManaColor::White),
                    ProtectionQuality::Color(ManaColor::Blue),
                    ProtectionQuality::Color(ManaColor::Black),
                    ProtectionQuality::Color(ManaColor::Red),
                    ProtectionQuality::Color(ManaColor::Green),
                ]);
            }
            ProtectionQualitySpec::ChosenColor => {
                qualities.push(ProtectionQuality::Color(
                    chosen_color.ok_or(KeywordExecutionError::MissingProtectionChoice)?,
                ));
            }
            ProtectionQualitySpec::Colored => qualities.push(ProtectionQuality::Colored),
            ProtectionQualitySpec::Colorless => qualities.push(ProtectionQuality::Colorless),
            ProtectionQualitySpec::Monocolored => qualities.push(ProtectionQuality::Monocolored),
            ProtectionQualitySpec::Multicolored => qualities.push(ProtectionQuality::Multicolored),
            ProtectionQualitySpec::CardType(card_type) => {
                qualities.push(ProtectionQuality::CardType(card_type.clone()));
            }
            ProtectionQualitySpec::Subtype(subtype) => {
                qualities.push(ProtectionQuality::Subtype(subtype.clone()));
            }
            ProtectionQualitySpec::Named(name) => {
                qualities.push(ProtectionQuality::Named(name.clone()));
            }
            ProtectionQualitySpec::ChosenPlayer => {
                qualities.push(ProtectionQuality::Player(
                    chosen_player.ok_or(KeywordExecutionError::MissingProtectionChoice)?,
                ));
            }
            ProtectionQualitySpec::ManaValueAtMost(value) => {
                qualities.push(ProtectionQuality::ManaValueAtMost(*value));
            }
            ProtectionQualitySpec::ManaValueAtLeast(value) => {
                qualities.push(ProtectionQuality::ManaValueAtLeast(*value));
            }
        }
    }
    Ok(qualities)
}

fn execute_install_targeting_restriction(
    state: &mut KeywordGameState,
    target: ProtectionTarget,
    keyword: OfficialKeyword,
    qualities: Option<&[ProtectionQualitySpec]>,
    chosen_color: Option<ManaColor>,
    chosen_player: Option<PlayerId>,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let qualities = resolve_quality_specs(
        state,
        qualities.unwrap_or_default(),
        chosen_color,
        chosen_player,
    )?;
    match target {
        ProtectionTarget::Object(object_id) => {
            let object = state.object_mut(object_id)?;
            if object.zone != Zone::Battlefield {
                return Err(KeywordExecutionError::WrongZone {
                    object: object_id,
                    expected: Zone::Battlefield,
                    actual: object.zone,
                });
            }
            match keyword {
                OfficialKeyword::Hexproof if qualities.is_empty() => {
                    object.has_hexproof = true;
                }
                OfficialKeyword::Hexproof => {
                    for quality in &qualities {
                        if !object.hexproof_qualities.contains(quality) {
                            object.hexproof_qualities.push(quality.clone());
                        }
                    }
                }
                OfficialKeyword::Shroud => {
                    object.has_shroud = true;
                }
                _ => return Err(KeywordExecutionError::TargetingRestrictionMismatch),
            }
            object.rules_keywords.insert(keyword);
            increment_keyword_instance(object, keyword);
        }
        ProtectionTarget::Player(player_id) => {
            let player = state
                .players
                .get_mut(&player_id)
                .ok_or(KeywordExecutionError::MissingPlayer)?;
            match keyword {
                OfficialKeyword::Hexproof if qualities.is_empty() => {
                    player.has_hexproof = true;
                }
                OfficialKeyword::Hexproof => {
                    for quality in &qualities {
                        if !player.hexproof_qualities.contains(quality) {
                            player.hexproof_qualities.push(quality.clone());
                        }
                    }
                }
                OfficialKeyword::Shroud => {
                    player.has_shroud = true;
                }
                _ => return Err(KeywordExecutionError::TargetingRestrictionMismatch),
            }
        }
    }
    Ok(vec![KeywordEvidenceEvent::TargetingRestrictionInstalled {
        target,
        keyword,
        qualities,
    }])
}

fn execute_install_flying(
    state: &mut KeywordGameState,
    creature: ObjectId,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object_mut(creature)?;
    if object.zone != Zone::Battlefield {
        return Err(KeywordExecutionError::WrongZone {
            object: creature,
            expected: Zone::Battlefield,
            actual: object.zone,
        });
    }
    if !object.is_creature() {
        return Err(KeywordExecutionError::NotCreature(creature));
    }
    object.combat_keywords.insert(CombatKeyword::Flying);
    object.rules_keywords.insert(OfficialKeyword::Flying);
    increment_keyword_instance(object, OfficialKeyword::Flying);
    Ok(vec![KeywordEvidenceEvent::FlyingInstalled { creature }])
}

fn execute_fight(
    state: &mut KeywordGameState,
    first: ObjectId,
    second: ObjectId,
    first_target_status: TargetStatus,
    second_target_status: TargetStatus,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let first_object = state.object(first)?.clone();
    let second_object = state.object(second)?.clone();
    if first_object.zone != Zone::Battlefield || second_object.zone != Zone::Battlefield {
        return Ok(vec![KeywordEvidenceEvent::FightDidNotHappen {
            first,
            second,
            reason: FightNoActionReason::ObjectLeftBattlefield,
        }]);
    }
    if !first_object.is_creature() || !second_object.is_creature() {
        return Ok(vec![KeywordEvidenceEvent::FightDidNotHappen {
            first,
            second,
            reason: FightNoActionReason::ObjectIsNotCreature,
        }]);
    }
    if first_target_status == TargetStatus::IllegalTarget
        || second_target_status == TargetStatus::IllegalTarget
    {
        return Ok(vec![KeywordEvidenceEvent::FightDidNotHappen {
            first,
            second,
            reason: FightNoActionReason::IllegalTarget,
        }]);
    }

    let first_power = first_object
        .effective_characteristics()
        .power
        .unwrap_or(0)
        .max(0) as u32;
    let second_power = second_object
        .effective_characteristics()
        .power
        .unwrap_or(0)
        .max(0) as u32;
    let first_source = SourceProfile::from_object(&first_object);
    let second_source = SourceProfile::from_object(&second_object);
    let first_dealt = if protection_forbids(
        state,
        ProtectionTarget::Object(second),
        &first_source,
        ProtectionInteraction::Damage,
    )? {
        0
    } else {
        first_power
    };
    let second_dealt = if protection_forbids(
        state,
        ProtectionTarget::Object(first),
        &second_source,
        ProtectionInteraction::Damage,
    )? {
        0
    } else {
        second_power
    };
    state.object_mut(first)?.damage_marked = state
        .object(first)?
        .damage_marked
        .saturating_add(second_dealt);
    state.object_mut(second)?.damage_marked = state
        .object(second)?
        .damage_marked
        .saturating_add(first_dealt);
    Ok(vec![KeywordEvidenceEvent::FightResolved {
        first,
        second,
        first_dealt,
        second_dealt,
    }])
}

fn execute_investigate(
    state: &mut KeywordGameState,
    player: PlayerId,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    if !state.players.contains_key(&player) {
        return Err(KeywordExecutionError::MissingPlayer);
    }
    let token = ObjectId(state.next_object_id);
    state.next_object_id = state
        .next_object_id
        .checked_add(1)
        .ok_or(KeywordExecutionError::ObjectIdOverflow)?;
    let mut card_types = BTreeSet::new();
    card_types.insert(CardType::Artifact);
    let mut subtypes = BTreeSet::new();
    subtypes.insert("Clue".to_owned());
    let mut clue = KeywordObject::new(
        token,
        player,
        player,
        Zone::Battlefield,
        ObjectCharacteristics {
            name: Some("Clue".into()),
            card_types,
            supertypes: BTreeSet::new(),
            subtypes,
            colors: BTreeSet::new(),
            mana_value: 0,
            power: None,
            toughness: None,
            oracle_text: Some("{2}, Sacrifice this token: Draw a card.".into()),
        },
    );
    clue.is_token = true;
    state.insert_object(clue)?;
    Ok(vec![KeywordEvidenceEvent::ClueCreated { player, token }])
}

fn execute_kicker(
    state: &mut KeywordGameState,
    player: PlayerId,
    spell: ObjectId,
    program: &KickerProgram,
    cost_index: usize,
    payment: &ManaPayment,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let cost = program
        .costs
        .get(cost_index)
        .ok_or(KeywordExecutionError::KickerCostOutOfRange)?
        .clone();
    {
        let spell_object = state.object(spell)?;
        if spell_object.zone != Zone::Stack {
            return Err(KeywordExecutionError::WrongZone {
                object: spell,
                expected: Zone::Stack,
                actual: spell_object.zone,
            });
        }
        if spell_object.controller != player {
            return Err(KeywordExecutionError::WrongController);
        }
        if program.multiplicity == KickerMultiplicity::OncePerCost
            && spell_object.kicker_payments.contains(&cost_index)
        {
            return Err(KeywordExecutionError::KickerCostAlreadyPaid);
        }
        if program.multiplicity == KickerMultiplicity::AnyNumberOfTimes && cost_index != 0 {
            return Err(KeywordExecutionError::KickerCostOutOfRange);
        }
    }
    let payment_evidence = pay_mana_cost(state, player, &cost, payment)?;
    let spell_object = state.object_mut(spell)?;
    spell_object.kicker_payments.push(cost_index);
    let payment_number = spell_object.kicker_payments.len();
    Ok(vec![KeywordEvidenceEvent::KickerPaid {
        player,
        spell,
        cost_index,
        payment_number,
        mana_spent: payment_evidence.mana_spent,
        life_paid: payment_evidence.life_paid,
    }])
}

fn execute_flashback_cast(
    state: &mut KeywordGameState,
    player: PlayerId,
    card: ObjectId,
    program: &FlashbackProgram,
    payment: &ManaPayment,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let card_object = state.object(card)?;
    if card_object.zone != Zone::Graveyard {
        return Err(KeywordExecutionError::WrongZone {
            object: card,
            expected: Zone::Graveyard,
            actual: card_object.zone,
        });
    }
    if card_object.owner != player {
        return Err(KeywordExecutionError::WrongOwner);
    }
    let types = &card_object.printed.card_types;
    if !types.contains(&CardType::Instant) && !types.contains(&CardType::Sorcery) {
        return Err(KeywordExecutionError::NotInstantOrSorcery(card));
    }
    let payment_evidence = pay_mana_cost(state, player, &program.alternative_cost, payment)?;
    state.move_object(card, Zone::Stack)?;
    let object = state.object_mut(card)?;
    object.controller = player;
    object.cast_method = Some(CastMethod::Flashback);
    Ok(vec![KeywordEvidenceEvent::FlashbackCast {
        player,
        card,
        mana_spent: payment_evidence.mana_spent,
        life_paid: payment_evidence.life_paid,
    }])
}

fn execute_flashback_stack_exit(
    state: &mut KeywordGameState,
    card: ObjectId,
    requested_destination: Zone,
    program: &FlashbackProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    if requested_destination == Zone::Stack {
        return Err(KeywordExecutionError::InvalidStackDestination);
    }
    let object = state.object(card)?;
    if object.zone != Zone::Stack {
        return Err(KeywordExecutionError::WrongZone {
            object: card,
            expected: Zone::Stack,
            actual: object.zone,
        });
    }
    if object.cast_method != Some(CastMethod::Flashback) {
        return Err(KeywordExecutionError::ActionProgramMismatch);
    }
    let actual_destination = if program.exile_replaces_every_stack_destination {
        Zone::Exile
    } else {
        requested_destination
    };
    state.move_object(card, actual_destination)?;
    state.object_mut(card)?.cast_method = None;
    Ok(vec![KeywordEvidenceEvent::FlashbackDestinationReplaced {
        card,
        requested_destination,
        actual_destination,
    }])
}

fn execute_morph_cast(
    state: &mut KeywordGameState,
    player: PlayerId,
    card: ObjectId,
    can_cast_from_current_zone: bool,
    program: &MorphProgram,
    payment: &ManaPayment,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    if !can_cast_from_current_zone {
        return Err(KeywordExecutionError::CastPermissionMissing);
    }
    let object = state.object(card)?;
    if matches!(object.zone, Zone::Battlefield | Zone::Stack) {
        return Err(KeywordExecutionError::CastPermissionMissing);
    }
    let payment_evidence = pay_mana_cost(state, player, &program.face_down_cast_cost, payment)?;
    state.move_object(card, Zone::Stack)?;
    let object = state.object_mut(card)?;
    object.controller = player;
    object.face_down = true;
    object.cast_method = Some(CastMethod::MorphFaceDown);
    object.morph_face_up_cost = Some(program.face_up_cost.clone());
    Ok(vec![KeywordEvidenceEvent::MorphCastFaceDown {
        player,
        card,
        mana_spent: payment_evidence.mana_spent,
        life_paid: payment_evidence.life_paid,
    }])
}

fn execute_morph_resolution(
    state: &mut KeywordGameState,
    card: ObjectId,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object(card)?;
    if object.zone != Zone::Stack {
        return Err(KeywordExecutionError::WrongZone {
            object: card,
            expected: Zone::Stack,
            actual: object.zone,
        });
    }
    if object.cast_method != Some(CastMethod::MorphFaceDown) || !object.face_down {
        return Err(KeywordExecutionError::NotFaceDownMorph);
    }
    state.move_object(card, Zone::Battlefield)?;
    state.object_mut(card)?.cast_method = None;
    Ok(vec![KeywordEvidenceEvent::MorphResolvedFaceDown { card }])
}

fn execute_turn_morph_face_up(
    state: &mut KeywordGameState,
    player: PlayerId,
    permanent: ObjectId,
    program: &MorphProgram,
    payment: &ManaPayment,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object(permanent)?;
    if object.zone != Zone::Battlefield {
        return Err(KeywordExecutionError::WrongZone {
            object: permanent,
            expected: Zone::Battlefield,
            actual: object.zone,
        });
    }
    if object.controller != player {
        return Err(KeywordExecutionError::WrongController);
    }
    if !object.face_down || object.morph_face_up_cost.as_ref() != Some(&program.face_up_cost) {
        return Err(KeywordExecutionError::NotFaceDownMorph);
    }
    let payment_evidence = pay_mana_cost(state, player, &program.face_up_cost, payment)?;
    state.object_mut(permanent)?.face_down = false;
    Ok(vec![KeywordEvidenceEvent::MorphTurnedFaceUp {
        player,
        permanent,
        mana_spent: payment_evidence.mana_spent,
        life_paid: payment_evidence.life_paid,
        used_stack: false,
        caused_enter_trigger: false,
    }])
}

fn execute_install_static_keyword(
    state: &mut KeywordGameState,
    object: ObjectId,
    program: &KeywordProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let keyword = program.keyword();
    if matches!(
        program.kind(),
        KeywordProgramKind::Fear(_)
            | KeywordProgramKind::Shadow(_)
            | KeywordProgramKind::Landwalk(_)
    ) {
        let object_state = state.object(object)?;
        if object_state.zone != Zone::Battlefield {
            return Err(KeywordExecutionError::WrongZone {
                object,
                expected: Zone::Battlefield,
                actual: object_state.zone,
            });
        }
        if !object_state.is_creature() {
            return Err(KeywordExecutionError::NotCreature(object));
        }
    }
    let object_state = state.object_mut(object)?;
    let combat_keyword = match program.kind() {
        KeywordProgramKind::Fear(FearProgram {
            artifact_or_black_blockers_only: true,
        }) => Some(CombatKeyword::Fear),
        KeywordProgramKind::Shadow(ShadowProgram {
            requires_matching_shadow_status: true,
        }) => Some(CombatKeyword::Shadow),
        KeywordProgramKind::Landwalk(LandwalkProgram {
            quality,
            checks_defending_player: true,
            same_kind_instances_are_redundant: true,
        }) => {
            let instances = object_state.landwalk_instances.entry(*quality).or_default();
            *instances = instances.saturating_add(1);
            None
        }
        KeywordProgramKind::Fear(_)
        | KeywordProgramKind::Shadow(_)
        | KeywordProgramKind::Landwalk(_) => {
            return Err(KeywordExecutionError::ActionProgramMismatch);
        }
        KeywordProgramKind::Menace => Some(CombatKeyword::Menace),
        KeywordProgramKind::Defender => Some(CombatKeyword::Defender),
        KeywordProgramKind::Reach => Some(CombatKeyword::Reach),
        KeywordProgramKind::Haste => Some(CombatKeyword::Haste),
        KeywordProgramKind::Vigilance => Some(CombatKeyword::Vigilance),
        KeywordProgramKind::Trample => Some(CombatKeyword::Trample),
        KeywordProgramKind::Deathtouch => Some(CombatKeyword::Deathtouch),
        KeywordProgramKind::Lifelink => Some(CombatKeyword::Lifelink),
        KeywordProgramKind::FirstStrike => Some(CombatKeyword::FirstStrike),
        KeywordProgramKind::DoubleStrike => Some(CombatKeyword::DoubleStrike),
        KeywordProgramKind::Indestructible => Some(CombatKeyword::Indestructible),
        KeywordProgramKind::Prowess => Some(CombatKeyword::Prowess),
        KeywordProgramKind::Flash | KeywordProgramKind::Devoid => None,
        _ => return Err(KeywordExecutionError::ActionProgramMismatch),
    };
    object_state.rules_keywords.insert(keyword);
    increment_keyword_instance(object_state, keyword);
    if let Some(combat_keyword) = combat_keyword {
        object_state.combat_keywords.insert(combat_keyword);
    }
    Ok(vec![KeywordEvidenceEvent::StaticKeywordInstalled {
        object,
        keyword,
    }])
}

fn increment_keyword_instance(object: &mut KeywordObject, keyword: OfficialKeyword) {
    let instances = object.keyword_instances.entry(keyword).or_default();
    *instances = instances.saturating_add(1);
}

pub fn can_cast_at_instant_timing(
    state: &KeywordGameState,
    object: ObjectId,
    can_play_from_current_zone: bool,
) -> Result<bool, KeywordExecutionError> {
    Ok(can_play_from_current_zone
        && state
            .object(object)?
            .rules_keywords
            .contains(&OfficialKeyword::Flash))
}

pub fn can_attack(
    state: &KeywordGameState,
    creature: ObjectId,
) -> Result<bool, KeywordExecutionError> {
    let object = state.object(creature)?;
    Ok(object.zone == Zone::Battlefield
        && object.is_creature()
        && !object.tapped
        && !object.combat_keywords.contains(&CombatKeyword::Defender)
        && (object.controlled_since_turn_began
            || object.combat_keywords.contains(&CombatKeyword::Haste)))
}

pub fn can_activate_tap_or_untap_symbol(
    state: &KeywordGameState,
    permanent: ObjectId,
) -> Result<bool, KeywordExecutionError> {
    let object = state.object(permanent)?;
    Ok(object.zone == Zone::Battlefield
        && (!object.is_creature()
            || object.controlled_since_turn_began
            || object.combat_keywords.contains(&CombatKeyword::Haste)))
}

fn require_keyword(
    object: &KeywordObject,
    keyword: OfficialKeyword,
) -> Result<(), KeywordExecutionError> {
    if object.rules_keywords.contains(&keyword) {
        Ok(())
    } else {
        Err(KeywordExecutionError::KeywordNotInstalled {
            object: object.id,
            keyword,
        })
    }
}

pub fn block_assignment_is_legal(
    state: &KeywordGameState,
    attacker: ObjectId,
    blockers: &[ObjectId],
) -> Result<bool, KeywordExecutionError> {
    let attacker_object = state.object(attacker)?;
    let defending_player = blockers
        .first()
        .map(|blocker| state.object(*blocker).map(|object| object.controller))
        .transpose()?;
    let mut unique = BTreeSet::new();
    for blocker in blockers {
        if !unique.insert(*blocker)
            || !can_block_for_defending_player(
                state,
                attacker,
                *blocker,
                defending_player.expect("a blocker establishes the defending player"),
            )?
        {
            return Ok(false);
        }
    }
    if attacker_object
        .combat_keywords
        .contains(&CombatKeyword::Menace)
        && !blockers.is_empty()
        && blockers.len() < 2
    {
        return Ok(false);
    }
    Ok(true)
}

fn execute_declare_attacker_with_vigilance(
    state: &mut KeywordGameState,
    creature: ObjectId,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    {
        let object = state.object(creature)?;
        require_keyword(object, OfficialKeyword::Vigilance)?;
        if !can_attack(state, creature)? {
            return Err(KeywordExecutionError::InvalidCombatParticipant(creature));
        }
    }
    let object = state.object_mut(creature)?;
    object.attacking = true;
    Ok(vec![KeywordEvidenceEvent::AttackerDeclared {
        creature,
        tapped: object.tapped,
    }])
}

fn execute_assign_trample_damage(
    state: &mut KeywordGameState,
    attacker: ObjectId,
    blockers: &[TrampleBlockerAssignment],
    defending_player: PlayerId,
    player_damage: u32,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    if !state.players.contains_key(&defending_player) {
        return Err(KeywordExecutionError::MissingPlayer);
    }
    let attacker_object = state.object(attacker)?.clone();
    require_keyword(&attacker_object, OfficialKeyword::Trample)?;
    if attacker_object.zone != Zone::Battlefield
        || !attacker_object.is_creature()
        || !attacker_object.attacking
        || attacker_object.controller == defending_player
    {
        return Err(KeywordExecutionError::InvalidCombatParticipant(attacker));
    }
    let assigned_power = attacker_object
        .effective_characteristics()
        .power
        .unwrap_or(0)
        .max(0) as u32;
    let assigned_to_blockers = blockers.iter().try_fold(0u32, |total, assignment| {
        total
            .checked_add(assignment.assigned_damage)
            .ok_or(KeywordExecutionError::IllegalCombatAssignment)
    })?;
    if assigned_to_blockers
        .checked_add(player_damage)
        .ok_or(KeywordExecutionError::IllegalCombatAssignment)?
        != assigned_power
    {
        return Err(KeywordExecutionError::IllegalCombatAssignment);
    }

    let has_deathtouch = attacker_object
        .combat_keywords
        .contains(&CombatKeyword::Deathtouch);
    let mut unique_blockers = BTreeSet::new();
    for assignment in blockers {
        if assignment.blocker == attacker || !unique_blockers.insert(assignment.blocker) {
            return Err(KeywordExecutionError::IllegalCombatAssignment);
        }
        let blocker = state.object(assignment.blocker)?;
        if blocker.zone != Zone::Battlefield
            || !blocker.is_creature()
            || !blocker.blocking
            || blocker.controller != defending_player
        {
            return Err(KeywordExecutionError::InvalidCombatParticipant(
                assignment.blocker,
            ));
        }
        let remaining_toughness = blocker
            .effective_characteristics()
            .toughness
            .unwrap_or(0)
            .max(0) as u32;
        let remaining_after_other_damage = remaining_toughness
            .saturating_sub(blocker.damage_marked)
            .saturating_sub(assignment.damage_assigned_by_other_attackers);
        let lethal = if remaining_after_other_damage == 0 {
            0
        } else if has_deathtouch {
            1
        } else {
            remaining_after_other_damage
        };
        if player_damage > 0 && assignment.assigned_damage < lethal {
            return Err(KeywordExecutionError::IllegalCombatAssignment);
        }
    }
    if blockers.is_empty() && player_damage != assigned_power {
        return Err(KeywordExecutionError::IllegalCombatAssignment);
    }

    let source = SourceProfile::from_object(&attacker_object);
    let mut actual_damage_dealt = 0u32;
    for assignment in blockers {
        let prevented = protection_forbids(
            state,
            ProtectionTarget::Object(assignment.blocker),
            &source,
            ProtectionInteraction::Damage,
        )?;
        if !prevented {
            let blocker = state.object_mut(assignment.blocker)?;
            blocker.damage_marked = blocker
                .damage_marked
                .saturating_add(assignment.assigned_damage);
            if has_deathtouch && assignment.assigned_damage > 0 {
                blocker.damaged_by_deathtouch_since_state_check = true;
            }
            actual_damage_dealt = actual_damage_dealt
                .checked_add(assignment.assigned_damage)
                .ok_or(KeywordExecutionError::IllegalCombatAssignment)?;
        }
    }
    let player_damage_prevented = protection_forbids(
        state,
        ProtectionTarget::Player(defending_player),
        &source,
        ProtectionInteraction::Damage,
    )?;
    if !player_damage_prevented {
        let defending_state = state
            .players
            .get_mut(&defending_player)
            .ok_or(KeywordExecutionError::MissingPlayer)?;
        defending_state.life = defending_state
            .life
            .saturating_sub(i32::try_from(player_damage).unwrap_or(i32::MAX));
        actual_damage_dealt = actual_damage_dealt
            .checked_add(player_damage)
            .ok_or(KeywordExecutionError::IllegalCombatAssignment)?;
    }
    if attacker_object
        .combat_keywords
        .contains(&CombatKeyword::Lifelink)
        && actual_damage_dealt > 0
    {
        gain_lifelink_life(state, &attacker_object, actual_damage_dealt)?;
    }

    Ok(vec![KeywordEvidenceEvent::TrampleDamageAssigned {
        attacker,
        blockers: blockers.to_vec(),
        defending_player,
        assigned_to_player: player_damage,
        actual_damage_dealt,
    }])
}

fn execute_record_deathtouch_damage(
    state: &mut KeywordGameState,
    source: ObjectId,
    creature: ObjectId,
    damage: u32,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let source_object = state.object(source)?;
    require_keyword(source_object, OfficialKeyword::Deathtouch)?;
    if damage == 0 {
        return Err(KeywordExecutionError::InvalidDamageSource);
    }
    let creature_object = state.object(creature)?;
    if creature_object.zone != Zone::Battlefield || !creature_object.is_creature() {
        return Err(KeywordExecutionError::InvalidCombatParticipant(creature));
    }
    let creature_object = state.object_mut(creature)?;
    creature_object.damage_marked = creature_object.damage_marked.saturating_add(damage);
    creature_object.damaged_by_deathtouch_since_state_check = true;
    Ok(vec![KeywordEvidenceEvent::DeathtouchDamageRecorded {
        source,
        creature,
        damage,
    }])
}

fn gain_lifelink_life(
    state: &mut KeywordGameState,
    source: &KeywordObject,
    damage_dealt: u32,
) -> Result<PlayerId, KeywordExecutionError> {
    let player = source.controller;
    let player_state = state
        .players
        .get_mut(&player)
        .ok_or(KeywordExecutionError::MissingPlayer)?;
    player_state.life = player_state
        .life
        .saturating_add(i32::try_from(damage_dealt).unwrap_or(i32::MAX));
    Ok(player)
}

fn execute_apply_lifelink(
    state: &mut KeywordGameState,
    source: ObjectId,
    damage_dealt: u32,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let source_object = state.object(source)?.clone();
    require_keyword(&source_object, OfficialKeyword::Lifelink)?;
    let player = gain_lifelink_life(state, &source_object, damage_dealt)?;
    Ok(vec![KeywordEvidenceEvent::LifelinkLifeGained {
        source,
        player,
        amount: damage_dealt,
    }])
}

fn execute_record_combat_damage_eligibility(
    state: &KeywordGameState,
    creature: ObjectId,
    step: CombatDamageStep,
    first_step_exists: bool,
    first_step_snapshot: FirstStepAbilitySnapshot,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object(creature)?;
    if object.zone != Zone::Battlefield
        || !object.is_creature()
        || !(object.attacking || object.blocking)
    {
        return Err(KeywordExecutionError::InvalidCombatParticipant(creature));
    }
    let eligible = match step {
        CombatDamageStep::First => {
            first_step_exists && first_step_snapshot != FirstStepAbilitySnapshot::Neither
        }
        CombatDamageStep::Second if !first_step_exists => true,
        CombatDamageStep::Second => {
            first_step_snapshot == FirstStepAbilitySnapshot::Neither
                || object
                    .combat_keywords
                    .contains(&CombatKeyword::DoubleStrike)
        }
    };
    Ok(vec![
        KeywordEvidenceEvent::CombatDamageEligibilityRecorded {
            creature,
            step,
            eligible,
        },
    ])
}

fn execute_attempt_indestructible_destruction(
    state: &KeywordGameState,
    permanent: ObjectId,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object(permanent)?;
    require_keyword(object, OfficialKeyword::Indestructible)?;
    if object.zone != Zone::Battlefield {
        return Err(KeywordExecutionError::WrongZone {
            object: permanent,
            expected: Zone::Battlefield,
            actual: object.zone,
        });
    }
    Ok(vec![
        KeywordEvidenceEvent::DestructionIgnoredByIndestructible { permanent },
    ])
}

fn execute_resolve_prowess_trigger(
    state: &mut KeywordGameState,
    creature: ObjectId,
    spell: ObjectId,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let creature_object = state.object(creature)?;
    require_keyword(creature_object, OfficialKeyword::Prowess)?;
    if creature_object.zone != Zone::Battlefield || !creature_object.is_creature() {
        return Err(KeywordExecutionError::InvalidProwessTrigger);
    }
    let spell_object = state.object(spell)?;
    if spell_object.zone != Zone::Stack
        || spell_object.controller != creature_object.controller
        || spell_object
            .effective_characteristics()
            .card_types
            .contains(&CardType::Creature)
    {
        return Err(KeywordExecutionError::InvalidProwessTrigger);
    }
    let creature_object = state.object_mut(creature)?;
    creature_object.temporary_power_delta = creature_object.temporary_power_delta.saturating_add(1);
    creature_object.temporary_toughness_delta =
        creature_object.temporary_toughness_delta.saturating_add(1);
    Ok(vec![KeywordEvidenceEvent::ProwessResolved {
        creature,
        spell,
        power_delta: 1,
        toughness_delta: 1,
    }])
}

pub fn clear_end_of_turn_keyword_effects(state: &mut KeywordGameState) {
    for object in state.objects.values_mut() {
        object.temporary_power_delta = 0;
        object.temporary_toughness_delta = 0;
        object.damaged_by_deathtouch_since_state_check = false;
    }
}

fn execute_install_ward(
    state: &mut KeywordGameState,
    permanent: ObjectId,
    program: &WardProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object_mut(permanent)?;
    if object.zone != Zone::Battlefield {
        return Err(KeywordExecutionError::WrongZone {
            object: permanent,
            expected: Zone::Battlefield,
            actual: object.zone,
        });
    }
    object.rules_keywords.insert(OfficialKeyword::Ward);
    increment_keyword_instance(object, OfficialKeyword::Ward);
    object.ward_costs.push(program.cost.clone());
    Ok(vec![KeywordEvidenceEvent::WardInstalled {
        permanent,
        cost: program.cost.clone(),
    }])
}

fn execute_resolve_ward(
    state: &mut KeywordGameState,
    permanent: ObjectId,
    source: WardSource,
    payer: PlayerId,
    payment: Option<&ManaPayment>,
    program: &WardProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let permanent_object = state.object(permanent)?;
    if permanent_object.zone != Zone::Battlefield {
        return Err(KeywordExecutionError::WrongZone {
            object: permanent,
            expected: Zone::Battlefield,
            actual: permanent_object.zone,
        });
    }
    require_keyword(permanent_object, OfficialKeyword::Ward)?;
    if !permanent_object.ward_costs.contains(&program.cost) {
        return Err(KeywordExecutionError::WardCostNotInstalled);
    }
    let permanent_controller = permanent_object.controller;
    let (source_controller, countered_spell, countered_ability) = match &source {
        WardSource::Spell(spell) => {
            let spell_object = state.object(*spell)?;
            if spell_object.zone != Zone::Stack {
                return Err(KeywordExecutionError::InvalidWardSource);
            }
            (spell_object.controller, Some(*spell), None)
        }
        WardSource::Ability {
            stack_item_id,
            source,
        } => {
            if state.countered_abilities.contains(stack_item_id) {
                return Err(KeywordExecutionError::InvalidWardSource);
            }
            (source.controller, None, Some(*stack_item_id))
        }
    };
    if source_controller == permanent_controller {
        return Err(KeywordExecutionError::WardDidNotTrigger);
    }
    if payer != source_controller {
        return Err(KeywordExecutionError::WrongController);
    }
    if let Some(payment) = payment {
        let evidence = pay_mana_cost(state, payer, &program.cost, payment)?;
        return Ok(vec![KeywordEvidenceEvent::WardPaid {
            permanent,
            payer,
            mana_spent: evidence.mana_spent,
            life_paid: evidence.life_paid,
        }]);
    }
    if let Some(spell) = countered_spell {
        state.move_object(spell, Zone::Graveyard)?;
        return Ok(vec![KeywordEvidenceEvent::WardCounteredSpell {
            permanent,
            spell,
        }]);
    }
    let stack_item_id = countered_ability.ok_or(KeywordExecutionError::InvalidWardSource)?;
    state.countered_abilities.insert(stack_item_id);
    Ok(vec![KeywordEvidenceEvent::WardCounteredAbility {
        permanent,
        stack_item_id,
    }])
}

fn validate_library_partition(
    looked: &[ObjectId],
    first_destination: &[ObjectId],
    second_destination: &[ObjectId],
) -> Result<(), KeywordExecutionError> {
    let looked_set = looked.iter().copied().collect::<BTreeSet<_>>();
    let destinations = first_destination
        .iter()
        .chain(second_destination)
        .copied()
        .collect::<Vec<_>>();
    let destination_set = destinations.iter().copied().collect::<BTreeSet<_>>();
    if looked_set.len() != looked.len()
        || destination_set.len() != destinations.len()
        || looked_set != destination_set
    {
        return Err(KeywordExecutionError::InvalidLibraryDecision);
    }
    Ok(())
}

fn looked_library_cards(
    state: &KeywordGameState,
    player: PlayerId,
    amount: u32,
) -> Result<Vec<ObjectId>, KeywordExecutionError> {
    let player_state = state
        .players
        .get(&player)
        .ok_or(KeywordExecutionError::MissingPlayer)?;
    let looked = player_state
        .library
        .iter()
        .take(usize::try_from(amount).unwrap_or(usize::MAX))
        .copied()
        .collect::<Vec<_>>();
    for object_id in &looked {
        let object = state.object(*object_id)?;
        if object.owner != player || object.zone != Zone::Library {
            return Err(KeywordExecutionError::LibraryInvariant(*object_id));
        }
    }
    Ok(looked)
}

fn execute_scry(
    state: &mut KeywordGameState,
    player: PlayerId,
    amount: u32,
    top_order: &[ObjectId],
    bottom_order: &[ObjectId],
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    if amount == 0 {
        if !top_order.is_empty() || !bottom_order.is_empty() {
            return Err(KeywordExecutionError::InvalidLibraryDecision);
        }
        return Ok(vec![KeywordEvidenceEvent::LibraryReordered {
            player,
            keyword: OfficialKeyword::Scry,
            looked: Vec::new(),
            top_order: Vec::new(),
            other_destination: Vec::new(),
            event_occurred: false,
        }]);
    }
    let looked = looked_library_cards(state, player, amount)?;
    validate_library_partition(&looked, top_order, bottom_order)?;
    let player_state = state
        .players
        .get_mut(&player)
        .ok_or(KeywordExecutionError::MissingPlayer)?;
    for _ in 0..looked.len() {
        player_state.library.pop_front();
    }
    let remaining = std::mem::take(&mut player_state.library);
    player_state.library.extend(top_order.iter().copied());
    player_state.library.extend(remaining);
    player_state.library.extend(bottom_order.iter().copied());
    Ok(vec![KeywordEvidenceEvent::LibraryReordered {
        player,
        keyword: OfficialKeyword::Scry,
        looked,
        top_order: top_order.to_vec(),
        other_destination: bottom_order.to_vec(),
        event_occurred: true,
    }])
}

fn execute_surveil(
    state: &mut KeywordGameState,
    player: PlayerId,
    amount: u32,
    additional_cards: u32,
    top_order: &[ObjectId],
    graveyard_order: &[ObjectId],
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let total = amount.saturating_add(additional_cards);
    if total == 0 {
        if !top_order.is_empty() || !graveyard_order.is_empty() {
            return Err(KeywordExecutionError::InvalidLibraryDecision);
        }
        return Ok(vec![KeywordEvidenceEvent::LibraryReordered {
            player,
            keyword: OfficialKeyword::Surveil,
            looked: Vec::new(),
            top_order: Vec::new(),
            other_destination: Vec::new(),
            event_occurred: false,
        }]);
    }
    let looked = looked_library_cards(state, player, total)?;
    validate_library_partition(&looked, top_order, graveyard_order)?;
    {
        let player_state = state
            .players
            .get_mut(&player)
            .ok_or(KeywordExecutionError::MissingPlayer)?;
        for _ in 0..looked.len() {
            player_state.library.pop_front();
        }
        let remaining = std::mem::take(&mut player_state.library);
        player_state.library.extend(top_order.iter().copied());
        player_state.library.extend(remaining);
        player_state
            .graveyard
            .extend(graveyard_order.iter().copied());
    }
    for object in graveyard_order {
        state.object_mut(*object)?.zone = Zone::Graveyard;
    }
    Ok(vec![KeywordEvidenceEvent::LibraryReordered {
        player,
        keyword: OfficialKeyword::Surveil,
        looked,
        top_order: top_order.to_vec(),
        other_destination: graveyard_order.to_vec(),
        event_occurred: true,
    }])
}

fn execute_cycle(
    state: &mut KeywordGameState,
    player: PlayerId,
    card: ObjectId,
    payment: &ManaPayment,
    program: &CyclingProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let card_object = state.object(card)?;
    if card_object.zone != program.activation_zone {
        return Err(KeywordExecutionError::WrongZone {
            object: card,
            expected: program.activation_zone,
            actual: card_object.zone,
        });
    }
    if card_object.owner != player {
        return Err(KeywordExecutionError::WrongOwner);
    }
    let payment_evidence = pay_mana_cost(state, player, &program.activation_cost, payment)?;
    state.move_object(card, Zone::Graveyard)?;

    let drawn = state
        .players
        .get(&player)
        .ok_or(KeywordExecutionError::MissingPlayer)?
        .library
        .front()
        .copied();
    let failed_draw = drawn.is_none();
    if let Some(drawn) = drawn {
        let drawn_object = state.object(drawn)?;
        if drawn_object.owner != player || drawn_object.zone != Zone::Library {
            return Err(KeywordExecutionError::LibraryInvariant(drawn));
        }
        state.move_object(drawn, Zone::Hand)?;
    } else {
        let player_state = state
            .players
            .get_mut(&player)
            .ok_or(KeywordExecutionError::MissingPlayer)?;
        player_state.failed_draw_attempts = player_state.failed_draw_attempts.saturating_add(1);
    }
    Ok(vec![KeywordEvidenceEvent::Cycled {
        player,
        card,
        drawn,
        failed_draw,
        mana_spent: payment_evidence.mana_spent,
        life_paid: payment_evidence.life_paid,
    }])
}

fn execute_convoke(
    state: &mut KeywordGameState,
    player: PlayerId,
    spell: ObjectId,
    total_cost: &ManaCost,
    convoking_creatures: &BTreeMap<usize, Vec<ObjectId>>,
    mana_payment: &ManaPayment,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let spell_object = state.object(spell)?;
    if spell_object.zone != Zone::Stack {
        return Err(KeywordExecutionError::WrongZone {
            object: spell,
            expected: Zone::Stack,
            actual: spell_object.zone,
        });
    }
    if spell_object.controller != player {
        return Err(KeywordExecutionError::WrongController);
    }
    if convoking_creatures
        .keys()
        .any(|index| *index >= total_cost.symbols.len())
    {
        return Err(KeywordExecutionError::InvalidConvokeTotalCost);
    }

    let has_variable = total_cost
        .symbols
        .iter()
        .any(|symbol| matches!(symbol, ManaSymbol::VariableX));
    let locked_x = match (has_variable, mana_payment.x_value) {
        (true, Some(value)) => Some(value),
        (true, None) => return Err(KeywordExecutionError::MissingVariableValue),
        (false, Some(_)) => return Err(KeywordExecutionError::UnexpectedVariableValue),
        (false, None) => None,
    };

    let mut used_creatures = BTreeSet::new();
    let mut adjusted_symbols = total_cost.symbols.clone();
    for (symbol_index, symbol) in total_cost.symbols.iter().enumerate() {
        let creatures = convoking_creatures
            .get(&symbol_index)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let maximum = match symbol {
            ManaSymbol::Generic(amount) => usize::try_from(*amount)
                .map_err(|_| KeywordExecutionError::InvalidConvokeTotalCost)?,
            ManaSymbol::Colored(_) | ManaSymbol::Hybrid(_, _) | ManaSymbol::Phyrexian(_) => 1,
            ManaSymbol::Colorless | ManaSymbol::Snow => 0,
            ManaSymbol::VariableX => {
                usize::try_from(locked_x.ok_or(KeywordExecutionError::MissingVariableValue)?)
                    .map_err(|_| KeywordExecutionError::InvalidConvokeTotalCost)?
            }
        };
        if creatures.len() > maximum {
            return Err(KeywordExecutionError::InvalidConvokeTotalCost);
        }
        for creature_id in creatures {
            if !used_creatures.insert(*creature_id) {
                return Err(KeywordExecutionError::DuplicateConvokeCreature(
                    *creature_id,
                ));
            }
            let creature = state.object(*creature_id)?;
            if creature.zone != Zone::Battlefield
                || creature.controller != player
                || creature.tapped
                || !creature.is_creature()
            {
                return Err(KeywordExecutionError::InvalidConvokeCreature(*creature_id));
            }
            let colors = &creature.effective_characteristics().colors;
            let matches_colored_requirement = match symbol {
                ManaSymbol::Colored(color) | ManaSymbol::Phyrexian(color) => colors.contains(color),
                ManaSymbol::Hybrid(first, second) => {
                    colors.contains(first) || colors.contains(second)
                }
                _ => true,
            };
            if !matches_colored_requirement {
                return Err(KeywordExecutionError::InvalidConvokeCreature(*creature_id));
            }
        }
        adjusted_symbols[symbol_index] = match symbol {
            ManaSymbol::Generic(amount) => ManaSymbol::Generic(
                amount
                    .checked_sub(u32::try_from(creatures.len()).unwrap_or(u32::MAX))
                    .ok_or(KeywordExecutionError::InvalidConvokeTotalCost)?,
            ),
            ManaSymbol::Colored(_) | ManaSymbol::Hybrid(_, _) | ManaSymbol::Phyrexian(_)
                if creatures.len() == 1 =>
            {
                ManaSymbol::Generic(0)
            }
            ManaSymbol::Colored(color) => ManaSymbol::Colored(*color),
            ManaSymbol::Hybrid(first, second) => ManaSymbol::Hybrid(*first, *second),
            ManaSymbol::Phyrexian(color) => ManaSymbol::Phyrexian(*color),
            ManaSymbol::VariableX => ManaSymbol::Generic(
                locked_x
                    .ok_or(KeywordExecutionError::MissingVariableValue)?
                    .checked_sub(u32::try_from(creatures.len()).unwrap_or(u32::MAX))
                    .ok_or(KeywordExecutionError::InvalidConvokeTotalCost)?,
            ),
            ManaSymbol::Colorless => ManaSymbol::Colorless,
            ManaSymbol::Snow => ManaSymbol::Snow,
        };
    }
    let adjusted_cost = ManaCost {
        raw: total_cost.raw.clone(),
        symbols: adjusted_symbols,
    };
    let mut adjusted_payment = mana_payment.clone();
    if has_variable {
        adjusted_payment.x_value = None;
    }
    let payment_evidence = pay_mana_cost(state, player, &adjusted_cost, &adjusted_payment)?;
    for creature in &used_creatures {
        state.object_mut(*creature)?.tapped = true;
    }
    Ok(vec![KeywordEvidenceEvent::ConvokeCostPaid {
        player,
        spell,
        convoking_creatures: used_creatures.into_iter().collect(),
        mana_spent: payment_evidence.mana_spent,
        life_paid: payment_evidence.life_paid,
    }])
}

fn execute_equip_activation(
    state: &mut KeywordGameState,
    player: PlayerId,
    equipment: ObjectId,
    target: ObjectId,
    sorcery_timing_legal: bool,
    program: &EquipProgram,
    payment: &ManaPayment,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    if program.sorcery_timing_only && !sorcery_timing_legal {
        return Err(KeywordExecutionError::InvalidTiming);
    }
    if state.pending_equip.contains_key(&equipment) {
        return Err(KeywordExecutionError::PendingEquipAlreadyExists);
    }
    let equipment_object = state.object(equipment)?;
    if equipment_object.zone != Zone::Battlefield {
        return Err(KeywordExecutionError::WrongZone {
            object: equipment,
            expected: Zone::Battlefield,
            actual: equipment_object.zone,
        });
    }
    if equipment_object.controller != player {
        return Err(KeywordExecutionError::WrongController);
    }
    let equipment_characteristics = equipment_object.effective_characteristics();
    if !equipment_characteristics
        .card_types
        .contains(&CardType::Artifact)
        || !equipment_characteristics
            .subtypes
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case("Equipment"))
        || (equipment_object.is_creature() && !equipment_object.has_reconfigure)
        || equipment == target
    {
        return Err(KeywordExecutionError::IllegalAttachment);
    }
    if !matches_object_predicate(state, target, &program.target_filter, player)? {
        return Err(KeywordExecutionError::IllegalAttachment);
    }
    let source = SourceProfile::from_object(equipment_object);
    if protection_forbids(
        state,
        ProtectionTarget::Object(target),
        &source,
        ProtectionInteraction::Target,
    )? {
        return Err(KeywordExecutionError::IllegalAttachment);
    }
    let payment_evidence = pay_mana_cost(state, player, &program.activation_cost, payment)?;
    state.pending_equip.insert(
        equipment,
        PendingEquipActivation {
            equipment,
            target,
            activating_player: player,
            target_filter: program.target_filter.clone(),
            planeswalker_as_creature: program.planeswalker_as_creature,
        },
    );
    Ok(vec![KeywordEvidenceEvent::EquipActivated {
        player,
        equipment,
        target,
        mana_spent: payment_evidence.mana_spent,
        life_paid: payment_evidence.life_paid,
    }])
}

fn execute_equip_resolution(
    state: &mut KeywordGameState,
    equipment: ObjectId,
    _program: &EquipProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let pending = state
        .pending_equip
        .remove(&equipment)
        .ok_or(KeywordExecutionError::MissingPendingEquip)?;
    let legal = state
        .objects
        .get(&equipment)
        .zip(state.objects.get(&pending.target))
        .is_some_and(|(equipment_object, target_object)| {
            equipment_object.zone == Zone::Battlefield
                && target_object.zone == Zone::Battlefield
                && equipment_object.controller == pending.activating_player
        })
        && matches_object_predicate(
            state,
            pending.target,
            &pending.target_filter,
            pending.activating_player,
        )
        .unwrap_or(false)
        && state
            .objects
            .get(&equipment)
            .is_some_and(|equipment_object| {
                let source = SourceProfile::from_object(equipment_object);
                !protection_forbids(
                    state,
                    ProtectionTarget::Object(pending.target),
                    &source,
                    ProtectionInteraction::Target,
                )
                .unwrap_or(true)
            });
    if !legal {
        return Ok(vec![KeywordEvidenceEvent::EquipResolutionFailed {
            equipment,
            target: pending.target,
        }]);
    }
    let previous_target = match state.object(equipment)?.attached_to {
        Some(ProtectionTarget::Object(object)) => Some(object),
        _ => None,
    };
    state.object_mut(equipment)?.attached_to = Some(ProtectionTarget::Object(pending.target));
    Ok(vec![KeywordEvidenceEvent::EquipmentAttached {
        equipment,
        target: pending.target,
        previous_target,
    }])
}

fn execute_aura_resolution(
    state: &mut KeywordGameState,
    player: PlayerId,
    aura: ObjectId,
    target: ProtectionTarget,
    program: &EnchantProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let aura_object = state.object(aura)?;
    if aura_object.zone != Zone::Stack {
        return Err(KeywordExecutionError::WrongZone {
            object: aura,
            expected: Zone::Stack,
            actual: aura_object.zone,
        });
    }
    if aura_object.controller != player {
        return Err(KeywordExecutionError::WrongController);
    }
    let characteristics = aura_object.effective_characteristics();
    let is_aura = characteristics.card_types.contains(&CardType::Enchantment)
        && characteristics
            .subtypes
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case("Aura"));
    if !is_aura || aura_object.is_creature() {
        return Err(KeywordExecutionError::IllegalAttachment);
    }
    let source = SourceProfile::from_object(aura_object);
    let legal = matches_attachment_filter(state, target, &program.target_filter, player)?
        && target != ProtectionTarget::Object(aura)
        && !protection_forbids(state, target, &source, ProtectionInteraction::Target)?;
    if !legal {
        state.move_object(aura, Zone::Graveyard)?;
        return Ok(vec![KeywordEvidenceEvent::AuraMovedToGraveyard { aura }]);
    }
    state.move_object(aura, Zone::Battlefield)?;
    state.object_mut(aura)?.attached_to = Some(target);
    Ok(vec![KeywordEvidenceEvent::AuraAttached { aura, target }])
}

fn execute_aura_state_check(
    state: &mut KeywordGameState,
    aura: ObjectId,
    program: &EnchantProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let aura_object = state.object(aura)?.clone();
    if aura_object.zone != Zone::Battlefield {
        return Err(KeywordExecutionError::WrongZone {
            object: aura,
            expected: Zone::Battlefield,
            actual: aura_object.zone,
        });
    }
    let target = aura_object.attached_to;
    let legal = target.is_some_and(|target| {
        target != ProtectionTarget::Object(aura)
            && !aura_object.is_creature()
            && matches_attachment_filter(
                state,
                target,
                &program.target_filter,
                aura_object.controller,
            )
            .unwrap_or(false)
            && {
                let source = SourceProfile::from_object(&aura_object);
                !protection_forbids(state, target, &source, ProtectionInteraction::Enchant)
                    .unwrap_or(true)
            }
    });
    if legal {
        return Ok(Vec::new());
    }
    state.object_mut(aura)?.attached_to = None;
    state.move_object(aura, Zone::Graveyard)?;
    Ok(vec![KeywordEvidenceEvent::AuraMovedToGraveyard { aura }])
}

fn matches_attachment_filter(
    state: &KeywordGameState,
    target: ProtectionTarget,
    filter: &AttachmentFilter,
    actor: PlayerId,
) -> Result<bool, KeywordExecutionError> {
    match (target, filter) {
        (ProtectionTarget::Object(object), AttachmentFilter::Object(predicate)) => {
            matches_object_predicate(state, object, predicate, actor)
        }
        (ProtectionTarget::Player(player), AttachmentFilter::Player(relation)) => {
            if !state.players.contains_key(&player) {
                return Err(KeywordExecutionError::MissingPlayer);
            }
            Ok(matches_relative_player(player, actor, *relation))
        }
        _ => Ok(false),
    }
}

fn matches_object_predicate(
    state: &KeywordGameState,
    object: ObjectId,
    predicate: &ObjectPredicate,
    actor: PlayerId,
) -> Result<bool, KeywordExecutionError> {
    let object = state.object(object)?;
    let characteristics = object.effective_characteristics();
    Ok(match predicate {
        ObjectPredicate::Permanent => object.zone == Zone::Battlefield,
        ObjectPredicate::CardType(card_type) => characteristics.card_types.contains(card_type),
        ObjectPredicate::Subtype(subtype) => characteristics
            .subtypes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(subtype)),
        ObjectPredicate::Supertype(supertype) => characteristics
            .supertypes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(supertype)),
        ObjectPredicate::Color(color) => characteristics.colors.contains(color),
        ObjectPredicate::Commander => object.is_commander,
        ObjectPredicate::Tapped => object.tapped,
        ObjectPredicate::Controller(relation) => {
            matches_relative_player(object.controller, actor, *relation)
        }
        ObjectPredicate::Zone(zone) => object.zone == *zone,
        ObjectPredicate::Not(inner) => !matches_object_predicate(state, object.id, inner, actor)?,
        ObjectPredicate::All(predicates) => {
            for predicate in predicates {
                if !matches_object_predicate(state, object.id, predicate, actor)? {
                    return Ok(false);
                }
            }
            true
        }
        ObjectPredicate::Any(predicates) => {
            let mut matched = false;
            for predicate in predicates {
                matched |= matches_object_predicate(state, object.id, predicate, actor)?;
            }
            matched
        }
    })
}

fn matches_relative_player(candidate: PlayerId, actor: PlayerId, relation: RelativePlayer) -> bool {
    match relation {
        RelativePlayer::You => candidate == actor,
        RelativePlayer::Opponent => candidate != actor,
        RelativePlayer::Any => true,
    }
}

fn execute_saga_entry(
    state: &mut KeywordGameState,
    permanent: ObjectId,
    program: &SagaProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    if !program.enters_with_one_lore_counter {
        return Err(KeywordExecutionError::InvalidSagaState);
    }
    if lore_counters(state.object(permanent)?) != 0 {
        return Err(KeywordExecutionError::InvalidSagaState);
    }
    add_saga_lore_counter(state, permanent, program)
}

fn execute_saga_advance(
    state: &mut KeywordGameState,
    permanent: ObjectId,
    active_player: PlayerId,
    program: &SagaProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object(permanent)?;
    if object.zone != Zone::Battlefield || object.controller != active_player {
        return Err(KeywordExecutionError::InvalidSagaState);
    }
    add_saga_lore_counter(state, permanent, program)
}

fn add_saga_lore_counter(
    state: &mut KeywordGameState,
    permanent: ObjectId,
    program: &SagaProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object(permanent)?;
    let characteristics = object.effective_characteristics();
    if object.zone != Zone::Battlefield
        || !characteristics
            .subtypes
            .iter()
            .any(|subtype| subtype.eq_ignore_ascii_case("Saga"))
    {
        return Err(KeywordExecutionError::InvalidSagaState);
    }
    let before = lore_counters(object);
    let after = before
        .checked_add(1)
        .ok_or(KeywordExecutionError::InvalidSagaState)?;
    state
        .object_mut(permanent)?
        .counters
        .insert("lore".into(), after);
    let mut triggered = Vec::new();
    for chapter in &program.chapters {
        for number in &chapter.numbers {
            if before < *number && after >= *number {
                triggered.push(*number);
                state.pending_saga_chapters.push(PendingSagaChapter {
                    saga: permanent,
                    chapter_number: *number,
                    oracle_effect: chapter.oracle_effect.clone(),
                });
            }
        }
    }
    Ok(vec![KeywordEvidenceEvent::SagaLoreCountersAdded {
        saga: permanent,
        before,
        after,
        triggered_chapters: triggered,
    }])
}

fn lore_counters(object: &KeywordObject) -> u32 {
    object.counters.get("lore").copied().unwrap_or(0)
}

fn execute_saga_chapter_exit(
    state: &mut KeywordGameState,
    saga: ObjectId,
    chapter_number: u32,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let Some(index) = state
        .pending_saga_chapters
        .iter()
        .position(|pending| pending.saga == saga && pending.chapter_number == chapter_number)
    else {
        return Err(KeywordExecutionError::MissingSagaChapter);
    };
    state.pending_saga_chapters.remove(index);
    Ok(vec![KeywordEvidenceEvent::SagaChapterLeftStack {
        saga,
        chapter_number,
    }])
}

fn execute_saga_sacrifice_check(
    state: &mut KeywordGameState,
    permanent: ObjectId,
    program: &SagaProgram,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object(permanent)?;
    if object.zone != Zone::Battlefield || lore_counters(object) < program.final_chapter {
        return Err(KeywordExecutionError::InvalidSagaState);
    }
    let pending = state
        .pending_saga_chapters
        .iter()
        .filter(|chapter| chapter.saga == permanent)
        .map(|chapter| chapter.chapter_number)
        .collect::<Vec<_>>();
    if !pending.is_empty() {
        return Ok(vec![KeywordEvidenceEvent::SagaSacrificeDeferred {
            saga: permanent,
            pending_chapters: pending,
        }]);
    }
    state.move_object(permanent, Zone::Graveyard)?;
    Ok(vec![KeywordEvidenceEvent::SagaSacrificed {
        saga: permanent,
    }])
}

fn execute_cumulative_upkeep(
    state: &mut KeywordGameState,
    permanent: ObjectId,
    player: PlayerId,
    program: &CumulativeUpkeepProgram,
    payments: Option<&[CumulativeUpkeepPayment]>,
) -> Result<Vec<KeywordEvidenceEvent>, KeywordExecutionError> {
    let object = state.object(permanent)?;
    if object.zone != Zone::Battlefield {
        return Err(KeywordExecutionError::WrongZone {
            object: permanent,
            expected: Zone::Battlefield,
            actual: object.zone,
        });
    }
    if object.controller != player {
        return Err(KeywordExecutionError::WrongController);
    }
    let age_counters = object
        .counters
        .get("age")
        .copied()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(KeywordExecutionError::InvalidCumulativePaymentCount {
            expected: usize::MAX,
            actual: 0,
        })?;
    state
        .object_mut(permanent)?
        .counters
        .insert("age".into(), age_counters);
    let mut events = vec![KeywordEvidenceEvent::CumulativeUpkeepAgeCounterAdded {
        permanent,
        age_counters,
    }];
    let Some(payments) = payments else {
        state.move_object(permanent, Zone::Graveyard)?;
        events.push(
            KeywordEvidenceEvent::CumulativeUpkeepDeclinedAndSacrificed {
                permanent,
                age_counters,
            },
        );
        return Ok(events);
    };
    let expected = usize::try_from(age_counters).unwrap_or(usize::MAX);
    if payments.len() != expected {
        return Err(KeywordExecutionError::InvalidCumulativePaymentCount {
            expected,
            actual: payments.len(),
        });
    }
    let mut mana_spent = Vec::new();
    let mut life_paid = 0u32;
    match &program.cost_per_age_counter {
        CumulativeUpkeepCost::ManaAlternatives(alternatives) => {
            for payment in payments {
                let cost = alternatives
                    .get(payment.alternative_index)
                    .ok_or(KeywordExecutionError::CumulativeAlternativeOutOfRange)?;
                let evidence = pay_mana_cost(state, player, cost, &payment.mana_payment)?;
                mana_spent.extend(evidence.mana_spent);
                life_paid = life_paid
                    .checked_add(evidence.life_paid)
                    .ok_or(KeywordExecutionError::InsufficientLife)?;
            }
        }
        CumulativeUpkeepCost::PayLife(amount) => {
            if payments.iter().any(|payment| {
                payment.alternative_index != 0
                    || !payment.mana_payment.symbols.is_empty()
                    || payment.mana_payment.x_value.is_some()
            }) {
                return Err(KeywordExecutionError::CumulativeAlternativeOutOfRange);
            }
            let total = amount
                .checked_mul(age_counters)
                .ok_or(KeywordExecutionError::InsufficientLife)?;
            pay_life(state, player, total)?;
            life_paid = total;
        }
    }
    events.push(KeywordEvidenceEvent::CumulativeUpkeepPaid {
        permanent,
        age_counters,
        mana_spent,
        life_paid,
    });
    Ok(events)
}

fn pay_life(
    state: &mut KeywordGameState,
    player: PlayerId,
    amount: u32,
) -> Result<(), KeywordExecutionError> {
    let player = state
        .players
        .get_mut(&player)
        .ok_or(KeywordExecutionError::MissingPlayer)?;
    let amount = i32::try_from(amount).map_err(|_| KeywordExecutionError::InsufficientLife)?;
    if player.life < amount {
        return Err(KeywordExecutionError::InsufficientLife);
    }
    player.life -= amount;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManaPaymentEvidence {
    mana_spent: Vec<ManaUnitId>,
    life_paid: u32,
}

fn pay_mana_cost(
    state: &mut KeywordGameState,
    player: PlayerId,
    cost: &ManaCost,
    payment: &ManaPayment,
) -> Result<ManaPaymentEvidence, KeywordExecutionError> {
    let player_state = state
        .players
        .get(&player)
        .ok_or(KeywordExecutionError::MissingPlayer)?;
    let units = player_state
        .mana_pool
        .iter()
        .map(|unit| (unit.id, unit.clone()))
        .collect::<BTreeMap<_, _>>();
    let has_variable = cost
        .symbols
        .iter()
        .any(|symbol| matches!(symbol, ManaSymbol::VariableX));
    if has_variable && payment.x_value.is_none() {
        return Err(KeywordExecutionError::MissingVariableValue);
    }
    if !has_variable && payment.x_value.is_some() {
        return Err(KeywordExecutionError::UnexpectedVariableValue);
    }

    let mut used = BTreeSet::new();
    let mut mana_spent = Vec::new();
    let mut life_paid = 0u32;
    for (symbol_index, symbol) in cost.symbols.iter().enumerate() {
        let required_units = match symbol {
            ManaSymbol::Generic(amount) => *amount,
            ManaSymbol::VariableX => payment
                .x_value
                .ok_or(KeywordExecutionError::MissingVariableValue)?,
            ManaSymbol::Colored(_)
            | ManaSymbol::Colorless
            | ManaSymbol::Snow
            | ManaSymbol::Hybrid(_, _)
            | ManaSymbol::Phyrexian(_) => 1,
        };
        let supplied = payment.symbols.get(&symbol_index);
        if required_units == 0 {
            if supplied.is_some() {
                return Err(KeywordExecutionError::UnexpectedManaPayment { symbol_index });
            }
            continue;
        }
        let supplied =
            supplied.ok_or(KeywordExecutionError::MissingManaPayment { symbol_index })?;
        match (symbol, supplied) {
            (ManaSymbol::Phyrexian(_), SymbolPayment::Life(amount)) => {
                if *amount != 2 {
                    return Err(KeywordExecutionError::InvalidManaPayment { symbol_index });
                }
                life_paid = life_paid
                    .checked_add(*amount)
                    .ok_or(KeywordExecutionError::InsufficientLife)?;
            }
            (_, SymbolPayment::Life(_)) => {
                return Err(KeywordExecutionError::InvalidManaPayment { symbol_index });
            }
            (symbol, SymbolPayment::Mana(ids)) => {
                if ids.len() != usize::try_from(required_units).unwrap_or(usize::MAX) {
                    return Err(KeywordExecutionError::InvalidManaPayment { symbol_index });
                }
                for id in ids {
                    if !used.insert(*id) {
                        return Err(KeywordExecutionError::DuplicateManaUnit(*id));
                    }
                    let unit = units
                        .get(id)
                        .ok_or(KeywordExecutionError::MissingManaUnit(*id))?;
                    let legal = match symbol {
                        ManaSymbol::Generic(_) | ManaSymbol::VariableX => true,
                        ManaSymbol::Colored(color) => unit.color == *color,
                        ManaSymbol::Colorless => unit.color == ManaColor::Colorless,
                        ManaSymbol::Snow => unit.from_snow_source,
                        ManaSymbol::Hybrid(first, second) => {
                            unit.color == *first || unit.color == *second
                        }
                        ManaSymbol::Phyrexian(color) => unit.color == *color,
                    };
                    if !legal {
                        return Err(KeywordExecutionError::InvalidManaPayment { symbol_index });
                    }
                    mana_spent.push(*id);
                }
            }
        }
    }
    if let Some(unexpected) = payment
        .symbols
        .keys()
        .find(|index| **index >= cost.symbols.len())
    {
        return Err(KeywordExecutionError::UnexpectedManaPayment {
            symbol_index: *unexpected,
        });
    }
    if player_state.life < i32::try_from(life_paid).unwrap_or(i32::MAX) {
        return Err(KeywordExecutionError::InsufficientLife);
    }

    let player_state = state
        .players
        .get_mut(&player)
        .ok_or(KeywordExecutionError::MissingPlayer)?;
    player_state.life -= i32::try_from(life_paid).unwrap_or(i32::MAX);
    player_state
        .mana_pool
        .retain(|unit| !used.contains(&unit.id));
    Ok(ManaPaymentEvidence {
        mana_spent,
        life_paid,
    })
}

pub fn protection_forbids(
    state: &KeywordGameState,
    protected: ProtectionTarget,
    source: &SourceProfile,
    interaction: ProtectionInteraction,
) -> Result<bool, KeywordExecutionError> {
    let qualities = match protected {
        ProtectionTarget::Object(object) => &state.object(object)?.protections,
        ProtectionTarget::Player(player) => {
            &state
                .players
                .get(&player)
                .ok_or(KeywordExecutionError::MissingPlayer)?
                .protections
        }
    };
    let interaction_is_protected = matches!(
        interaction,
        ProtectionInteraction::Target
            | ProtectionInteraction::Enchant
            | ProtectionInteraction::Equip
            | ProtectionInteraction::Fortify
            | ProtectionInteraction::Damage
            | ProtectionInteraction::Block
    );
    Ok(interaction_is_protected
        && qualities
            .iter()
            .any(|quality| protection_quality_matches(quality, source)))
}

pub fn targeting_is_legal(
    state: &KeywordGameState,
    target: ProtectionTarget,
    source: &SourceProfile,
) -> Result<bool, KeywordExecutionError> {
    let forbidden_by_targeting_restriction = match target {
        ProtectionTarget::Object(object_id) => {
            let object = state.object(object_id)?;
            object.has_shroud
                || (source.controller != object.controller
                    && (object.has_hexproof
                        || object
                            .hexproof_qualities
                            .iter()
                            .any(|quality| protection_quality_matches(quality, source))))
        }
        ProtectionTarget::Player(player_id) => {
            let player = state
                .players
                .get(&player_id)
                .ok_or(KeywordExecutionError::MissingPlayer)?;
            player.has_shroud
                || (source.controller != player_id
                    && (player.has_hexproof
                        || player
                            .hexproof_qualities
                            .iter()
                            .any(|quality| protection_quality_matches(quality, source))))
        }
    };
    if forbidden_by_targeting_restriction {
        return Ok(false);
    }
    Ok(!protection_forbids(
        state,
        target,
        source,
        ProtectionInteraction::Target,
    )?)
}

fn protection_quality_matches(quality: &ProtectionQuality, source: &SourceProfile) -> bool {
    match quality {
        ProtectionQuality::Everything => true,
        ProtectionQuality::Color(color) => source.colors.contains(color),
        ProtectionQuality::Colored => !source.colors.is_empty(),
        ProtectionQuality::Colorless => source.colors.is_empty(),
        ProtectionQuality::Monocolored => source.colors.len() == 1,
        ProtectionQuality::Multicolored => source.colors.len() > 1,
        ProtectionQuality::CardType(card_type) => source
            .card_types
            .iter()
            .any(|candidate| candidate.normalized_name().eq_ignore_ascii_case(card_type)),
        ProtectionQuality::Subtype(subtype) => source
            .subtypes
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(subtype)),
        ProtectionQuality::Named(name) => source
            .name
            .as_ref()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name)),
        ProtectionQuality::Player(player) => {
            source.controller == *player
                || (source.owner == *player && source.controller == source.owner)
        }
        ProtectionQuality::ManaValueAtMost(value) => source.mana_value <= *value,
        ProtectionQuality::ManaValueAtLeast(value) => source.mana_value >= *value,
    }
}

pub fn can_block(
    state: &KeywordGameState,
    attacker: ObjectId,
    blocker: ObjectId,
) -> Result<bool, KeywordExecutionError> {
    let defending_player = state.object(blocker)?.controller;
    can_block_for_defending_player(state, attacker, blocker, defending_player)
}

pub fn can_block_for_defending_player(
    state: &KeywordGameState,
    attacker: ObjectId,
    blocker: ObjectId,
    defending_player: PlayerId,
) -> Result<bool, KeywordExecutionError> {
    if !state.players.contains_key(&defending_player) {
        return Err(KeywordExecutionError::MissingPlayer);
    }
    let attacker_object = state.object(attacker)?;
    let blocker_object = state.object(blocker)?;
    if attacker_object.zone != Zone::Battlefield
        || blocker_object.zone != Zone::Battlefield
        || !attacker_object.is_creature()
        || !blocker_object.is_creature()
        || attacker_object.controller == defending_player
        || blocker_object.controller != defending_player
    {
        return Ok(false);
    }
    let blocker_source = SourceProfile::from_object(blocker_object);
    if protection_forbids(
        state,
        ProtectionTarget::Object(attacker),
        &blocker_source,
        ProtectionInteraction::Block,
    )? {
        return Ok(false);
    }
    let attacker_has_shadow = attacker_object
        .combat_keywords
        .contains(&CombatKeyword::Shadow);
    let blocker_has_shadow = blocker_object
        .combat_keywords
        .contains(&CombatKeyword::Shadow);
    if attacker_has_shadow != blocker_has_shadow {
        return Ok(false);
    }
    if attacker_object
        .combat_keywords
        .contains(&CombatKeyword::Fear)
    {
        let blocker_characteristics = blocker_object.effective_characteristics();
        let is_artifact = blocker_characteristics
            .card_types
            .contains(&CardType::Artifact);
        let is_black = blocker_characteristics.colors.contains(&ManaColor::Black);
        if !is_artifact && !is_black {
            return Ok(false);
        }
    }
    if attacker_object
        .landwalk_instances
        .keys()
        .copied()
        .any(|quality| defending_player_controls_land_quality(state, defending_player, quality))
    {
        return Ok(false);
    }
    let attacker_flying = attacker_object
        .combat_keywords
        .contains(&CombatKeyword::Flying);
    let blocker_reaches_flying = blocker_object
        .combat_keywords
        .iter()
        .any(|keyword| matches!(keyword, CombatKeyword::Flying | CombatKeyword::Reach));
    Ok(!attacker_flying || blocker_reaches_flying)
}

fn defending_player_controls_land_quality(
    state: &KeywordGameState,
    defending_player: PlayerId,
    quality: LandwalkQuality,
) -> bool {
    state.objects.values().any(|object| {
        if object.zone != Zone::Battlefield || object.controller != defending_player {
            return false;
        }
        let characteristics = object.effective_characteristics();
        if !characteristics.card_types.contains(&CardType::Land) {
            return false;
        }
        match quality {
            LandwalkQuality::Plains => characteristics
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("Plains")),
            LandwalkQuality::Island => characteristics
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("Island")),
            LandwalkQuality::Swamp => characteristics
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("Swamp")),
            LandwalkQuality::Mountain => characteristics
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("Mountain")),
            LandwalkQuality::Forest => characteristics
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("Forest")),
            LandwalkQuality::Desert => characteristics
                .subtypes
                .iter()
                .any(|subtype| subtype.eq_ignore_ascii_case("Desert")),
            LandwalkQuality::LegendaryLand => characteristics
                .supertypes
                .iter()
                .any(|supertype| supertype.eq_ignore_ascii_case("Legendary")),
            LandwalkQuality::NonbasicLand => !characteristics
                .supertypes
                .iter()
                .any(|supertype| supertype.eq_ignore_ascii_case("Basic")),
            LandwalkQuality::SnowLand => characteristics
                .supertypes
                .iter()
                .any(|supertype| supertype.eq_ignore_ascii_case("Snow")),
        }
    })
}

pub fn resolve_creature_destruction_state_based_action(
    state: &mut KeywordGameState,
    creature: ObjectId,
    regeneration_choice: Option<RegenerationChoice>,
) -> Result<CreatureDestructionStateBasedOutcome, KeywordExecutionError> {
    let before = state.clone();
    let result = (|| {
        let object = state.object(creature)?;
        if object.zone != Zone::Battlefield {
            return Err(KeywordExecutionError::WrongZone {
                object: creature,
                expected: Zone::Battlefield,
                actual: object.zone,
            });
        }
        if !object.is_creature() {
            return Err(KeywordExecutionError::NotCreature(creature));
        }
        let toughness = object.effective_characteristics().toughness.unwrap_or(0);
        let lethal_damage =
            toughness > 0 && object.damage_marked >= u32::try_from(toughness).unwrap_or(u32::MAX);
        let deathtouch_damage = toughness > 0 && object.damaged_by_deathtouch_since_state_check;
        if !lethal_damage && !deathtouch_damage {
            state
                .object_mut(creature)?
                .damaged_by_deathtouch_since_state_check = false;
            return Ok(CreatureDestructionStateBasedOutcome::NoDestruction);
        }
        if object
            .combat_keywords
            .contains(&CombatKeyword::Indestructible)
        {
            state
                .object_mut(creature)?
                .damaged_by_deathtouch_since_state_check = false;
            return Ok(CreatureDestructionStateBasedOutcome::IgnoredByIndestructible);
        }
        let outcome = match resolve_destruction_inner(state, creature, regeneration_choice)? {
            KeywordEvidenceEvent::RegenerationReplacedDestruction { .. } => {
                CreatureDestructionStateBasedOutcome::Regenerated
            }
            KeywordEvidenceEvent::DestroyedWithoutRegeneration { .. } => {
                CreatureDestructionStateBasedOutcome::Destroyed
            }
            _ => unreachable!("destruction helper returned an unrelated event"),
        };
        state
            .object_mut(creature)?
            .damaged_by_deathtouch_since_state_check = false;
        Ok(outcome)
    })();
    if result.is_err() {
        *state = before;
    }
    result
}

pub fn resolve_destruction(
    state: &mut KeywordGameState,
    program: &KeywordProgram,
    permanent: ObjectId,
    choice: Option<RegenerationChoice>,
) -> Result<KeywordReceipt, KeywordExecutionError> {
    if program.runtime_version != KEYWORD_RULES_RUNTIME_VERSION {
        return Err(KeywordExecutionError::ActionProgramMismatch);
    }
    let KeywordProgramKind::Regenerate(regeneration) = &program.kind else {
        return Err(KeywordExecutionError::ActionProgramMismatch);
    };
    if regeneration.recipients != RegenerationRecipientScope::SourcePermanent {
        return Err(KeywordExecutionError::ActionProgramMismatch);
    }
    let before = state.clone();
    match resolve_destruction_inner(state, permanent, choice) {
        Ok(event) => Ok(receipt(program, vec![event])),
        Err(error) => {
            *state = before;
            Err(error)
        }
    }
}

fn resolve_destruction_inner(
    state: &mut KeywordGameState,
    permanent: ObjectId,
    choice: Option<RegenerationChoice>,
) -> Result<KeywordEvidenceEvent, KeywordExecutionError> {
    let object = state.object(permanent)?;
    if object.zone != Zone::Battlefield {
        return Err(KeywordExecutionError::WrongZone {
            object: permanent,
            expected: Zone::Battlefield,
            actual: object.zone,
        });
    }
    if object
        .combat_keywords
        .contains(&CombatKeyword::Indestructible)
    {
        return Err(KeywordExecutionError::IndestructibleRequiresOwnContract);
    }
    let has_one_shot = object.regeneration_shields > 0;
    let has_static = object.static_regeneration;
    let selected = match (has_one_shot, has_static, choice) {
        (false, false, None) => None,
        (true, false, None | Some(RegenerationChoice::OneShotReplacement)) => {
            Some(RegenerationChoice::OneShotReplacement)
        }
        (false, true, None | Some(RegenerationChoice::StaticReplacement)) => {
            Some(RegenerationChoice::StaticReplacement)
        }
        (true, true, Some(selection)) => Some(selection),
        _ => return Err(KeywordExecutionError::InvalidRegenerationChoice),
    };
    if let Some(selected) = selected {
        let object = state.object_mut(permanent)?;
        let removed_damage = object.damage_marked;
        let removed_from_combat = object.attacking || object.blocking;
        object.damage_marked = 0;
        object.tapped = true;
        object.attacking = false;
        object.blocking = false;
        if selected == RegenerationChoice::OneShotReplacement {
            object.regeneration_shields -= 1;
        }
        return Ok(KeywordEvidenceEvent::RegenerationReplacedDestruction {
            permanent,
            removed_damage,
            tapped: true,
            removed_from_combat,
            remaining_one_shot_replacements: object.regeneration_shields,
        });
    }
    state.move_object(permanent, Zone::Graveyard)?;
    Ok(KeywordEvidenceEvent::DestroyedWithoutRegeneration { permanent })
}

pub fn clear_end_of_turn_regeneration(state: &mut KeywordGameState) {
    for object in state.objects.values_mut() {
        object.regeneration_shields = 0;
    }
}

pub fn remove_static_regeneration(
    state: &mut KeywordGameState,
    permanent: ObjectId,
) -> Result<(), KeywordExecutionError> {
    state.object_mut(permanent)?.static_regeneration = false;
    Ok(())
}
