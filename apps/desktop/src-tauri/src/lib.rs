pub mod ability_clause_bridge;
mod ability_ir;
mod ability_program;
pub mod alternate_zone_cast_keyword_runtime;
mod alternative_cast_runtime;
mod analysis;
pub mod attachment_entry_runtime;
pub mod attachment_filter_runtime;
pub mod bounded_oracle_consumer;
pub mod bounded_oracle_mana;
pub mod bounded_oracle_runtime;
pub mod bounded_oracle_simulation;
mod cache;
mod card_data;
pub mod cast_choice_keyword_runtime;
pub mod cast_modifier_keyword_runtime;
mod characteristic_oracle_runtime;
pub(crate) mod combat_effects;
pub mod combat_restriction_runtime;
pub mod combat_special_keyword_runtime;
pub mod combat_terminal;
pub mod combat_trigger_keyword_runtime;
pub mod combo_data;
mod combo_store;
pub mod common_action_procedure_runtime;
mod comprehensive_rules;
mod continuous_trigger_runtime;
pub mod creature_counter_keyword_runtime;
pub mod damage_clause_compiler;
pub mod damage_transaction_runtime;
pub mod delayed_counter_keyword_runtime;
mod domain;
mod dynamic_characteristic_runtime;
pub mod early_turn_evaluator;
mod effects;
mod empty_library_win;
pub mod entry_choice_keyword_runtime;
mod equip_production_runtime;
pub mod execution_coverage;
pub mod extended_cast_zone_keyword_runtime;
pub mod face_down_merge_keyword_runtime;
mod face_layout_runtime;
pub mod graveyard_hand_library_keyword_runtime;
pub mod graveyard_transform_keyword_runtime;
mod importers;
mod interaction_runtime;
mod interaction_scenarios;
mod interference;
pub(crate) mod keyword_production_bridge;
pub mod keyword_rules_runtime;
mod land_runtime;
pub mod level_progression_runtime;
pub mod library_access_runtime;
pub mod linked_cast_cost_keyword_runtime;
mod mana;
mod mana_network_runtime;
pub mod mechanic_runtime;
mod object_lifecycle_runtime;
pub mod object_state_clause_runtime;
pub mod old_transform_runtime;
mod opponent_library;
pub mod oracle_action_algebra_runtime;
pub mod oracle_clause_backend;
pub mod oracle_clause_composition;
pub mod oracle_clause_syntax;
pub mod oracle_face_program_assembler;
mod parser;
mod policy_store;
pub mod pregame_clause_runtime;
mod printed_cost_runtime;
pub mod regeneration_action_runtime;
pub mod residual_cost_keyword_runtime;
mod restriction_protection_runtime;
pub mod rules;
mod rules_capabilities;
mod runtime_receipts;
pub mod saga_transform_runtime;
mod scoring;
mod semantic_store;
mod semantics;
mod simulation;
pub mod special_resource_runtime;
pub mod standalone_oracle_annotation;
pub mod static_special_keyword_runtime;
mod strategic_profile;
mod strict_engine;
pub mod targeting_protection_runtime;
pub(crate) mod turn_event_state;
pub mod turn_planner;
mod tutor_runtime;
mod utility_modal_runtime;

use std::collections::HashMap;
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{Manager, State};

use crate::card_data::CardRepository;
use crate::combo_store::{
    ComboStore, ComboStoreStatus, ComboUpdateOutcome, ComboUpdateProgress, ComboUpdateReporter,
};
use crate::comprehensive_rules::{
    ComprehensiveRulesStatus, ComprehensiveRulesStore, ComprehensiveRulesUpdateOutcome,
    ComprehensiveRulesUpdateProgress, ComprehensiveRulesUpdateReporter,
};
use crate::domain::{
    AnalysisProgress, AnalysisReport, AnalyzeRequest, DataStatus, DataUpdateProgress,
    DeckParseResult, ImportResult,
};
use crate::policy_store::{PolicyImportOutcome, PolicyPackageStatus, PolicyStore};
use crate::semantic_store::{SemanticImportOutcome, SemanticPackageStatus, SemanticStore};

struct AppState {
    card_database_path: PathBuf,
    analysis_cache_path: PathBuf,
    combo_store: ComboStore,
    comprehensive_rules_store: ComprehensiveRulesStore,
    policy_store: PolicyStore,
    semantic_store: SemanticStore,
    active_runs: Mutex<HashMap<String, Arc<AtomicBool>>>,
    data_access_lock: tokio::sync::RwLock<()>,
    combo_access_lock: tokio::sync::RwLock<()>,
    comprehensive_rules_access_lock: tokio::sync::RwLock<()>,
    policy_access_lock: tokio::sync::RwLock<()>,
    semantic_access_lock: tokio::sync::RwLock<()>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeUpdateCheckItem {
    id: String,
    label: String,
    update_available: bool,
    installed_version: Option<String>,
    available_version: Option<String>,
    detail: String,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeUpdateCheck {
    checked_at: String,
    update_available: bool,
    items: Vec<KnowledgeUpdateCheckItem>,
}

#[tauri::command]
fn parse_decklist(deck_text: String) -> DeckParseResult {
    parser::parse_decklist(&deck_text)
}

const EXTERNAL_CREDIT_URLS: &[&str] = &[
    "https://magic.wizards.com/en/rules",
    "https://magic.wizards.com/en/formats/commander",
    "https://scryfall.com/",
    "https://scryfall.com/docs/api/bulk-data",
    "https://commanderspellbook.com/",
    "https://archidekt.com/",
    "https://deckstats.net/",
    "https://moxfield.com/",
];

fn external_credit_url_allowed(url: &str) -> bool {
    EXTERNAL_CREDIT_URLS.contains(&url)
}

#[tauri::command]
fn open_external_credit_url(url: String) -> Result<(), String> {
    if !external_credit_url_allowed(&url) {
        return Err("That external credit URL is not in the application allow-list.".into());
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer.exe")
            .arg(&url)
            .spawn()
            .map_err(|error| format!("The system browser could not be opened: {error}"))?;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = url;
        Err("External credit links are currently supported only by the Windows application.".into())
    }
}

#[tauri::command]
async fn import_deck_url(url: String) -> Result<ImportResult, String> {
    importers::import_deck_url(&url)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn read_deck_file(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err("Choose an absolute file path.".into());
    }
    let metadata =
        std::fs::metadata(&path).map_err(|error| format!("Could not open the file: {error}"))?;
    if metadata.len() > 5 * 1024 * 1024 {
        return Err("Deck files must be smaller than 5 MB.".into());
    }
    let bytes =
        std::fs::read(&path).map_err(|error| format!("Could not read the file: {error}"))?;
    decode_text_file(&bytes)
}

#[tauri::command]
fn write_text_file(path: String, contents: String) -> Result<(), String> {
    const MAXIMUM_REPORT_BYTES: usize = 10 * 1024 * 1024;
    if contents.len() > MAXIMUM_REPORT_BYTES {
        return Err("Reports must be smaller than 10 MB.".into());
    }
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err("Choose an absolute report path.".into());
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if extension.as_deref() != Some("md") {
        return Err("Reports can be saved only as Markdown files.".into());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "The selected report path has no parent directory.".to_string())?;
    if !parent.is_dir() {
        return Err("The selected report directory does not exist.".into());
    }
    std::fs::write(&path, contents).map_err(|error| format!("Could not save the report: {error}"))
}

#[tauri::command]
fn get_data_status(state: State<'_, AppState>) -> Result<DataStatus, String> {
    CardRepository::new(&state.card_database_path)
        .and_then(|repository| repository.status())
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn check_for_knowledge_updates(
    state: State<'_, AppState>,
) -> Result<KnowledgeUpdateCheck, String> {
    let card_check = async {
        let _lease = state.data_access_lock.read().await;
        let repository = CardRepository::for_update_check(&state.card_database_path);
        let result = repository.check_for_update().await;
        match result {
            Ok(check) => KnowledgeUpdateCheckItem {
                id: "cardData".into(),
                label: "Oracle card definitions".into(),
                update_available: check.update_available,
                installed_version: check.installed_version,
                available_version: check.available_version,
                detail: check.detail,
                error: None,
            },
            Err(error) => {
                knowledge_update_error("cardData", "Oracle card definitions", error.to_string())
            }
        }
    };
    let combo_check = async {
        let _lease = state.combo_access_lock.read().await;
        match state.combo_store.check_for_update().await {
            Ok(check) => KnowledgeUpdateCheckItem {
                id: "comboData".into(),
                label: "Commander Spellbook combinations".into(),
                update_available: check.update_available,
                installed_version: check.installed_version,
                available_version: check.available_version,
                detail: check.detail,
                error: None,
            },
            Err(error) => knowledge_update_error(
                "comboData",
                "Commander Spellbook combinations",
                error.to_string(),
            ),
        }
    };
    let rules_check = async {
        let _lease = state.comprehensive_rules_access_lock.read().await;
        match state.comprehensive_rules_store.check_for_update().await {
            Ok(check) => KnowledgeUpdateCheckItem {
                id: "comprehensiveRules".into(),
                label: "Comprehensive Rules".into(),
                update_available: check.update_available,
                installed_version: check.installed_version,
                available_version: check.available_version,
                detail: check.detail,
                error: None,
            },
            Err(error) => knowledge_update_error(
                "comprehensiveRules",
                "Comprehensive Rules",
                error.to_string(),
            ),
        }
    };
    let (card, combo, rules) = tokio::join!(card_check, combo_check, rules_check);
    let items = vec![card, combo, rules];
    Ok(KnowledgeUpdateCheck {
        checked_at: Utc::now().to_rfc3339(),
        update_available: items.iter().any(|item| item.update_available),
        items,
    })
}

fn knowledge_update_error(id: &str, label: &str, error: String) -> KnowledgeUpdateCheckItem {
    KnowledgeUpdateCheckItem {
        id: id.into(),
        label: label.into(),
        update_available: false,
        installed_version: None,
        available_version: None,
        detail: format!("{label} could not be checked."),
        error: Some(error),
    }
}

#[tauri::command]
async fn update_card_database(
    state: State<'_, AppState>,
    on_progress: Channel<DataUpdateProgress>,
) -> Result<DataStatus, String> {
    let _update_guard = state.data_access_lock.write().await;
    let repository =
        CardRepository::new(&state.card_database_path).map_err(|error| error.to_string())?;
    repository
        .install_full_snapshot(|progress| {
            let _ = on_progress.send(progress);
        })
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_combo_data_status(state: State<'_, AppState>) -> Result<ComboStoreStatus, String> {
    state
        .combo_store
        .status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn update_combo_database(
    state: State<'_, AppState>,
    on_progress: Channel<ComboUpdateProgress>,
) -> Result<ComboUpdateOutcome, String> {
    let _update_guard = state.combo_access_lock.write().await;
    let reporter: ComboUpdateReporter = Arc::new(move |progress| {
        let _ = on_progress.send(progress);
    });
    state
        .combo_store
        .update_from_network(Some(reporter))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn get_comprehensive_rules_status(
    state: State<'_, AppState>,
) -> Result<ComprehensiveRulesStatus, String> {
    let _lease = state.comprehensive_rules_access_lock.read().await;
    state
        .comprehensive_rules_store
        .status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn update_comprehensive_rules(
    state: State<'_, AppState>,
    on_progress: Channel<ComprehensiveRulesUpdateProgress>,
) -> Result<ComprehensiveRulesUpdateOutcome, String> {
    let _update_guard = state.comprehensive_rules_access_lock.write().await;
    let reporter: ComprehensiveRulesUpdateReporter = Arc::new(move |progress| {
        let _ = on_progress.send(progress);
    });
    state
        .comprehensive_rules_store
        .update_from_network(Some(reporter))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn get_policy_package_status(
    state: State<'_, AppState>,
) -> Result<PolicyPackageStatus, String> {
    let _lease = state.policy_access_lock.read().await;
    state
        .policy_store
        .status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn import_policy_package(
    state: State<'_, AppState>,
    path: String,
) -> Result<PolicyImportOutcome, String> {
    let _activation_guard = state.policy_access_lock.write().await;
    let store = state.policy_store.clone();
    tokio::task::spawn_blocking(move || store.import_local_file(&PathBuf::from(path)))
        .await
        .map_err(|error| format!("Policy import worker stopped unexpectedly: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn reset_policy_package(state: State<'_, AppState>) -> Result<PolicyImportOutcome, String> {
    let _activation_guard = state.policy_access_lock.write().await;
    let store = state.policy_store.clone();
    tokio::task::spawn_blocking(move || store.reset_to_bundled())
        .await
        .map_err(|error| format!("Policy reset worker stopped unexpectedly: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn get_semantic_package_status(
    state: State<'_, AppState>,
) -> Result<SemanticPackageStatus, String> {
    let _lease = state.semantic_access_lock.read().await;
    state
        .semantic_store
        .status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn import_semantic_package(
    state: State<'_, AppState>,
    path: String,
) -> Result<SemanticImportOutcome, String> {
    let _activation_guard = state.semantic_access_lock.write().await;
    let store = state.semantic_store.clone();
    tokio::task::spawn_blocking(move || store.import_local_file(&PathBuf::from(path)))
        .await
        .map_err(|error| format!("Semantic-package import worker stopped unexpectedly: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn reset_semantic_package(
    state: State<'_, AppState>,
) -> Result<SemanticImportOutcome, String> {
    let _activation_guard = state.semantic_access_lock.write().await;
    let store = state.semantic_store.clone();
    tokio::task::spawn_blocking(move || store.reset_to_bundled())
        .await
        .map_err(|error| format!("Semantic-package reset worker stopped unexpectedly: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn analyze_deck(
    state: State<'_, AppState>,
    request: AnalyzeRequest,
    on_progress: Channel<AnalysisProgress>,
) -> Result<AnalysisReport, String> {
    let cancellation = Arc::new(AtomicBool::new(false));
    let run_id = request.run_id.clone();
    {
        let mut active_runs = state
            .active_runs
            .lock()
            .map_err(|_| "Analysis state lock was poisoned.".to_string())?;
        if let Some(previous) = active_runs.insert(request.run_id.clone(), cancellation.clone()) {
            previous.store(true, Ordering::Relaxed);
        }
    }

    let result = async {
        // Snapshot updates swap SQLite files. Shared leases keep definitions,
        // matches, report provenance, and cache fingerprints on one generation.
        // Registration happens first so a queued run can still be cancelled.
        let _data_guard = state.data_access_lock.read().await;
        let _combo_guard = state.combo_access_lock.read().await;
        let _comprehensive_rules_guard = state.comprehensive_rules_access_lock.read().await;
        let _policy_guard = state.policy_access_lock.read().await;
        let _semantic_guard = state.semantic_access_lock.read().await;
        if cancellation.load(Ordering::Relaxed) {
            return Err("Analysis cancelled.".into());
        }

        let repository =
            CardRepository::new(&state.card_database_path).map_err(|error| error.to_string())?;
        let policy_package = state
            .policy_store
            .load_active()
            .map_err(|error| error.to_string())?;
        let semantic_package = state
            .semantic_store
            .load_active()
            .map_err(|error| error.to_string())?;
        let comprehensive_rules = state
            .comprehensive_rules_store
            .load_active()
            .map_err(|error| error.to_string())?;
        let cache = cache::AnalysisCache::new(&state.analysis_cache_path).ok();
        let channel = Arc::new(on_progress);
        let reporter = {
            let channel = channel.clone();
            move |progress| {
                let _ = channel.send(progress);
            }
        };
        analysis::analyze(
            repository,
            state.combo_store.clone(),
            analysis::AnalysisSnapshots {
                policy: policy_package,
                semantics: semantic_package,
                comprehensive_rules,
            },
            cache,
            request,
            cancellation,
            reporter,
        )
        .await
        .map_err(|error| error.to_string())
    }
    .await;

    if let Ok(mut active_runs) = state.active_runs.lock() {
        active_runs.remove(&run_id);
    }
    result
}

#[tauri::command]
fn cancel_analysis(state: State<'_, AppState>, run_id: String) -> bool {
    let Ok(active_runs) = state.active_runs.lock() else {
        return false;
    };
    if let Some(cancellation) = active_runs.get(&run_id) {
        cancellation.store(true, Ordering::Relaxed);
        true
    } else {
        false
    }
}

fn decode_text_file(bytes: &[u8]) -> Result<String, String> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let words = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16(&words)
            .map_err(|_| "The selected UTF-16 file contains invalid text.".into());
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let words = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16(&words)
            .map_err(|_| "The selected UTF-16 file contains invalid text.".into());
    }
    let utf8 = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    String::from_utf8(utf8.to_vec())
        .map_err(|_| "The selected file is not valid UTF-8 or UTF-16 text.".into())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_directory = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&data_directory)?;
            let card_database_path = data_directory.join("card-data").join("cards.sqlite");
            let analysis_cache_path = data_directory.join("analysis-cache").join("reports.sqlite");
            let combo_store =
                ComboStore::new(data_directory.join("combo-data").join("combos.sqlite"))
                    .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            let comprehensive_rules_store =
                ComprehensiveRulesStore::new(data_directory.join("comprehensive-rules"))
                    .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            let policy_store = PolicyStore::new(data_directory.join("policy-data"))
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            let semantic_store = SemanticStore::new(data_directory.join("semantic-data"))
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            // Initialize immediately so the UI can report a deterministic first-launch state.
            CardRepository::new(&card_database_path)
                .map_err(|error| Box::<dyn std::error::Error>::from(error.to_string()))?;
            app.manage(AppState {
                card_database_path,
                analysis_cache_path,
                combo_store,
                comprehensive_rules_store,
                policy_store,
                semantic_store,
                active_runs: Mutex::new(HashMap::new()),
                data_access_lock: tokio::sync::RwLock::new(()),
                combo_access_lock: tokio::sync::RwLock::new(()),
                comprehensive_rules_access_lock: tokio::sync::RwLock::new(()),
                policy_access_lock: tokio::sync::RwLock::new(()),
                semantic_access_lock: tokio::sync::RwLock::new(()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            parse_decklist,
            open_external_credit_url,
            import_deck_url,
            read_deck_file,
            write_text_file,
            get_data_status,
            check_for_knowledge_updates,
            update_card_database,
            get_combo_data_status,
            update_combo_database,
            get_comprehensive_rules_status,
            update_comprehensive_rules,
            get_policy_package_status,
            import_policy_package,
            reset_policy_package,
            get_semantic_package_status,
            import_semantic_package,
            reset_semantic_package,
            analyze_deck,
            cancel_analysis
        ])
        .run(tauri::generate_context!())
        .expect("error while running Commander Deck Analyzer");
}
