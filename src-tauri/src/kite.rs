use std::collections::HashMap;
#[cfg(target_os = "windows")]
use std::fs;
#[cfg(target_os = "windows")]
use std::fs::File;
#[cfg(target_os = "windows")]
use std::io::{self, Read, Write};
#[cfg(target_os = "windows")]
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(target_os = "windows")]
use flate2::read::GzDecoder;
use reqwest::{
    header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE},
    Client, Method, StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[cfg(target_os = "windows")]
use sha2::{Digest, Sha256};
#[cfg(target_os = "windows")]
use tar::Archive;
use tauri::{ipc::Channel, AppHandle, State};
use tokio::sync::oneshot;
use uuid::Uuid;
#[cfg(target_os = "windows")]
use zip::ZipArchive;

const KITE_MCP_URL_KEY: &str = "kite_mcp_url";
const KITE_SESSION_ID_KEY: &str = "kite_mcp_session_id";
const KITE_AUTH_STATE_KEY: &str = "kite_auth_state";
const KITE_LAST_PAYER_KEY: &str = "kite_last_payer_addr";
const KITE_SIGNUP_EMAIL_KEY: &str = "kite_signup_email";
const KITE_PENDING_SIGNUP_ID_KEY: &str = "kite_pending_signup_id";
const KITE_PENDING_LOGIN_ID_KEY: &str = "kite_pending_login_id";
const KITE_ACTIVE_SPENDING_SESSION_KEY: &str = "kite_active_spending_session_id";

const KITE_CLI_BASE_URL: &str = "https://cli.gokite.ai";
const KITE_DOCS_URL: &str = "https://docs.gokite.ai/kite-agent-passport/developer-guide";
const KITE_PORTAL_URL: &str = "https://x402-portal-eight.vercel.app/";
const KITE_INSTALLER_URL: &str = "https://cli.gokite.ai/install.sh";
const KITE_TESTNET_MCP_URL: &str = "https://neo.dev.gokite.ai/v1/mcp";
const MCP_PROTOCOL_VERSION: &str = "2025-03-26";
const KITE_AGENT_TYPE: &str = "codex";

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KiteAuthState {
    Ready,
    Unverified,
    MissingMcpUrl,
    CliMissing,
    AuthRequired,
    SessionCreationRequired,
    SessionExpired,
    InsufficientBudget,
    Unauthorized,
    AgentNotFound,
    InvalidPaymentResponse,
    NetworkError,
    UnknownError,
}

impl KiteAuthState {
    fn as_storage_str(&self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Unverified => "unverified",
            Self::MissingMcpUrl => "missing_mcp_url",
            Self::CliMissing => "cli_missing",
            Self::AuthRequired => "auth_required",
            Self::SessionCreationRequired => "session_creation_required",
            Self::SessionExpired => "session_expired",
            Self::InsufficientBudget => "insufficient_budget",
            Self::Unauthorized => "unauthorized",
            Self::AgentNotFound => "agent_not_found",
            Self::InvalidPaymentResponse => "invalid_payment_response",
            Self::NetworkError => "network_error",
            Self::UnknownError => "unknown_error",
        }
    }

    fn from_storage(raw: &str) -> Self {
        match raw.trim() {
            "ready" => Self::Ready,
            "missing_mcp_url" => Self::MissingMcpUrl,
            "cli_missing" => Self::CliMissing,
            "auth_required" => Self::AuthRequired,
            "session_creation_required" => Self::SessionCreationRequired,
            "session_expired" => Self::SessionExpired,
            "insufficient_budget" => Self::InsufficientBudget,
            "unauthorized" => Self::Unauthorized,
            "agent_not_found" => Self::AgentNotFound,
            "invalid_payment_response" => Self::InvalidPaymentResponse,
            "network_error" => Self::NetworkError,
            "unknown_error" => Self::UnknownError,
            _ => Self::Unverified,
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteSetupStatus {
    pub cli_installed: bool,
    pub cli_path: Option<String>,
    pub mcp_url_configured: bool,
    pub masked_mcp_url: Option<String>,
    pub auth_state: KiteAuthState,
    pub connected: bool,
    pub last_payer_addr: Option<String>,
    pub signup_email: Option<String>,
    pub pending_signup_id: Option<String>,
    pub invite_only: bool,
    pub docs_url: String,
    pub portal_url: String,
    pub installer_url: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteVerifyResponse {
    pub connected: bool,
    pub auth_state: KiteAuthState,
    pub available_tools: Vec<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteAgentCapability {
    pub available: bool,
    pub mode: String,
    pub provider: String,
    pub model: Option<String>,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteAccountSummary {
    pub logged_in: bool,
    pub email: Option<String>,
    pub user_id: Option<String>,
    pub signup_email: Option<String>,
    pub pending_signup_id: Option<String>,
    pub pending_login_id: Option<String>,
    pub current_agent_identity: Option<String>,
    pub auth_state: KiteAuthState,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteWalletAsset {
    pub symbol: String,
    pub balance: String,
    pub native: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteWalletSummary {
    pub wallet_address: String,
    pub wallet_type: String,
    pub chain_id: i64,
    pub assets: Vec<KiteWalletAsset>,
    pub can_use_faucet: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteWalletTransfer {
    pub wallet_address: String,
    pub recipient_address: String,
    pub asset: String,
    pub amount: String,
    pub transaction_hash: String,
    pub chain_id: i64,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteSessionSummary {
    pub id: String,
    pub status: String,
    pub agent_type: String,
    pub expires_at: Option<String>,
    pub task_summary: Option<String>,
    pub assets: Vec<String>,
    pub max_amount_per_tx: Option<String>,
    pub max_total_amount: Option<String>,
    pub spent_total: Option<String>,
    pub reserved_total: Option<String>,
    pub selected: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteSessionRequestStatus {
    pub request_id: String,
    pub status: String,
    pub session_id: Option<String>,
    pub approval_url: Option<String>,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteActivityEvent {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub occurred_at: String,
    pub amount_display: Option<String>,
    pub chain_name: Option<String>,
    pub tx_hash: Option<String>,
    pub order_id: Option<String>,
    pub item_titles: Vec<String>,
    pub error_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteShopItem {
    pub provider: String,
    pub external_identifier: String,
    pub title: String,
    pub price: String,
    pub rating: Option<String>,
    pub reviews: Option<String>,
    pub link: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteCartItem {
    pub provider: String,
    pub external_identifier: String,
    pub product_locator: String,
    pub title: String,
    pub price: String,
    pub quantity: i64,
    pub link: Option<String>,
    pub thumbnail: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteShippingSummary {
    pub name: Option<String>,
    pub email: Option<String>,
    pub line1: Option<String>,
    pub line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: Option<String>,
    pub complete: bool,
    pub missing: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteCartSummary {
    pub items: Vec<KiteCartItem>,
    pub item_count: usize,
    pub payment_currency: Option<String>,
    pub payment_chain: Option<String>,
    pub shipping: Option<KiteShippingSummary>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteOrderSummary {
    pub order_id: String,
    pub phase: Option<String>,
    pub payment_status: Option<String>,
    pub tx_hash: Option<String>,
    pub currency: Option<String>,
    pub chain: Option<String>,
    pub delivery_status: Option<String>,
    pub title: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteDeveloperSummary {
    pub auth_state: KiteAuthState,
    pub connected: bool,
    pub masked_mcp_url: Option<String>,
    pub last_payer_addr: Option<String>,
    pub current_session_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KiteHubState {
    pub setup: KiteSetupStatus,
    pub account: KiteAccountSummary,
    pub wallet: Option<KiteWalletSummary>,
    pub sessions: Vec<KiteSessionSummary>,
    pub activity: Vec<KiteActivityEvent>,
    pub cart: Option<KiteCartSummary>,
    pub orders: Vec<KiteOrderSummary>,
    pub developer: KiteDeveloperSummary,
    pub issues: Vec<String>,
}

pub struct KiteRuntimeState {
    pending_payment_id: Mutex<Option<String>>,
    pending_payment_tx: Mutex<Option<oneshot::Sender<bool>>>,
}

impl KiteRuntimeState {
    pub fn new() -> Self {
        Self {
            pending_payment_id: Mutex::new(None),
            pending_payment_tx: Mutex::new(None),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum KiteEvent {
    CheckingCli,
    InstallingCli,
    InstallingAgentPassport,
    OpeningPortal { target: String },
    WaitingForMcpConfig,
    VerifyingConnection,
    FetchingPayer,
    ApprovingPayment,
    RetryingPaidRequest,
    EnteringAgentMode { reason: String },
    AdvisoryFallback { reason: String, guidance: String },
    AwaitingSensitiveValue { field: String, instructions: String },
    AwaitingPaymentConfirmation { action_id: String, summary: String },
    ResumingAfterUserStep { step: String },
    Token(String),
    Done,
    Error(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum KiteCommand {
    Help,
    Setup {
        email: Option<String>,
        code: Option<String>,
    },
    Login {
        email: Option<String>,
        code: Option<String>,
    },
    Logout,
    Me,
    Connect,
    Status,
    Wallet,
    Send {
        to: String,
        amount: String,
        asset: String,
    },
    Faucet {
        token: String,
    },
    Sessions,
    SessionCreate {
        max_amount_per_tx: String,
        ttl: String,
        max_total_amount: Option<String>,
        assets: Option<String>,
        payment_approach: Option<String>,
        task_summary: Option<String>,
    },
    SessionUse {
        session_id: String,
    },
    SessionStatus {
        request_id: String,
        wait: bool,
    },
    Activity {
        kind: Option<String>,
    },
    ShopSearch {
        query: String,
    },
    Cart,
    Checkout {
        confirmed: bool,
    },
    Orders {
        order_id: Option<String>,
    },
    Payer,
    Approve {
        payee_addr: String,
        amount: String,
        token_type: String,
        merchant_name: Option<String>,
    },
    Call {
        url: String,
        method: String,
        body: Option<String>,
        merchant_name: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum KiteError {
    #[allow(dead_code)]
    CliMissing,
    CliInstallFailed(String),
    MissingMcpUrl,
    AuthRequired(String),
    SessionCreationRequired(String),
    SessionExpired(String),
    InsufficientBudget(String),
    Unauthorized(String),
    AgentNotFound(String),
    InvalidPaymentResponse(String),
    Network(String),
    BadInput(String),
    Unknown(String),
}

impl KiteError {
    fn auth_state(&self) -> KiteAuthState {
        match self {
            Self::CliMissing => KiteAuthState::CliMissing,
            Self::CliInstallFailed(_) => KiteAuthState::CliMissing,
            Self::MissingMcpUrl => KiteAuthState::MissingMcpUrl,
            Self::AuthRequired(_) => KiteAuthState::AuthRequired,
            Self::SessionCreationRequired(_) => KiteAuthState::SessionCreationRequired,
            Self::SessionExpired(_) => KiteAuthState::SessionExpired,
            Self::InsufficientBudget(_) => KiteAuthState::InsufficientBudget,
            Self::Unauthorized(_) => KiteAuthState::Unauthorized,
            Self::AgentNotFound(_) => KiteAuthState::AgentNotFound,
            Self::InvalidPaymentResponse(_) => KiteAuthState::InvalidPaymentResponse,
            Self::Network(_) => KiteAuthState::NetworkError,
            Self::BadInput(_) | Self::Unknown(_) => KiteAuthState::UnknownError,
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::CliMissing => "Kite CLI was not found on this machine.",
            Self::CliInstallFailed(msg) => msg,
            Self::MissingMcpUrl => {
                "No Kite MCP URL is saved yet. Paste the portal-provided URL into Settings first."
            }
            Self::AuthRequired(msg)
            | Self::SessionCreationRequired(msg)
            | Self::SessionExpired(msg)
            | Self::InsufficientBudget(msg)
            | Self::Unauthorized(msg)
            | Self::AgentNotFound(msg)
            | Self::InvalidPaymentResponse(msg)
            | Self::Network(msg)
            | Self::BadInput(msg)
            | Self::Unknown(msg) => msg,
        }
    }
}

#[derive(Clone, Debug)]
struct KiteSecrets {
    mcp_url: String,
    session_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct JsonRpcEnvelope {
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Clone, Debug, Deserialize)]
struct JsonRpcError {
    #[serde(default)]
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PaymentRequest {
    payee_addr: String,
    amount: String,
    token_type: String,
    merchant_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PaymentAuthorization {
    x_payment: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct X402CallResult {
    status: u16,
    body: String,
    payment: PaymentRequest,
}

#[derive(Clone, Debug, Deserialize)]
struct KiteBundleManifest {
    cli: KitePlatformBundle,
    #[serde(default)]
    ksearch: Option<KitePlatformBundle>,
    skills: KiteArchiveBundle,
}

#[derive(Clone, Debug, Deserialize)]
struct KitePlatformBundle {
    version: String,
    platforms: HashMap<String, KitePlatformArtifact>,
}

#[derive(Clone, Debug, Deserialize)]
struct KitePlatformArtifact {
    archive: String,
    #[serde(default)]
    checksum: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct KiteArchiveBundle {
    version: String,
    archive: String,
    #[serde(default)]
    checksum: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KiteCliInstallOutcome {
    cli_path: PathBuf,
    path_updated: bool,
    skills_bootstrapped: bool,
    notes: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct KiteSetupRequest {
    email: Option<String>,
    code: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct KpassEnvelope {
    status: String,
    #[serde(default)]
    hint: String,
    #[serde(default)]
    error: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct KpassMeResponse {
    user_id: String,
    email: String,
    status: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct KpassSignupInitResponse {
    signup_id: String,
    status: String,
    #[serde(default)]
    hint: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct KpassSignupExchangeResponse {
    user_id: String,
    email: String,
    status: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct KpassLoginInitResponse {
    login_id: String,
    status: String,
    #[serde(default)]
    hint: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct KpassWalletBalanceResponse {
    wallet_address: String,
    wallet_type: String,
    chain_id: i64,
    #[serde(default)]
    assets: Vec<KiteWalletAsset>,
    status: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct KpassWalletTransferResponse {
    wallet_address: String,
    recipient_address: String,
    asset: String,
    amount: String,
    transaction_hash: String,
    chain_id: i64,
    status: String,
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn get_kite_setup_status(
    db: State<'_, crate::history::Database>,
) -> Result<KiteSetupStatus, String> {
    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| e.to_string())?;
    let status = load_setup_status(&conn, detect_kite_cli());
    Ok(status)
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn set_kite_mcp_url(
    url: String,
    db: State<'_, crate::history::Database>,
) -> Result<(), String> {
    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| e.to_string())?;
    persist_mcp_url(&conn, &url).map_err(|e| e.to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn disconnect_kite(db: State<'_, crate::history::Database>) -> Result<(), String> {
    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| e.to_string())?;
    clear_kite_settings(&conn).map_err(|e| e.to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn get_kite_hub_state(
    db: State<'_, crate::history::Database>,
) -> Result<KiteHubState, String> {
    get_kite_hub_state_inner(db.inner()).map_err(|e| e.message().to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn kite_logout(db: State<'_, crate::history::Database>) -> Result<KiteAccountSummary, String> {
    let cli_path = detect_kite_cli().ok_or_else(|| KiteError::CliMissing.message().to_string())?;
    kite_logout_inner(db.inner(), &cli_path).map_err(|e| e.message().to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn kite_wallet_send(
    to: String,
    amount: String,
    asset: String,
    _db: State<'_, crate::history::Database>,
) -> Result<KiteWalletTransfer, String> {
    let cli_path = detect_kite_cli().ok_or_else(|| KiteError::CliMissing.message().to_string())?;
    kite_wallet_send_inner(&cli_path, &to, &amount, &asset)
        .map_err(|e| e.message().to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn kite_faucet_drop(
    token: String,
    _db: State<'_, crate::history::Database>,
) -> Result<KiteWalletTransfer, String> {
    let cli_path = detect_kite_cli().ok_or_else(|| KiteError::CliMissing.message().to_string())?;
    kite_faucet_drop_inner(&cli_path, &token).map_err(|e| e.message().to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn kite_use_session(
    session_id: String,
    db: State<'_, crate::history::Database>,
) -> Result<KiteSessionSummary, String> {
    let cli_path = detect_kite_cli().ok_or_else(|| KiteError::CliMissing.message().to_string())?;
    kite_use_session_inner(db.inner(), &cli_path, &session_id).map_err(|e| e.message().to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn kite_shop_search(
    query: String,
) -> Result<Vec<KiteShopItem>, String> {
    let cli_path = detect_kite_cli().ok_or_else(|| KiteError::CliMissing.message().to_string())?;
    kite_shop_search_inner(&cli_path, &query).map_err(|e| e.message().to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn kite_cart_add(
    provider: String,
    external_id: String,
    quantity: Option<i64>,
    _db: State<'_, crate::history::Database>,
) -> Result<KiteCartSummary, String> {
    let cli_path = detect_kite_cli().ok_or_else(|| KiteError::CliMissing.message().to_string())?;
    kite_cart_add_inner(&cli_path, &provider, &external_id, quantity.unwrap_or(1))
        .map_err(|e| e.message().to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn kite_cart_remove(
    provider: String,
    external_id: String,
    _db: State<'_, crate::history::Database>,
) -> Result<KiteCartSummary, String> {
    let cli_path = detect_kite_cli().ok_or_else(|| KiteError::CliMissing.message().to_string())?;
    kite_cart_remove_inner(&cli_path, &provider, &external_id)
        .map_err(|e| e.message().to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub async fn install_kite_cli() -> Result<String, String> {
    install_kite_cli_inner_async()
        .await
        .map(|outcome| outcome.cli_path.display().to_string())
        .map_err(|e| e.message().to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn open_kite_setup_target(target: String) -> Result<(), String> {
    let url =
        kite_target_url(&target).ok_or_else(|| format!("Unknown Kite setup target: {target}"))?;
    open_external_target(url)
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub async fn verify_kite_connection(
    client: State<'_, Client>,
    db: State<'_, crate::history::Database>,
) -> Result<KiteVerifyResponse, String> {
    verify_kite_connection_inner(client.inner(), db.inner())
        .await
        .map_err(|e| e.message().to_string())
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub async fn get_kite_agent_capability(
    client: State<'_, Client>,
    chat_provider: State<'_, crate::providers::SharedChatProvider>,
    db: State<'_, crate::history::Database>,
    agent_state: State<'_, Arc<crate::agent::AgentState>>,
    active_model: State<'_, crate::models::ActiveModelState>,
    config: State<'_, parking_lot::RwLock<crate::config::AppConfig>>,
) -> Result<KiteAgentCapability, String> {
    let config_snapshot = config.read().clone();
    kite_agent_capability_inner(
        client.inner(),
        chat_provider.inner(),
        db.inner(),
        agent_state.inner(),
        active_model.inner(),
        &config_snapshot,
    )
    .await
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub async fn start_kite_agent_mode(
    app_handle: AppHandle,
    input: String,
    reason: Option<String>,
    client: State<'_, Client>,
    chat_provider: State<'_, crate::providers::SharedChatProvider>,
    db: State<'_, crate::history::Database>,
    agent_state: State<'_, Arc<crate::agent::AgentState>>,
    active_model: State<'_, crate::models::ActiveModelState>,
    config: State<'_, parking_lot::RwLock<crate::config::AppConfig>>,
) -> Result<String, String> {
    let config_snapshot = config.read().clone();
    match start_kite_agent_mode_inner(
        &app_handle,
        &input,
        reason.as_deref(),
        client.inner(),
        chat_provider.inner(),
        db.inner(),
        agent_state.inner(),
        active_model.inner(),
        &config_snapshot,
    )
    .await
    {
        Ok(message) => Ok(message),
        Err(KiteError::BadInput(guidance)) => Ok(guidance),
        Err(err) => Err(err.message().to_string()),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn confirm_kite_payment_action(
    action_id: String,
    runtime: State<'_, Arc<KiteRuntimeState>>,
) -> Result<String, String> {
    resolve_pending_payment(runtime.inner(), &action_id, true)
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub fn reject_kite_payment_action(
    action_id: String,
    runtime: State<'_, Arc<KiteRuntimeState>>,
) -> Result<String, String> {
    resolve_pending_payment(runtime.inner(), &action_id, false)
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[cfg_attr(not(coverage), tauri::command)]
pub async fn run_kite_command(
    app_handle: AppHandle,
    input: String,
    on_event: Channel<KiteEvent>,
    client: State<'_, Client>,
    chat_provider: State<'_, crate::providers::SharedChatProvider>,
    db: State<'_, crate::history::Database>,
    runtime: State<'_, Arc<KiteRuntimeState>>,
    agent_state: State<'_, Arc<crate::agent::AgentState>>,
    active_model: State<'_, crate::models::ActiveModelState>,
    config: State<'_, parking_lot::RwLock<crate::config::AppConfig>>,
) -> Result<(), String> {
    let config_snapshot = config.read().clone();
    let mut emit = move |event| {
        let _ = on_event.send(event);
    };
    match run_kite_command_inner(
        &app_handle,
        input,
        client.inner(),
        chat_provider.inner(),
        db.inner(),
        runtime.inner(),
        agent_state.inner(),
        active_model.inner(),
        &config_snapshot,
        &mut emit,
    )
    .await
    {
        Ok(()) => emit(KiteEvent::Done),
        Err(err) => emit(KiteEvent::Error(err.message().to_string())),
    }
    Ok(())
}

async fn run_kite_command_inner(
    app_handle: &AppHandle,
    input: String,
    client: &Client,
    chat_provider: &crate::providers::SharedChatProvider,
    db: &crate::history::Database,
    runtime: &Arc<KiteRuntimeState>,
    agent_state: &Arc<crate::agent::AgentState>,
    active_model: &crate::models::ActiveModelState,
    config: &crate::config::AppConfig,
    emit: &mut impl FnMut(KiteEvent),
) -> Result<(), KiteError> {
    let command = parse_kite_command(&input)?;

    match command {
        KiteCommand::Help => {
            emit(KiteEvent::Token(kite_help_text()));
            Ok(())
        }
        KiteCommand::Setup { email, code } => {
            let status = match handle_kite_setup(db, KiteSetupRequest { email, code }, emit).await {
                Ok(status) => status,
                Err(err) if should_escalate_to_agent(&err) => {
                    maybe_escalate_kite_mode(
                        app_handle,
                        &input,
                        Some(err.message()),
                        client,
                        chat_provider,
                        db,
                        agent_state,
                        active_model,
                        config,
                        emit,
                    )
                    .await?;
                    return Ok(());
                }
                Err(err) => return Err(err),
            };
            if !status.mcp_url_configured {
                emit(KiteEvent::AwaitingSensitiveValue {
                    field: "kite_mcp_url".to_string(),
                    instructions: "Paste the portal-provided MCP URL in Settings > Agent > Kite Passport, then verify the connection.".to_string(),
                });
            }
            if !status.connected {
                maybe_escalate_kite_mode(
                    app_handle,
                    &input,
                    Some("Kite setup still needs help with portal navigation, MCP setup, or connection recovery."),
                    client,
                    chat_provider,
                    db,
                    agent_state,
                    active_model,
                    config,
                    emit,
                )
                .await?;
            }
            Ok(())
        }
        KiteCommand::Login { email, code } => {
            let show_summary = code.is_some();
            let account = handle_kite_login(
                db,
                KiteSetupRequest { email, code },
                emit,
            )?;
            if show_summary {
                emit(KiteEvent::Token(format_account_summary(&account)));
            }
            Ok(())
        }
        KiteCommand::Logout => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let account = kite_logout_inner(db, &cli_path)?;
            emit(KiteEvent::Token(format_account_summary(&account)));
            Ok(())
        }
        KiteCommand::Me => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let conn =
                db.0.lock()
                    .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
            let account = load_kite_account_summary(&conn, Some(&cli_path));
            drop(conn);
            emit(KiteEvent::Token(format_account_summary(&account)));
            Ok(())
        }
        KiteCommand::Connect | KiteCommand::Status => {
            emit(KiteEvent::VerifyingConnection);
            match verify_kite_connection_inner(client, db).await {
                Ok(response) => {
                    emit(KiteEvent::Token(format_verify_response(&response)));
                    if !response.connected {
                        maybe_escalate_kite_mode(
                            app_handle,
                            &input,
                            Some("Kite is connected but the required payment tools are missing or incomplete."),
                            client,
                            chat_provider,
                            db,
                            agent_state,
                            active_model,
                            config,
                            emit,
                        )
                        .await?;
                    }
                    Ok(())
                }
                Err(err) => {
                    if should_escalate_to_agent(&err) {
                        maybe_escalate_kite_mode(
                            app_handle,
                            &input,
                            Some(err.message()),
                            client,
                            chat_provider,
                            db,
                            agent_state,
                            active_model,
                            config,
                            emit,
                        )
                        .await
                    } else {
                        Err(err)
                    }
                }
            }
        }
        KiteCommand::Wallet => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let wallet = load_kite_wallet_summary(&cli_path)?;
            emit(KiteEvent::Token(format_wallet_summary(&wallet)));
            Ok(())
        }
        KiteCommand::Send { to, amount, asset } => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let transfer = kite_wallet_send_inner(&cli_path, &to, &amount, &asset)?;
            emit(KiteEvent::Token(format_wallet_transfer(&transfer, "Transfer complete")));
            Ok(())
        }
        KiteCommand::Faucet { token } => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let transfer = kite_faucet_drop_inner(&cli_path, &token)?;
            emit(KiteEvent::Token(format_wallet_transfer(&transfer, "Faucet drop complete")));
            Ok(())
        }
        KiteCommand::Sessions => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let conn =
                db.0.lock()
                    .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
            let selected = load_optional_setting(&conn, KITE_ACTIVE_SPENDING_SESSION_KEY);
            drop(conn);
            let sessions = load_kite_sessions(&cli_path, selected.as_deref())?;
            emit(KiteEvent::Token(format_sessions_summary(&sessions)));
            Ok(())
        }
        KiteCommand::SessionCreate {
            max_amount_per_tx,
            ttl,
            max_total_amount,
            assets,
            payment_approach,
            task_summary,
        } => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let status = create_kite_session(
                &cli_path,
                &max_amount_per_tx,
                &ttl,
                max_total_amount.as_deref(),
                assets.as_deref(),
                payment_approach.as_deref(),
                task_summary.as_deref(),
            )?;
            emit(KiteEvent::Token(format_session_request_status(&status)));
            Ok(())
        }
        KiteCommand::SessionUse { session_id } => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let session = kite_use_session_inner(db, &cli_path, &session_id)?;
            emit(KiteEvent::Token(format_sessions_summary(&[session])));
            Ok(())
        }
        KiteCommand::SessionStatus { request_id, wait } => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let status = load_kite_session_request_status(&cli_path, &request_id, wait)?;
            if let Some(session_id) = status.session_id.as_deref() {
                let _ = store_string_setting(db, KITE_ACTIVE_SPENDING_SESSION_KEY, session_id);
            }
            emit(KiteEvent::Token(format_session_request_status(&status)));
            Ok(())
        }
        KiteCommand::Activity { kind } => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let events = load_kite_activity_events(&cli_path, kind.as_deref(), 10)?;
            emit(KiteEvent::Token(format_activity_summary(&events)));
            Ok(())
        }
        KiteCommand::ShopSearch { query } => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let items = kite_shop_search_inner(&cli_path, &query)?;
            emit(KiteEvent::Token(format_shop_search_results(&query, &items)));
            Ok(())
        }
        KiteCommand::Cart => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            let cart = load_kite_cart_summary(&cli_path)?;
            emit(KiteEvent::Token(format_cart_summary(&cart)));
            Ok(())
        }
        KiteCommand::Checkout { confirmed } => {
            if !confirmed {
                emit(KiteEvent::Token(
                    "Kite checkout pauses for explicit confirmation.\n\nUse `/kite checkout --confirmed yes` only after you have reviewed the cart, shipping profile, and active spending session.".to_string(),
                ));
                return Ok(());
            }
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            run_kpass_json_value(
                &cli_path,
                &[
                    "shop:checkout",
                    "--confirmed",
                    "--output",
                    "json",
                    "--no-interactive",
                ],
                &[],
                "shopping checkout",
            )?;
            let events = load_kite_activity_events(&cli_path, Some("shopping_checkout"), 1)?;
            emit(KiteEvent::Token(format_activity_summary(&events)));
            Ok(())
        }
        KiteCommand::Orders { order_id } => {
            let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
            if let Some(order_id) = order_id {
                let order = load_kite_order_status(&cli_path, &order_id)?;
                emit(KiteEvent::Token(format_orders_summary(&[order])));
            } else {
                let events = load_kite_activity_events(&cli_path, Some("shopping_checkout"), 10)?;
                let orders = derive_order_summaries(&events);
                emit(KiteEvent::Token(format_orders_summary(&orders)));
            }
            Ok(())
        }
        KiteCommand::Payer => {
            emit(KiteEvent::FetchingPayer);
            let payer = match fetch_payer_address(client, db).await {
                Ok(payer) => payer,
                Err(err) if should_escalate_to_agent(&err) => {
                    return maybe_escalate_kite_mode(
                        app_handle,
                        &input,
                        Some(err.message()),
                        client,
                        chat_provider,
                        db,
                        agent_state,
                        active_model,
                        config,
                        emit,
                    )
                    .await;
                }
                Err(err) => return Err(err),
            };
            emit(KiteEvent::Token(format!(
                "Kite payer address\n{}\n\nFetched fresh from the saved MCP server.",
                payer
            )));
            Ok(())
        }
        KiteCommand::Approve {
            payee_addr,
            amount,
            token_type,
            merchant_name,
        } => {
            emit(KiteEvent::FetchingPayer);
            let payer_addr = match fetch_payer_address(client, db).await {
                Ok(payer_addr) => payer_addr,
                Err(err) if should_escalate_to_agent(&err) => {
                    return maybe_escalate_kite_mode(
                        app_handle,
                        &input,
                        Some(err.message()),
                        client,
                        chat_provider,
                        db,
                        agent_state,
                        active_model,
                        config,
                        emit,
                    )
                    .await;
                }
                Err(err) => return Err(err),
            };
            emit(KiteEvent::ApprovingPayment);
            let payment = PaymentRequest {
                payee_addr,
                amount,
                token_type,
                merchant_name,
            };
            request_payment_confirmation(
                runtime,
                emit,
                format!(
                    "Approve Kite payment?\nPayee: {}\nAmount: {} {}\nPayer: {}",
                    payment.payee_addr, payment.amount, payment.token_type, payer_addr
                ),
            )
            .await?;
            let auth = approve_payment(client, db, &payer_addr, &payment).await?;
            emit(KiteEvent::Token(format!(
                "Kite payment approved\nPayer: {}\nPayee: {}\nAmount: {} {}\n\nX-Payment\n{}",
                payer_addr, payment.payee_addr, payment.amount, payment.token_type, auth.x_payment
            )));
            Ok(())
        }
        KiteCommand::Call {
            url,
            method,
            body,
            merchant_name,
        } => {
            let result = match call_x402_service(
                client,
                db,
                runtime,
                &url,
                &method,
                body.as_deref(),
                merchant_name.clone(),
                emit,
            )
            .await
            {
                Ok(result) => result,
                Err(err) if should_escalate_to_agent(&err) => {
                    return maybe_escalate_kite_mode(
                        app_handle,
                        &input,
                        Some(err.message()),
                        client,
                        chat_provider,
                        db,
                        agent_state,
                        active_model,
                        config,
                        emit,
                    )
                    .await;
                }
                Err(err) => {
                    return Err(err);
                }
            };
            let payment_note = format!(
                "Paid {} {} to {}",
                result.payment.amount, result.payment.token_type, result.payment.payee_addr
            );
            let merchant_note = result
                .payment
                .merchant_name
                .as_ref()
                .map(|name| format!("\nMerchant: {name}"))
                .unwrap_or_default();
            emit(KiteEvent::Token(format!(
                "Kite x402 call complete\n{}\nStatus: {}\n{}\n\nResponse\n{}",
                payment_note, result.status, merchant_note, result.body
            )));
            Ok(())
        }
    }
}

async fn handle_kite_setup(
    db: &crate::history::Database,
    request: KiteSetupRequest,
    emit: &mut impl FnMut(KiteEvent),
) -> Result<KiteSetupStatus, KiteError> {
    emit(KiteEvent::CheckingCli);
    let mut installed_now = false;
    let mut install_outcome: Option<KiteCliInstallOutcome> = None;
    let cli = match detect_kite_cli() {
        Some(path) => Some(path),
        None => {
            emit(KiteEvent::InstallingCli);
            emit(KiteEvent::InstallingAgentPassport);
            let outcome = install_kite_cli_inner_async().await?;
            installed_now = true;
            let path = outcome.cli_path.clone();
            install_outcome = Some(outcome);
            Some(path)
        }
    };
    if !installed_now {
        if let Some(cli_path) = cli.as_ref() {
            let codex_skill_missing =
                kite_codex_skill_marker_path().is_some_and(|path| !path.exists());
            if codex_skill_missing {
                emit(KiteEvent::InstallingAgentPassport);
                let bootstrap = ensure_kite_agent_passport_bootstrap(cli_path)?;
                if bootstrap.path_updated
                    || bootstrap.skills_bootstrapped
                    || !bootstrap.notes.is_empty()
                {
                    install_outcome = Some(bootstrap);
                }
            }
        }
    }
    let cli_path = cli.ok_or_else(|| {
        KiteError::CliInstallFailed(
            "Thikra could not find Kite CLI after installation. Retry from Settings > Agent > Kite Passport.".to_string(),
        )
    })?;
    let current_user = load_current_kite_user(&cli_path)?;
    let saved_state = {
        let conn =
            db.0.lock()
                .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
        (
            load_optional_setting(&conn, KITE_SIGNUP_EMAIL_KEY),
            load_optional_setting(&conn, KITE_PENDING_SIGNUP_ID_KEY),
        )
    };
    let resolved_email = match request.email {
        Some(email) => Some(validate_signup_email(&email)?),
        None => saved_state.0.clone(),
    };
    let resolved_code = request
        .code
        .as_deref()
        .map(validate_signup_code)
        .transpose()?;

    let mut signup_summary: Option<String> = None;
    if current_user.is_none() {
        if let Some(code) = resolved_code.as_deref() {
            let signup_id = saved_state.1.clone().ok_or_else(|| {
                KiteError::BadInput(
                    "No pending Kite sign-up was found. Start with `/kite setup --email you@example.com` first.".to_string(),
                )
            })?;
            let exchange = complete_kite_signup(&cli_path, &signup_id, code)?;
            store_string_setting(db, KITE_SIGNUP_EMAIL_KEY, &exchange.email)?;
            store_string_setting(db, KITE_PENDING_SIGNUP_ID_KEY, "")?;
            signup_summary = Some(describe_signup_success(&exchange));
        } else if let Some(email) = resolved_email.as_deref() {
            store_string_setting(db, KITE_SIGNUP_EMAIL_KEY, email)?;
            let should_restart_signup =
                saved_state.1.is_none() || saved_state.0.as_deref() != Some(email);
            if should_restart_signup {
                let signup = start_kite_signup(&cli_path, email)?;
                store_string_setting(db, KITE_PENDING_SIGNUP_ID_KEY, &signup.signup_id)?;
            }
            emit(KiteEvent::AwaitingSensitiveValue {
                field: "kite_signup_code".to_string(),
                instructions: describe_signup_waiting_message(email),
            });
            let status = {
                let conn = db
                    .0
                    .lock()
                    .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
                load_setup_status(&conn, Some(cli_path))
            };
            return Ok(status);
        } else {
            emit(KiteEvent::AwaitingSensitiveValue {
                field: "kite_signup_email".to_string(),
                instructions: "Add your Kite Passport email in Settings > Agent > Kite Passport, or run `/kite setup --email you@example.com` to start sign-up.".to_string(),
            });
            let status = {
                let conn = db
                    .0
                    .lock()
                    .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
                load_setup_status(&conn, Some(cli_path))
            };
            return Ok(status);
        }
    }

    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
    let status = load_setup_status(&conn, Some(cli_path));
    let cli_line = match (status.cli_installed, status.cli_path.as_deref()) {
        (true, Some(path)) => format!("Kite CLI detected at `{path}`."),
        (true, None) => "Kite CLI is installed.".to_string(),
        (false, _) => "Thikra could not find Kite CLI after installation. Retry from Settings > Agent > Kite Passport.".to_string(),
    };
    let signup_line = if let Some(summary) = signup_summary {
        summary
    } else if let Some(user) = current_user {
        format!(
            "Kite Passport is already authenticated as `{}`.",
            user.email
        )
    } else {
        "Kite Passport sign-up has not finished yet.".to_string()
    };
    let mcp_line = if status.mcp_url_configured {
        format!(
            "A Kite MCP URL is already saved: {}",
            status
                .masked_mcp_url
                .clone()
                .unwrap_or_else(|| "(hidden)".to_string())
        )
    } else {
        "No Kite MCP URL is saved yet. After you create your agent in the Kite Portal, paste the portal-provided MCP URL into Settings > Agent > Kite Passport.".to_string()
    };
    emit(KiteEvent::OpeningPortal {
        target: "portal".to_string(),
    });
    emit(KiteEvent::WaitingForMcpConfig);
    emit(KiteEvent::Token(format!(
        "Kite setup\n{}\n\n{}\n\n{}\n\n{}\n\nNext steps\n1. Open the Kite Portal: {}\n2. Create your wallet and agent in the Portal.\n3. Copy the MCP URL from the Portal.\n4. Paste it into Settings > Agent > Kite Passport and click Verify.\n\n{}\n\nReference MCP endpoint format shown by Kite docs\n{}",
        cli_line,
        if installed_now {
            "Thikra installed Kite CLI with its own Windows fallback installer, so setup can continue even if Kite's hosted PowerShell bootstrap is broken."
        } else {
            "Kite CLI was already available, so setup can continue immediately."
        },
        describe_kite_agent_passport_install(install_outcome.as_ref()),
        signup_line,
        KITE_PORTAL_URL,
        mcp_line,
        KITE_TESTNET_MCP_URL
    )));
    Ok(status)
}

fn handle_kite_login(
    db: &crate::history::Database,
    request: KiteSetupRequest,
    emit: &mut impl FnMut(KiteEvent),
) -> Result<KiteAccountSummary, KiteError> {
    let cli_path = detect_kite_cli().ok_or(KiteError::CliMissing)?;
    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
    let saved_email = load_optional_setting(&conn, KITE_SIGNUP_EMAIL_KEY);
    let saved_login_id = load_optional_setting(&conn, KITE_PENDING_LOGIN_ID_KEY);
    drop(conn);

    let resolved_email = match request.email {
        Some(email) => Some(validate_signup_email(&email)?),
        None => saved_email.clone(),
    };
    let resolved_code = request
        .code
        .as_deref()
        .map(validate_signup_code)
        .transpose()?;

    if let Some(code) = resolved_code.as_deref() {
        let login_id = saved_login_id.ok_or_else(|| {
            KiteError::BadInput(
                "No pending Kite login was found. Start with `/kite login --email you@example.com` first.".to_string(),
            )
        })?;
        let user = complete_kite_login(&cli_path, &login_id, code)?;
        store_string_setting(db, KITE_SIGNUP_EMAIL_KEY, &user.email)?;
        store_string_setting(db, KITE_PENDING_LOGIN_ID_KEY, "")?;
    } else if let Some(email) = resolved_email.as_deref() {
        let login = start_kite_login(&cli_path, email)?;
        store_string_setting(db, KITE_SIGNUP_EMAIL_KEY, email)?;
        store_string_setting(db, KITE_PENDING_LOGIN_ID_KEY, &login.login_id)?;
        emit(KiteEvent::AwaitingSensitiveValue {
            field: "kite_login_code".to_string(),
            instructions: describe_login_waiting_message(email),
        });
    } else {
        emit(KiteEvent::AwaitingSensitiveValue {
            field: "kite_login_email".to_string(),
            instructions: "Run `/kite login --email you@example.com` or save your email in Settings > Agent > Kite Passport first.".to_string(),
        });
    }

    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
    Ok(load_kite_account_summary(&conn, Some(&cli_path)))
}

fn format_account_summary(account: &KiteAccountSummary) -> String {
    let status = if account.logged_in { "Logged in" } else { "Not logged in" };
    [
        "## Kite Account".to_string(),
        format!("- Status: {status}"),
        format!(
            "- Email: {}",
            account
                .email
                .as_deref()
                .or(account.signup_email.as_deref())
                .unwrap_or("Not set")
        ),
        format!("- User ID: {}", account.user_id.as_deref().unwrap_or("Unknown")),
        format!(
            "- Pending signup: {}",
            account.pending_signup_id.as_deref().unwrap_or("None")
        ),
        format!(
            "- Pending login: {}",
            account.pending_login_id.as_deref().unwrap_or("None")
        ),
        format!(
            "- Agent identity: {}",
            account.current_agent_identity.as_deref().unwrap_or("Not registered")
        ),
    ]
    .join("\n")
}

fn format_wallet_summary(wallet: &KiteWalletSummary) -> String {
    let mut lines = vec![
        "## Kite Wallet".to_string(),
        format!("- Address: `{}`", wallet.wallet_address),
        format!("- Type: {}", wallet.wallet_type),
        format!("- Chain: {}", wallet.chain_id),
    ];
    if wallet.assets.is_empty() {
        lines.push("- Assets: none reported yet".to_string());
    } else {
        lines.push("- Assets:".to_string());
        for asset in &wallet.assets {
            lines.push(format!(
                "  - {}: {}{}",
                asset.symbol,
                asset.balance,
                if asset.native { " (native)" } else { "" }
            ));
        }
    }
    if wallet.can_use_faucet {
        lines.push("- Faucet: available on this testnet wallet".to_string());
    }
    lines.join("\n")
}

fn format_wallet_transfer(transfer: &KiteWalletTransfer, title: &str) -> String {
    [
        format!("## {title}"),
        format!("- Amount: {} {}", transfer.amount, transfer.asset),
        format!("- To: `{}`", transfer.recipient_address),
        format!("- From: `{}`", transfer.wallet_address),
        format!("- Tx hash: `{}`", transfer.transaction_hash),
        format!("- Chain: {}", transfer.chain_id),
    ]
    .join("\n")
}

fn format_sessions_summary(sessions: &[KiteSessionSummary]) -> String {
    if sessions.is_empty() {
        return "## Kite Sessions\nNo Kite sessions were found yet.".to_string();
    }
    let mut lines = vec!["## Kite Sessions".to_string()];
    for session in sessions {
        lines.push(format!(
            "- `{}` [{}]{}",
            session.id,
            session.status,
            if session.selected { " (selected)" } else { "" }
        ));
        if let Some(summary) = session.task_summary.as_deref() {
            lines.push(format!("  - Task: {summary}"));
        }
        if let Some(expires_at) = session.expires_at.as_deref() {
            lines.push(format!("  - Expires: {expires_at}"));
        }
        if !session.assets.is_empty() {
            lines.push(format!("  - Assets: {}", session.assets.join(", ")));
        }
        if let Some(limit) = session.max_total_amount.as_deref() {
            lines.push(format!("  - Max total: {limit}"));
        }
        if let Some(spent) = session.spent_total.as_deref() {
            lines.push(format!("  - Spent: {spent}"));
        }
    }
    lines.join("\n")
}

fn format_session_request_status(status: &KiteSessionRequestStatus) -> String {
    [
        "## Kite Session Request".to_string(),
        format!("- Request ID: `{}`", status.request_id),
        format!("- Status: {}", status.status),
        format!("- Session ID: {}", status.session_id.as_deref().unwrap_or("Pending")),
        format!(
            "- Approval URL: {}",
            status
                .approval_url
                .as_deref()
                .unwrap_or("Use the Kite Portal / passkey flow")
        ),
        format!("- Detail: {}", status.message),
    ]
    .join("\n")
}

fn format_activity_summary(events: &[KiteActivityEvent]) -> String {
    if events.is_empty() {
        return "## Kite Activity\nNo activity was found yet.".to_string();
    }
    let mut lines = vec!["## Kite Activity".to_string()];
    for event in events {
        lines.push(format!("- {} [{}]", event.title, event.status));
        lines.push(format!("  - Kind: {}", event.kind));
        lines.push(format!("  - When: {}", event.occurred_at));
        if let Some(amount) = event.amount_display.as_deref() {
            lines.push(format!("  - Amount: {amount}"));
        }
        if let Some(order_id) = event.order_id.as_deref() {
            lines.push(format!("  - Order: `{order_id}`"));
        }
        if let Some(tx_hash) = event.tx_hash.as_deref() {
            lines.push(format!("  - Tx: `{tx_hash}`"));
        }
    }
    lines.join("\n")
}

fn format_shop_search_results(query: &str, items: &[KiteShopItem]) -> String {
    if items.is_empty() {
        return format!(
            "## Kite Shopping\nNo products were found for `{query}`. Try a more specific search."
        );
    }
    let mut lines = vec![format!("## Kite Shopping Results for `{query}`")];
    for (index, item) in items.iter().enumerate() {
        lines.push(format!(
            "- {}. {} — {}",
            index + 1,
            item.title,
            item.price
        ));
        lines.push(format!(
            "  - Locator: {}:{}",
            item.provider, item.external_identifier
        ));
        if let Some(rating) = item.rating.as_deref() {
            lines.push(format!(
                "  - Rating: {}{}",
                rating,
                item.reviews
                    .as_deref()
                    .map(|reviews| format!(" ({reviews} reviews)"))
                    .unwrap_or_default()
            ));
        }
    }
    lines.join("\n")
}

fn format_cart_summary(cart: &KiteCartSummary) -> String {
    let mut lines = vec![format!("## Kite Cart ({})", cart.item_count)];
    if cart.items.is_empty() {
        lines.push("- Your cart is empty.".to_string());
    } else {
        for item in &cart.items {
            lines.push(format!(
                "- {} — {} ×{}",
                item.title, item.price, item.quantity
            ));
            lines.push(format!("  - Locator: {}", item.product_locator));
        }
    }
    if let Some(currency) = cart.payment_currency.as_deref() {
        lines.push(format!("- Payment currency: {currency}"));
    }
    if let Some(chain) = cart.payment_chain.as_deref() {
        lines.push(format!("- Payment chain: {chain}"));
    }
    if let Some(shipping) = cart.shipping.as_ref() {
        lines.push(format!(
            "- Shipping: {}",
            if shipping.complete {
                "complete"
            } else {
                "missing details"
            }
        ));
        if !shipping.missing.is_empty() {
            lines.push(format!("  - Missing: {}", shipping.missing.join(", ")));
        }
    }
    lines.join("\n")
}

fn format_orders_summary(orders: &[KiteOrderSummary]) -> String {
    if orders.is_empty() {
        return "## Kite Orders\nNo Kite shopping orders were found yet.".to_string();
    }
    let mut lines = vec!["## Kite Orders".to_string()];
    for order in orders {
        lines.push(format!("- `{}`", order.order_id));
        if let Some(title) = order.title.as_deref() {
            lines.push(format!("  - Title: {title}"));
        }
        if let Some(phase) = order.phase.as_deref() {
            lines.push(format!("  - Phase: {phase}"));
        }
        if let Some(payment_status) = order.payment_status.as_deref() {
            lines.push(format!("  - Payment: {payment_status}"));
        }
        if let Some(chain) = order.chain.as_deref() {
            lines.push(format!("  - Chain: {chain}"));
        }
        if let Some(tx_hash) = order.tx_hash.as_deref() {
            lines.push(format!("  - Tx: `{tx_hash}`"));
        }
    }
    lines.join("\n")
}

fn resolve_pending_payment(
    runtime: &Arc<KiteRuntimeState>,
    action_id: &str,
    approved: bool,
) -> Result<String, String> {
    let pending_id = runtime
        .pending_payment_id
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    if pending_id.as_deref() != Some(action_id) {
        return Err("No pending Kite payment confirmation matches that action.".to_string());
    }
    let sender = runtime
        .pending_payment_tx
        .lock()
        .map_err(|e| e.to_string())?
        .take();
    if let Some(tx) = sender {
        let _ = tx.send(approved);
    } else {
        return Err("No Kite payment confirmation is waiting right now.".to_string());
    }
    *runtime
        .pending_payment_id
        .lock()
        .map_err(|e| e.to_string())? = None;
    Ok(if approved {
        "Kite payment confirmed.".to_string()
    } else {
        "Kite payment cancelled.".to_string()
    })
}

async fn request_payment_confirmation(
    runtime: &Arc<KiteRuntimeState>,
    emit: &mut impl FnMut(KiteEvent),
    summary: String,
) -> Result<(), KiteError> {
    let action_id = Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel::<bool>();
    *runtime
        .pending_payment_id
        .lock()
        .map_err(|e| KiteError::Unknown(e.to_string()))? = Some(action_id.clone());
    *runtime
        .pending_payment_tx
        .lock()
        .map_err(|e| KiteError::Unknown(e.to_string()))? = Some(tx);

    emit(KiteEvent::AwaitingPaymentConfirmation { action_id, summary });

    let approved = matches!(
        tokio::time::timeout(Duration::from_secs(60), rx).await,
        Ok(Ok(true))
    );

    *runtime
        .pending_payment_id
        .lock()
        .map_err(|e| KiteError::Unknown(e.to_string()))? = None;
    *runtime
        .pending_payment_tx
        .lock()
        .map_err(|e| KiteError::Unknown(e.to_string()))? = None;

    if approved {
        emit(KiteEvent::ResumingAfterUserStep {
            step: "payment_confirmation".to_string(),
        });
        Ok(())
    } else {
        Err(KiteError::BadInput(
            "Kite payment approval was cancelled before any payment was signed.".to_string(),
        ))
    }
}

fn should_escalate_to_agent(error: &KiteError) -> bool {
    matches!(
        error,
        KiteError::CliMissing
            | KiteError::CliInstallFailed(_)
            | KiteError::MissingMcpUrl
            | KiteError::AuthRequired(_)
            | KiteError::SessionCreationRequired(_)
            | KiteError::SessionExpired(_)
            | KiteError::Unauthorized(_)
            | KiteError::AgentNotFound(_)
            | KiteError::Network(_)
            | KiteError::Unknown(_)
    )
}

async fn maybe_escalate_kite_mode(
    app_handle: &AppHandle,
    input: &str,
    reason: Option<&str>,
    client: &Client,
    chat_provider: &crate::providers::SharedChatProvider,
    db: &crate::history::Database,
    agent_state: &Arc<crate::agent::AgentState>,
    active_model: &crate::models::ActiveModelState,
    config: &crate::config::AppConfig,
    emit: &mut impl FnMut(KiteEvent),
) -> Result<(), KiteError> {
    match start_kite_agent_mode_inner(
        app_handle,
        input,
        reason,
        client,
        chat_provider,
        db,
        agent_state,
        active_model,
        config,
    )
    .await
    {
        Ok(message) => {
            emit(KiteEvent::EnteringAgentMode {
                reason: reason
                    .unwrap_or("Kite needs agentic help to recover.")
                    .to_string(),
            });
            emit(KiteEvent::Token(message));
            Ok(())
        }
        Err(KiteError::BadInput(guidance)) => {
            let reason_text = reason.unwrap_or("Agentic Kite mode is unavailable right now.");
            emit(KiteEvent::AdvisoryFallback {
                reason: reason_text.to_string(),
                guidance: guidance.clone(),
            });
            Ok(())
        }
        Err(other) => Err(other),
    }
}

async fn start_kite_agent_mode_inner(
    app_handle: &AppHandle,
    input: &str,
    reason: Option<&str>,
    client: &Client,
    chat_provider: &crate::providers::SharedChatProvider,
    db: &crate::history::Database,
    agent_state: &Arc<crate::agent::AgentState>,
    active_model: &crate::models::ActiveModelState,
    config: &crate::config::AppConfig,
) -> Result<String, KiteError> {
    let provider_config = hydrate_kite_provider_config(chat_provider, db, agent_state, config)
        .map_err(KiteError::BadInput)?;
    let capability =
        kite_agent_capability_inner(client, chat_provider, db, agent_state, active_model, config)
            .await
            .map_err(KiteError::BadInput)?;
    if !capability.available {
        return Err(KiteError::BadInput(advisory_fallback_message(
            &capability,
            reason,
        )));
    }

    let task = build_kite_agent_task(input, reason, db)?;
    let launch_model = resolve_kite_agent_launch_model(provider_config.as_ref(), active_model)?;
    let ollama_url = config.inference.ollama_url.clone();

    crate::agent::spawn_agent_run(
        app_handle.clone(),
        agent_state.clone(),
        task,
        launch_model,
        ollama_url,
    );

    Ok(format!(
        "Kite agentic mode is taking over.\nProvider: {}\n{}\n\nThe connected AI can navigate the desktop to recover Kite setup, auth, and portal issues. It will not enter new secrets for you, and payments still require explicit confirmation.",
        capability.provider, capability.reason
    ))
}

async fn kite_agent_capability_inner(
    client: &Client,
    chat_provider: &crate::providers::SharedChatProvider,
    db: &crate::history::Database,
    agent_state: &Arc<crate::agent::AgentState>,
    active_model: &crate::models::ActiveModelState,
    config: &crate::config::AppConfig,
) -> Result<KiteAgentCapability, String> {
    if let Some(provider_config) =
        hydrate_kite_provider_config(chat_provider, db, agent_state, config)?
    {
        let provider = format!("{:?}", provider_config.provider);
        if !matches!(provider_config.provider, crate::providers::Provider::Ollama) {
            let reason = format!(
                "Using connected cloud provider `{}` for Kite desktop recovery and portal automation.",
                provider_config.model
            );
            return Ok(KiteAgentCapability {
                available: true,
                mode: "agentic".to_string(),
                provider,
                model: Some(provider_config.model),
                reason,
            });
        }
    }

    let selected_model = active_model.0.lock().map_err(|e| e.to_string())?.clone();

    let Some(model) = selected_model else {
        return match crate::models::fetch_installed_model_names(client, &config.inference.ollama_url)
            .await
        {
            Ok(models) if models.is_empty() => Ok(KiteAgentCapability {
                available: false,
                mode: "advisory_fallback".to_string(),
                provider: "ollama".to_string(),
                model: None,
                reason: "Ollama is reachable, but no local models are installed yet. Guided Kite help is available now, and full autopilot unlocks after you install and select a vision-capable model.".to_string(),
            }),
            Ok(models) => {
                let suggestion = models
                    .iter()
                    .find(|candidate| {
                        let slug = candidate.to_ascii_lowercase();
                        slug.contains("vision") || slug.contains("-vl") || slug.contains("vl-")
                    })
                    .cloned()
                    .or_else(|| models.first().cloned())
                    .unwrap_or_else(|| "a vision-capable Ollama model".to_string());
                Ok(KiteAgentCapability {
                    available: false,
                    mode: "advisory_fallback".to_string(),
                    provider: "ollama".to_string(),
                    model: None,
                    reason: format!(
                        "Ollama is running, but no local model is selected. Guided Kite help is available now. To unlock autopilot, pick a vision-capable local model such as `{suggestion}`."
                    ),
                })
            }
            Err(err) => Ok(KiteAgentCapability {
                available: false,
                mode: "advisory_fallback".to_string(),
                provider: "ollama".to_string(),
                model: None,
                reason: format!(
                    "Ollama is unreachable, so Thikra can only offer guided Kite help in chat right now. ({err})"
                ),
            }),
        };
    };

    let caps =
        crate::models::fetch_model_capabilities(client, &config.inference.ollama_url, &model).await;
    match caps {
        Ok(capabilities) if capabilities.vision => Ok(KiteAgentCapability {
            available: true,
            mode: "agentic".to_string(),
            provider: "ollama".to_string(),
            model: Some(model.clone()),
            reason: format!("Using local vision model `{model}` for on-device desktop navigation."),
        }),
        Ok(_) => Ok(KiteAgentCapability {
            available: false,
            mode: "advisory_fallback".to_string(),
            provider: "ollama".to_string(),
            model: Some(model.clone()),
            reason: format!(
                "Local model `{model}` does not advertise vision support. Guided Kite help is available now, and full autopilot requires a cloud provider or a local vision-capable Ollama model."
            ),
        }),
        Err(err) => Ok(KiteAgentCapability {
            available: false,
            mode: "advisory_fallback".to_string(),
            provider: "ollama".to_string(),
            model: Some(model),
            reason: format!(
                "Thikra could not confirm the local model's capabilities, so Kite is falling back to guided help. ({err})"
            ),
        }),
    }
}

fn hydrate_kite_provider_config(
    chat_provider: &crate::providers::SharedChatProvider,
    db: &crate::history::Database,
    agent_state: &Arc<crate::agent::AgentState>,
    config: &crate::config::AppConfig,
) -> Result<Option<crate::providers::ProviderConfig>, String> {
    if let Some(provider_config) = agent_state.get_provider_config() {
        if !matches!(provider_config.provider, crate::providers::Provider::Ollama) {
            return Ok(Some(provider_config));
        }
    }

    let chat_provider_config = chat_provider.0.lock().map_err(|e| e.to_string())?.clone();
    if let Some(provider_config) = chat_provider_config {
        if !matches!(provider_config.provider, crate::providers::Provider::Ollama) {
            agent_state.set_provider_config(provider_config.clone());
            return Ok(Some(provider_config));
        }
    }

    let persisted_provider = hydrate_kite_provider_from_persisted_settings(db, config)?;
    if let Some(provider_config) = persisted_provider.clone() {
        agent_state.set_provider_config(provider_config.clone());
        *chat_provider.0.lock().map_err(|e| e.to_string())? = Some(provider_config.clone());
    }
    Ok(persisted_provider)
}

fn hydrate_kite_provider_from_persisted_settings(
    db: &crate::history::Database,
    config: &crate::config::AppConfig,
) -> Result<Option<crate::providers::ProviderConfig>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let provider_mode = crate::database::get_config(&conn, "provider_mode")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let provider = if provider_mode.trim().eq_ignore_ascii_case("openrouter") {
        crate::providers::Provider::OpenRouter
    } else {
        match provider_from_str(&config.agent.provider) {
            Some(provider) => provider,
            None => return Ok(None),
        }
    };
    if matches!(provider, crate::providers::Provider::Ollama) {
        return Ok(None);
    }

    let settings_key = match provider {
        crate::providers::Provider::OpenAI => "api_key_openai",
        crate::providers::Provider::Anthropic => "api_key_anthropic",
        crate::providers::Provider::OpenRouter => "api_key_openrouter",
        crate::providers::Provider::Ollama => return Ok(None),
    };

    let api_key = crate::database::get_config(&conn, settings_key)
        .map_err(|e| e.to_string())?
        .unwrap_or_default()
        .trim()
        .to_string();
    if api_key.is_empty() {
        return Ok(None);
    }

    let openrouter_model = crate::database::get_config(&conn, "openrouter_model")
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let model = if matches!(provider, crate::providers::Provider::OpenRouter)
        && !openrouter_model.trim().is_empty()
    {
        openrouter_model.trim()
    } else {
        config.agent.model.trim()
    };
    let base_url = config.agent.base_url.trim();
    Ok(Some(crate::providers::ProviderConfig {
        provider: provider.clone(),
        model: if model.is_empty() {
            crate::providers::default_models(&provider)
                .first()
                .copied()
                .unwrap_or_default()
                .to_string()
        } else {
            model.to_string()
        },
        base_url: if base_url.is_empty() {
            crate::providers::default_base_url(&provider).to_string()
        } else {
            base_url.to_string()
        },
        api_key,
    }))
}

fn provider_from_str(provider: &str) -> Option<crate::providers::Provider> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "openai" => Some(crate::providers::Provider::OpenAI),
        "anthropic" => Some(crate::providers::Provider::Anthropic),
        "openrouter" => Some(crate::providers::Provider::OpenRouter),
        "ollama" => Some(crate::providers::Provider::Ollama),
        _ => None,
    }
}

fn resolve_kite_agent_local_model(
    active_model: &crate::models::ActiveModelState,
) -> Result<String, KiteError> {
    active_model
        .0
        .lock()
        .map_err(|e| KiteError::Unknown(e.to_string()))?
        .clone()
        .ok_or_else(|| {
            KiteError::BadInput(
                "No local model is selected yet. Guided Kite help can still continue, but autopilot needs a local vision model or an online agent provider.".to_string(),
            )
        })
}

fn resolve_kite_agent_launch_model(
    provider_config: Option<&crate::providers::ProviderConfig>,
    active_model: &crate::models::ActiveModelState,
) -> Result<String, KiteError> {
    match provider_config {
        Some(provider_config)
            if !matches!(provider_config.provider, crate::providers::Provider::Ollama) =>
        {
            Ok(provider_config.model.clone())
        }
        _ => resolve_kite_agent_local_model(active_model),
    }
}

fn advisory_fallback_message(capability: &KiteAgentCapability, reason: Option<&str>) -> String {
    let why = reason.unwrap_or("Kite needs help, but full desktop automation is not available.");
    format!(
        "Kite guided help\n{}\n\n{}\n\nNext steps\n1. Continue with the guided setup and troubleshooting steps in this chat.\n2. If you want autopilot, connect OpenRouter/OpenAI/Anthropic or choose a local vision-capable Ollama model.\n3. If the blocker is a new MCP URL or login step, enter it manually and rerun the command.",
        why, capability.reason
    )
}

fn build_kite_agent_task(
    input: &str,
    reason: Option<&str>,
    db: &crate::history::Database,
) -> Result<String, KiteError> {
    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
    let status = load_setup_status(&conn, detect_kite_cli());
    let cli_status = if status.cli_installed {
        format!(
            "Kite CLI is installed{}.",
            status
                .cli_path
                .as_deref()
                .map(|path| format!(" at `{path}`"))
                .unwrap_or_default()
        )
    } else {
        "Kite CLI is missing or still needs installation.".to_string()
    };
    let mcp_status = if status.mcp_url_configured {
        format!(
            "A saved MCP URL is available: {}.",
            status
                .masked_mcp_url
                .unwrap_or_else(|| "(hidden)".to_string())
        )
    } else {
        "No MCP URL is saved yet; the user must paste a new MCP URL manually when Kite exposes it."
            .to_string()
    };
    let payer_status = status
        .last_payer_addr
        .as_ref()
        .map(|payer| format!("Last known payer address: {payer}."))
        .unwrap_or_else(|| "No payer address has been fetched yet.".to_string());
    Ok(format!(
        "Help complete or recover Kite Passport inside Thuki.\n\nOriginal command: {input}\nReason for escalation: {}\n\nCurrent local state\n- {}\n- {}\n- Auth state: {:?}\n- Connected: {}\n- {}\n\nWhat to do\n- Install Kite CLI if missing.\n- Open the Kite Portal or docs when helpful.\n- Navigate the desktop/browser to diagnose setup, auth, session, and MCP issues.\n- Verify the Kite MCP connection when the user has provided the needed info.\n- Continue until the requested Kite flow is unblocked or clearly explain the blocker.\n\nHard rules\n- Do not type brand-new secrets, credentials, or MCP URLs for the user.\n- You may use already-saved Kite configuration when Thuki exposes it.\n- Stop and wait when a page requires the user to log in or paste a new secret.\n- Never approve payments without explicit user confirmation.",
        reason.unwrap_or("Kite setup or recovery needs agentic help."),
        cli_status,
        mcp_status,
        status.auth_state,
        status.connected,
        payer_status
    ))
}

async fn verify_kite_connection_inner(
    client: &Client,
    db: &crate::history::Database,
) -> Result<KiteVerifyResponse, KiteError> {
    let tools = list_tools(client, db).await?;
    let connected = tools.iter().any(|tool| tool == "get_payer_addr")
        && tools.iter().any(|tool| tool == "approve_payment");
    let message = if connected {
        "Kite MCP connection verified. Payment tools are available.".to_string()
    } else {
        format!(
            "Connected to the MCP server, but the expected Kite tools were not both available. Found: {}",
            if tools.is_empty() {
                "(none)".to_string()
            } else {
                tools.join(", ")
            }
        )
    };
    store_auth_state(
        db,
        if connected {
            KiteAuthState::Ready
        } else {
            KiteAuthState::UnknownError
        },
    )?;
    Ok(KiteVerifyResponse {
        connected,
        auth_state: if connected {
            KiteAuthState::Ready
        } else {
            KiteAuthState::UnknownError
        },
        available_tools: tools,
        message,
    })
}

async fn fetch_payer_address(
    client: &Client,
    db: &crate::history::Database,
) -> Result<String, KiteError> {
    let result = call_tool(client, db, "get_payer_addr", json!({})).await?;
    let object = extract_structured_object(&result).ok_or_else(|| {
        KiteError::InvalidPaymentResponse(
            "Kite returned an unreadable payer address payload.".to_string(),
        )
    })?;
    let payer = object
        .get("payer_addr")
        .or_else(|| object.get("payerAddr"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| extract_first_text(&result).map(|text| text.to_string()))
        .ok_or_else(|| {
            KiteError::InvalidPaymentResponse("Kite did not return `payer_addr`.".to_string())
        })?;
    store_string_setting(db, KITE_LAST_PAYER_KEY, &payer)?;
    Ok(payer)
}

async fn approve_payment(
    client: &Client,
    db: &crate::history::Database,
    payer_addr: &str,
    payment: &PaymentRequest,
) -> Result<PaymentAuthorization, KiteError> {
    let mut args = json!({
        "payer_addr": payer_addr,
        "payee_addr": payment.payee_addr,
        "amount": payment.amount,
        "token_type": payment.token_type,
    });
    if let Some(name) = payment.merchant_name.as_ref() {
        args["merchant_name"] = Value::String(name.clone());
    }
    let result = call_tool(client, db, "approve_payment", args).await?;
    let object = extract_structured_object(&result).ok_or_else(|| {
        KiteError::InvalidPaymentResponse(
            "Kite returned an unreadable payment authorization payload.".to_string(),
        )
    })?;
    let x_payment = object
        .get("x_payment")
        .or_else(|| object.get("xPayment"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .or_else(|| extract_first_text(&result).map(|text| text.to_string()))
        .ok_or_else(|| {
            KiteError::InvalidPaymentResponse(
                "Kite did not return an `x_payment` value.".to_string(),
            )
        })?;
    Ok(PaymentAuthorization { x_payment })
}

async fn call_x402_service(
    client: &Client,
    db: &crate::history::Database,
    runtime: &Arc<KiteRuntimeState>,
    url: &str,
    method: &str,
    body: Option<&str>,
    merchant_name: Option<String>,
    emit: &mut impl FnMut(KiteEvent),
) -> Result<X402CallResult, KiteError> {
    let method = parse_http_method(method)?;
    let initial = send_service_request(client, &method, url, body, None).await?;
    if initial.status() != StatusCode::PAYMENT_REQUIRED {
        let initial_status = initial.status().as_u16();
        let body = format_response_body(initial).await?;
        return Ok(X402CallResult {
            status: initial_status,
            body,
            payment: PaymentRequest {
                payee_addr: "(no payment required)".to_string(),
                amount: "0".to_string(),
                token_type: "UNKNOWN".to_string(),
                merchant_name,
            },
        });
    }

    let payment_info = parse_payment_required_response(initial, merchant_name).await?;
    emit(KiteEvent::FetchingPayer);
    let payer_addr = fetch_payer_address(client, db).await?;
    emit(KiteEvent::ApprovingPayment);
    request_payment_confirmation(
        runtime,
        emit,
        format!(
            "Approve Kite x402 payment?\nPayee: {}\nAmount: {} {}\nPayer: {}\nURL: {}",
            payment_info.payee_addr, payment_info.amount, payment_info.token_type, payer_addr, url
        ),
    )
    .await?;
    let auth = approve_payment(client, db, &payer_addr, &payment_info).await?;
    emit(KiteEvent::RetryingPaidRequest);
    let retried =
        send_service_request(client, &method, url, body, Some(auth.x_payment.as_str())).await?;
    let retried_status = retried.status().as_u16();
    let retried_body = format_response_body(retried).await?;
    Ok(X402CallResult {
        status: retried_status,
        body: retried_body,
        payment: payment_info,
    })
}

async fn send_service_request(
    client: &Client,
    method: &Method,
    url: &str,
    body: Option<&str>,
    x_payment: Option<&str>,
) -> Result<reqwest::Response, KiteError> {
    let mut request = client.request(method.clone(), url);
    if let Some(payment) = x_payment {
        request = request.header("X-Payment", payment);
    }
    if let Some(raw_body) = body {
        if let Ok(value) = serde_json::from_str::<Value>(raw_body) {
            request = request
                .header(CONTENT_TYPE, "application/json")
                .json(&value);
        } else {
            request = request.body(raw_body.to_string());
        }
    }
    request
        .send()
        .await
        .map_err(|err| KiteError::Network(format!("Could not call the x402 service: {err}")))
}

async fn call_tool(
    client: &Client,
    db: &crate::history::Database,
    tool_name: &str,
    arguments: Value,
) -> Result<Value, KiteError> {
    let mut secrets = load_kite_secrets(db)?;
    let result = ensure_session_and_send(
        client,
        &mut secrets,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments,
            }
        }),
    )
    .await?;
    persist_session_if_changed(db, &secrets)?;
    let payload = result.result.ok_or_else(|| {
        KiteError::InvalidPaymentResponse("Kite returned no tool payload.".to_string())
    })?;
    if payload
        .get("isError")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let text = extract_first_text(&payload).unwrap_or("Kite tool call failed.");
        let error = map_kite_failure(text);
        store_auth_state(db, error.auth_state())?;
        return Err(error);
    }
    store_auth_state(db, KiteAuthState::Ready)?;
    Ok(payload)
}

async fn list_tools(
    client: &Client,
    db: &crate::history::Database,
) -> Result<Vec<String>, KiteError> {
    let mut secrets = load_kite_secrets(db)?;
    let result = ensure_session_and_send(
        client,
        &mut secrets,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await?;
    persist_session_if_changed(db, &secrets)?;
    let payload = result.result.ok_or_else(|| {
        KiteError::InvalidPaymentResponse("Kite did not return a tools list.".to_string())
    })?;
    let tools = payload
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| {
            entry
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    Ok(tools)
}

async fn ensure_session_and_send(
    client: &Client,
    secrets: &mut KiteSecrets,
    payload: Value,
) -> Result<JsonRpcEnvelope, KiteError> {
    if secrets.session_id.is_none() {
        initialize_mcp_session(client, secrets).await?;
    }
    match send_jsonrpc_request(client, secrets, payload.clone(), false).await {
        Ok(response) => Ok(response),
        Err(KiteError::SessionExpired(_)) if secrets.session_id.is_some() => {
            secrets.session_id = None;
            initialize_mcp_session(client, secrets).await?;
            send_jsonrpc_request(client, secrets, payload, false).await
        }
        Err(err) => Err(err),
    }
}

async fn initialize_mcp_session(
    client: &Client,
    secrets: &mut KiteSecrets,
) -> Result<(), KiteError> {
    let init = send_jsonrpc_request(
        client,
        secrets,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "Thuki",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }
        }),
        true,
    )
    .await?;
    let server_version = init
        .result
        .as_ref()
        .and_then(|result| result.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(MCP_PROTOCOL_VERSION);
    if server_version != MCP_PROTOCOL_VERSION {
        return Err(KiteError::Unknown(format!(
            "Kite MCP protocol mismatch. Server responded with {server_version}, but Thuki expects {MCP_PROTOCOL_VERSION}."
        )));
    }
    send_notification(
        client,
        secrets,
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
    )
    .await?;
    Ok(())
}

async fn send_notification(
    client: &Client,
    secrets: &KiteSecrets,
    payload: Value,
) -> Result<(), KiteError> {
    let headers = build_mcp_headers(secrets, false)?;
    let response = client
        .post(&secrets.mcp_url)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .map_err(|err| KiteError::Network(format!("Could not reach Kite MCP server: {err}")))?;
    if response.status().is_success() || response.status() == StatusCode::ACCEPTED {
        Ok(())
    } else {
        Err(map_http_status(
            response.status(),
            response.text().await.unwrap_or_default(),
        ))
    }
}

async fn send_jsonrpc_request(
    client: &Client,
    secrets: &mut KiteSecrets,
    payload: Value,
    initialization: bool,
) -> Result<JsonRpcEnvelope, KiteError> {
    let headers = build_mcp_headers(secrets, initialization)?;
    let response = client
        .post(&secrets.mcp_url)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .map_err(|err| KiteError::Network(format!("Could not reach Kite MCP server: {err}")))?;
    if initialization {
        secrets.session_id = response
            .headers()
            .get("Mcp-Session-Id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
    }
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(map_http_status(status, body));
    }
    parse_jsonrpc_response(response).await
}

fn build_mcp_headers(secrets: &KiteSecrets, initialization: bool) -> Result<HeaderMap, KiteError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/event-stream"),
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if !initialization {
        let session_id = secrets.session_id.as_ref().ok_or_else(|| {
            KiteError::SessionExpired(
                "The Kite MCP session expired before the request could be sent.".to_string(),
            )
        })?;
        let header = HeaderValue::from_str(session_id).map_err(|_| {
            KiteError::Unknown("Kite returned an invalid MCP session id.".to_string())
        })?;
        headers.insert("Mcp-Session-Id", header);
    }
    Ok(headers)
}

async fn parse_jsonrpc_response(response: reqwest::Response) -> Result<JsonRpcEnvelope, KiteError> {
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response
        .text()
        .await
        .map_err(|err| KiteError::Network(format!("Could not read Kite MCP response: {err}")))?;
    if content_type.starts_with("text/event-stream") {
        parse_sse_jsonrpc(&body)
    } else {
        parse_jsonrpc_body(&body)
    }
}

fn parse_sse_jsonrpc(body: &str) -> Result<JsonRpcEnvelope, KiteError> {
    for line in body.lines() {
        let trimmed = line.trim();
        if let Some(data) = trimmed.strip_prefix("data:") {
            let candidate = data.trim();
            if candidate.is_empty() || candidate == "[DONE]" {
                continue;
            }
            return parse_jsonrpc_body(candidate);
        }
    }
    Err(KiteError::InvalidPaymentResponse(
        "Kite returned an SSE response without a JSON-RPC payload.".to_string(),
    ))
}

fn parse_jsonrpc_body(body: &str) -> Result<JsonRpcEnvelope, KiteError> {
    let envelope: JsonRpcEnvelope = serde_json::from_str(body).map_err(|err| {
        KiteError::InvalidPaymentResponse(format!("Kite returned invalid JSON-RPC: {err}"))
    })?;
    if let Some(error) = envelope.error.as_ref() {
        return Err(map_kite_failure(&format_jsonrpc_error(error)));
    }
    Ok(envelope)
}

async fn parse_payment_required_response(
    response: reqwest::Response,
    merchant_name: Option<String>,
) -> Result<PaymentRequest, KiteError> {
    let raw = response.text().await.unwrap_or_default();
    let value: Value = serde_json::from_str(&raw).map_err(|_| {
        KiteError::InvalidPaymentResponse(
            "The x402 service returned 402 Payment Required, but the payment body was not valid JSON.".to_string(),
        )
    })?;
    let payee_addr = value
        .get("payee_addr")
        .or_else(|| value.get("payeeAddr"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            KiteError::InvalidPaymentResponse(
                "The x402 service did not include `payee_addr` in its 402 response.".to_string(),
            )
        })?;
    let amount = value
        .get("amount")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            KiteError::InvalidPaymentResponse(
                "The x402 service did not include `amount` in its 402 response.".to_string(),
            )
        })?;
    let token_type = value
        .get("token_type")
        .or_else(|| value.get("tokenType"))
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            KiteError::InvalidPaymentResponse(
                "The x402 service did not include `token_type` in its 402 response.".to_string(),
            )
        })?;
    let merchant = merchant_name.or_else(|| {
        value
            .get("merchant_name")
            .or_else(|| value.get("merchantName"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    Ok(PaymentRequest {
        payee_addr,
        amount,
        token_type,
        merchant_name: merchant,
    })
}

async fn format_response_body(response: reqwest::Response) -> Result<String, KiteError> {
    let text = response.text().await.map_err(|err| {
        KiteError::Network(format!("Could not read the x402 service response: {err}"))
    })?;
    if let Ok(json_value) = serde_json::from_str::<Value>(&text) {
        serde_json::to_string_pretty(&json_value).map_err(|err| {
            KiteError::InvalidPaymentResponse(format!(
                "Could not format the x402 response JSON: {err}"
            ))
        })
    } else if text.trim().is_empty() {
        Ok("(empty response body)".to_string())
    } else {
        Ok(text)
    }
}

fn extract_structured_object(result: &Value) -> Option<serde_json::Map<String, Value>> {
    result
        .get("structuredContent")
        .and_then(Value::as_object)
        .cloned()
        .or_else(|| {
            extract_first_text(result)
                .and_then(|text| serde_json::from_str::<Value>(text).ok())
                .and_then(|value| value.as_object().cloned())
        })
}

fn extract_first_text(result: &Value) -> Option<&str> {
    result
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                if item.get("type").and_then(Value::as_str) == Some("text") {
                    item.get("text").and_then(Value::as_str)
                } else {
                    None
                }
            })
        })
}

fn load_kite_secrets(db: &crate::history::Database) -> Result<KiteSecrets, KiteError> {
    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
    let url = crate::database::get_config(&conn, KITE_MCP_URL_KEY)
        .map_err(|e| KiteError::Unknown(e.to_string()))?
        .unwrap_or_default();
    if url.trim().is_empty() {
        return Err(KiteError::MissingMcpUrl);
    }
    let session_id = crate::database::get_config(&conn, KITE_SESSION_ID_KEY)
        .map_err(|e| KiteError::Unknown(e.to_string()))?;
    Ok(KiteSecrets {
        mcp_url: url,
        session_id: session_id.filter(|value| !value.trim().is_empty()),
    })
}

fn persist_session_if_changed(
    db: &crate::history::Database,
    secrets: &KiteSecrets,
) -> Result<(), KiteError> {
    store_string_setting(
        db,
        KITE_SESSION_ID_KEY,
        secrets.session_id.as_deref().unwrap_or(""),
    )
}

fn persist_mcp_url(conn: &rusqlite::Connection, url: &str) -> rusqlite::Result<()> {
    crate::database::set_config(conn, KITE_MCP_URL_KEY, url.trim())?;
    crate::database::set_config(conn, KITE_SESSION_ID_KEY, "")?;
    crate::database::set_config(
        conn,
        KITE_AUTH_STATE_KEY,
        KiteAuthState::Unverified.as_storage_str(),
    )?;
    Ok(())
}

fn clear_kite_settings(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    crate::database::set_config(conn, KITE_MCP_URL_KEY, "")?;
    crate::database::set_config(conn, KITE_SESSION_ID_KEY, "")?;
    crate::database::set_config(
        conn,
        KITE_AUTH_STATE_KEY,
        KiteAuthState::MissingMcpUrl.as_storage_str(),
    )?;
    crate::database::set_config(conn, KITE_LAST_PAYER_KEY, "")?;
    crate::database::set_config(conn, KITE_PENDING_SIGNUP_ID_KEY, "")?;
    crate::database::set_config(conn, KITE_PENDING_LOGIN_ID_KEY, "")?;
    crate::database::set_config(conn, KITE_ACTIVE_SPENDING_SESSION_KEY, "")?;
    Ok(())
}

fn store_string_setting(
    db: &crate::history::Database,
    key: &str,
    value: &str,
) -> Result<(), KiteError> {
    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
    crate::database::set_config(&conn, key, value).map_err(|e| KiteError::Unknown(e.to_string()))
}

fn store_auth_state(db: &crate::history::Database, state: KiteAuthState) -> Result<(), KiteError> {
    store_string_setting(db, KITE_AUTH_STATE_KEY, state.as_storage_str())
}

fn load_optional_setting(conn: &rusqlite::Connection, key: &str) -> Option<String> {
    crate::database::get_config(conn, key)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn validate_signup_email(email: &str) -> Result<String, KiteError> {
    let trimmed = email.trim();
    if trimmed.is_empty() || !trimmed.contains('@') {
        return Err(KiteError::BadInput(
            "Provide a valid email with `/kite setup --email you@example.com` or save it in Settings > Agent > Kite Passport.".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_signup_code(code: &str) -> Result<String, KiteError> {
    let trimmed = code.trim();
    let valid = trimmed.len() == 8 && trimmed.chars().all(|ch| ch.is_ascii_alphanumeric());
    if !valid {
        return Err(KiteError::BadInput(
            "Kite sign-up codes are 8 alphanumeric characters. Retry with `/kite setup --code ABCD1234`.".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn format_kpass_error(action: &str, stdout: &str, stderr: &str) -> KiteError {
    let parsed = serde_json::from_str::<KpassEnvelope>(stdout).ok();
    let detail = parsed
        .as_ref()
        .and_then(|response| {
            let error = response.error.trim();
            let hint = response.hint.trim();
            if !error.is_empty() && !hint.is_empty() {
                Some(format!("{error} {hint}"))
            } else if !error.is_empty() {
                Some(error.to_string())
            } else if !hint.is_empty() {
                Some(hint.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| format_process_output(stdout.as_bytes(), stderr.as_bytes()));
    KiteError::BadInput(format!("Kite {action} failed. {detail}"))
}

fn run_kpass_json_command(
    cli_path: &Path,
    args: &[&str],
    envs: &[(&str, &str)],
) -> Result<(String, String, Option<i32>), KiteError> {
    let mut command = Command::new(cli_path);
    command.args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().map_err(|err| {
        KiteError::CliInstallFailed(format!(
            "Thikra could not run Kite CLI at `{}`. {}",
            cli_path.display(),
            err
        ))
    })?;
    Ok((
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).trim().to_string(),
        output.status.code(),
    ))
}

fn load_current_kite_user(cli_path: &Path) -> Result<Option<KpassMeResponse>, KiteError> {
    let (stdout, stderr, exit_code) =
        run_kpass_json_command(cli_path, &["me", "--output", "json"], &[])?;
    if exit_code == Some(0) {
        return serde_json::from_str::<KpassMeResponse>(&stdout)
            .map(Some)
            .map_err(|err| {
                KiteError::Unknown(format!("Kite returned unreadable account details. {err}"))
            });
    }
    if exit_code == Some(3) {
        return Ok(None);
    }
    Err(format_kpass_error("account check", &stdout, &stderr))
}

fn start_kite_signup(cli_path: &Path, email: &str) -> Result<KpassSignupInitResponse, KiteError> {
    let (stdout, stderr, exit_code) = run_kpass_json_command(
        cli_path,
        &[
            "signup",
            "init",
            "--email",
            email,
            "--client",
            "agent",
            "--output",
            "json",
            "--no-interactive",
        ],
        &[],
    )?;
    if exit_code == Some(0) {
        return serde_json::from_str::<KpassSignupInitResponse>(&stdout).map_err(|err| {
            KiteError::Unknown(format!("Kite returned unreadable sign-up details. {err}"))
        });
    }
    Err(format_kpass_error("sign-up start", &stdout, &stderr))
}

fn start_kite_login(cli_path: &Path, email: &str) -> Result<KpassLoginInitResponse, KiteError> {
    let (stdout, stderr, exit_code) = run_kpass_json_command(
        cli_path,
        &[
            "login",
            "init",
            "--email",
            email,
            "--client",
            "agent",
            "--output",
            "json",
            "--no-interactive",
        ],
        &[],
    )?;
    if exit_code == Some(0) {
        return serde_json::from_str::<KpassLoginInitResponse>(&stdout).map_err(|err| {
            KiteError::Unknown(format!("Kite returned unreadable login details. {err}"))
        });
    }
    Err(map_kpass_command_error("login start", &stdout, &stderr, exit_code))
}

fn complete_kite_login(
    cli_path: &Path,
    login_id: &str,
    code: &str,
) -> Result<KpassMeResponse, KiteError> {
    run_kpass_json_value(
        cli_path,
        &[
            "login",
            "verify",
            "--login-id",
            login_id,
            "--code",
            code,
            "--output",
            "json",
            "--no-interactive",
        ],
        &[],
        "login verification",
    )?;
    load_current_kite_user(cli_path)?
        .ok_or_else(|| KiteError::Unknown("Kite login succeeded but no user session was available.".to_string()))
}

fn complete_kite_signup(
    cli_path: &Path,
    signup_id: &str,
    code: &str,
) -> Result<KpassSignupExchangeResponse, KiteError> {
    let (stdout, stderr, exit_code) = run_kpass_json_command(
        cli_path,
        &[
            "signup",
            "exchange",
            "--signup-id",
            signup_id,
            "--output",
            "json",
        ],
        &[("KPASS_SIGNUP_CODE", code)],
    )?;
    if exit_code == Some(0) {
        return serde_json::from_str::<KpassSignupExchangeResponse>(&stdout).map_err(|err| {
            KiteError::Unknown(format!(
                "Kite returned unreadable sign-up completion details. {err}"
            ))
        });
    }
    Err(format_kpass_error("sign-up completion", &stdout, &stderr))
}

fn describe_signup_waiting_message(email: &str) -> String {
    format!(
        "Kite Passport sign-up is waiting on email verification.\n\nA verification link and 8-character sign-up code were sent to `{email}`.\n\nNext steps\n1. Open the verification email and click the link first.\n2. Find the 8-character code from the Kite sign-up email.\n3. Run `/kite setup --code ABCD1234` to finish creating the Passport account."
    )
}

fn describe_signup_success(user: &KpassSignupExchangeResponse) -> String {
    format!(
        "Kite Passport account created and logged in.\n\nEmail: `{}`\nUser ID: `{}`\nSession: active",
        user.email, user.user_id
    )
}

fn describe_login_waiting_message(email: &str) -> String {
    format!(
        "Kite login is waiting on the one-time code.\n\nAn 8-character code was sent to `{email}`.\n\nNext step\nRun `/kite login --code ABCD1234` after you receive it."
    )
}

fn map_kpass_command_error(
    action: &str,
    stdout: &str,
    stderr: &str,
    exit_code: Option<i32>,
) -> KiteError {
    let parsed = serde_json::from_str::<KpassEnvelope>(stdout).ok();
    let detail = parsed
        .as_ref()
        .and_then(|response| {
            let error = response.error.trim();
            let hint = response.hint.trim();
            if !error.is_empty() && !hint.is_empty() {
                Some(format!("{error} {hint}"))
            } else if !error.is_empty() {
                Some(error.to_string())
            } else if !hint.is_empty() {
                Some(hint.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| format_process_output(stdout.as_bytes(), stderr.as_bytes()));
    let normalized = detail.to_ascii_lowercase();
    if normalized.contains("not logged in")
        || normalized.contains("authenticate")
        || normalized.contains("invalid otp")
        || normalized.contains("expired session")
    {
        KiteError::AuthRequired(format!("Kite {action} needs you to log in first. {detail}"))
    } else if normalized.contains("agent not registered") {
        KiteError::AgentNotFound(format!(
            "Kite {action} needs a registered Thikra agent first. Open Sessions in the Kite hub or run `/kite session create ...` after logging in. {detail}"
        ))
    } else if normalized.contains("session not found") || normalized.contains("no active sessions") {
        KiteError::SessionCreationRequired(format!(
            "Kite {action} needs an active spending session first. Create one in Sessions before retrying. {detail}"
        ))
    } else if normalized.contains("insufficient_balance")
        || normalized.contains("transfer amount exceeds balance")
    {
        KiteError::InsufficientBudget(format!(
            "Kite {action} needs more wallet balance before it can continue. {detail}"
        ))
    } else if exit_code == Some(3) {
        KiteError::AuthRequired(format!("Kite {action} failed authentication. {detail}"))
    } else if exit_code == Some(4) {
        KiteError::BadInput(format!("Kite {action} could not find that record. {detail}"))
    } else {
        KiteError::BadInput(format!("Kite {action} failed. {detail}"))
    }
}

fn run_kpass_json_value(
    cli_path: &Path,
    args: &[&str],
    envs: &[(&str, &str)],
    action: &str,
) -> Result<Value, KiteError> {
    let (stdout, stderr, exit_code) = run_kpass_json_command(cli_path, args, envs)?;
    if exit_code == Some(0) {
        return serde_json::from_str::<Value>(&stdout).map_err(|err| {
            KiteError::Unknown(format!("Kite returned unreadable {action} data. {err}"))
        });
    }
    Err(map_kpass_command_error(action, &stdout, &stderr, exit_code))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn bool_field(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn integer_field(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn array_field<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn format_decimal_amount(raw: &str, decimals: u64, symbol: &str) -> Option<String> {
    if raw.trim().is_empty() {
        return None;
    }
    let raw = raw.trim();
    let digits = raw.strip_prefix('-').unwrap_or(raw);
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(format!("{raw} {symbol}"));
    }
    let negative = raw.starts_with('-');
    let decimals = decimals as usize;
    let digits = digits.trim_start_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let rendered = if decimals == 0 {
        digits.to_string()
    } else if digits.len() <= decimals {
        let padded = format!("{digits:0>width$}", width = decimals);
        format!("0.{}", padded.trim_end_matches('0'))
    } else {
        let split = digits.len() - decimals;
        let whole = &digits[..split];
        let frac = digits[split..].trim_end_matches('0');
        if frac.is_empty() {
            whole.to_string()
        } else {
            format!("{whole}.{frac}")
        }
    };
    let signed = if negative && rendered != "0" {
        format!("-{rendered}")
    } else {
        rendered
    };
    Some(format!("{signed} {symbol}"))
}

fn load_kite_account_summary(
    conn: &rusqlite::Connection,
    cli_path: Option<&Path>,
) -> KiteAccountSummary {
    let signup_email = load_optional_setting(conn, KITE_SIGNUP_EMAIL_KEY);
    let pending_signup_id = load_optional_setting(conn, KITE_PENDING_SIGNUP_ID_KEY);
    let pending_login_id = load_optional_setting(conn, KITE_PENDING_LOGIN_ID_KEY);
    let auth_state = load_setup_status(conn, cli_path.map(Path::to_path_buf)).auth_state;
    let me = cli_path.and_then(|path| load_current_kite_user(path).ok()).flatten();
    let current_agent_identity = cli_path
        .and_then(|path| load_registered_agent_identity(path).ok())
        .flatten();
    KiteAccountSummary {
        logged_in: me.is_some(),
        email: me.as_ref().map(|user| user.email.clone()),
        user_id: me.as_ref().map(|user| user.user_id.clone()),
        signup_email,
        pending_signup_id,
        pending_login_id,
        current_agent_identity,
        auth_state,
    }
}

fn load_registered_agent_identity(cli_path: &Path) -> Result<Option<String>, KiteError> {
    let value = run_kpass_json_value(
        cli_path,
        &["user", "agents", "--agent-type", KITE_AGENT_TYPE, "--output", "json", "--no-interactive"],
        &[],
        "agent list",
    )?;
    let Some(agent) = array_field(&value, "agents").first() else {
        return Ok(None);
    };
    let id = string_field(agent, "id");
    let agent_type = string_field(agent, "type");
    Ok(match (id, agent_type) {
        (Some(id), Some(agent_type)) => Some(format!("{id} ({agent_type})")),
        (Some(id), None) => Some(id),
        _ => None,
    })
}

fn load_kite_wallet_summary(cli_path: &Path) -> Result<KiteWalletSummary, KiteError> {
    let value = run_kpass_json_value(
        cli_path,
        &["wallet", "balance", "--output", "json", "--no-interactive"],
        &[],
        "wallet lookup",
    )?;
    let response: KpassWalletBalanceResponse = serde_json::from_value(value).map_err(|err| {
        KiteError::Unknown(format!("Kite returned unreadable wallet data. {err}"))
    })?;
    Ok(KiteWalletSummary {
        wallet_address: response.wallet_address,
        wallet_type: response.wallet_type,
        chain_id: response.chain_id,
        can_use_faucet: response.chain_id == 2368,
        assets: response.assets,
    })
}

fn kite_wallet_send_inner(
    cli_path: &Path,
    to: &str,
    amount: &str,
    asset: &str,
) -> Result<KiteWalletTransfer, KiteError> {
    let value = run_kpass_json_value(
        cli_path,
        &[
            "wallet",
            "send",
            "--to",
            to,
            "--amount",
            amount,
            "--asset",
            asset,
            "--output",
            "json",
            "--no-interactive",
        ],
        &[],
        "wallet transfer",
    )?;
    let response: KpassWalletTransferResponse =
        serde_json::from_value(value).map_err(|err| {
            KiteError::Unknown(format!("Kite returned unreadable transfer data. {err}"))
        })?;
    Ok(KiteWalletTransfer {
        wallet_address: response.wallet_address,
        recipient_address: response.recipient_address,
        asset: response.asset,
        amount: response.amount,
        transaction_hash: response.transaction_hash,
        chain_id: response.chain_id,
    })
}

fn kite_faucet_drop_inner(cli_path: &Path, token: &str) -> Result<KiteWalletTransfer, KiteError> {
    let wallet = load_kite_wallet_summary(cli_path)?;
    if wallet.chain_id != 2368 {
        return Err(KiteError::BadInput(
            "Kite's faucet is only available on testnet wallets (chain_id 2368).".to_string(),
        ));
    }
    let value = run_kpass_json_value(
        cli_path,
        &[
            "faucet",
            "drop",
            "--recipient",
            &wallet.wallet_address,
            "--token",
            token,
            "--output",
            "json",
            "--no-interactive",
        ],
        &[],
        "faucet drop",
    )?;
    let response: KpassWalletTransferResponse =
        serde_json::from_value(value).map_err(|err| {
            KiteError::Unknown(format!("Kite returned unreadable faucet data. {err}"))
        })?;
    Ok(KiteWalletTransfer {
        wallet_address: response.wallet_address,
        recipient_address: response.recipient_address,
        asset: response.asset,
        amount: response.amount,
        transaction_hash: response.transaction_hash,
        chain_id: response.chain_id,
    })
}

fn load_kite_sessions(cli_path: &Path, selected_session_id: Option<&str>) -> Result<Vec<KiteSessionSummary>, KiteError> {
    let value = run_kpass_json_value(
        cli_path,
        &["user", "sessions", "--output", "json", "--no-interactive"],
        &[],
        "session list",
    )?;
    Ok(array_field(&value, "sessions")
        .iter()
        .map(|session| {
            let payment_policy = session
                .get("delegation")
                .and_then(|delegation| delegation.get("payment_policy"))
                .cloned()
                .unwrap_or(Value::Null);
            let task_summary = session
                .get("delegation")
                .and_then(|delegation| delegation.get("task"))
                .and_then(|task| string_field(task, "summary"));
            let assets = array_field(&payment_policy, "assets")
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>();
            let session_id = string_field(session, "id").unwrap_or_else(|| "unknown".to_string());
            KiteSessionSummary {
                id: session_id.clone(),
                status: string_field(session, "status").unwrap_or_else(|| "unknown".to_string()),
                agent_type: string_field(session, "agent_type")
                    .unwrap_or_else(|| KITE_AGENT_TYPE.to_string()),
                expires_at: string_field(session, "expires_at"),
                task_summary,
                assets,
                max_amount_per_tx: string_field(&payment_policy, "max_amount_per_tx"),
                max_total_amount: string_field(&payment_policy, "max_total_amount"),
                spent_total: session
                    .get("usage")
                    .and_then(|usage| string_field(usage, "spent_total")),
                reserved_total: session
                    .get("usage")
                    .and_then(|usage| string_field(usage, "reserved_total")),
                selected: selected_session_id.is_some_and(|selected| selected == session_id),
            }
        })
        .collect())
}

fn ensure_kite_agent_registered(cli_path: &Path) -> Result<(), KiteError> {
    if load_registered_agent_identity(cli_path)?.is_some() {
        return Ok(());
    }
    run_kpass_json_value(
        cli_path,
        &[
            "agent:register",
            "--type",
            KITE_AGENT_TYPE,
            "--output",
            "json",
            "--no-interactive",
        ],
        &[],
        "agent registration",
    )?;
    Ok(())
}

fn create_kite_session(
    cli_path: &Path,
    max_amount_per_tx: &str,
    ttl: &str,
    max_total_amount: Option<&str>,
    assets: Option<&str>,
    payment_approach: Option<&str>,
    task_summary: Option<&str>,
) -> Result<KiteSessionRequestStatus, KiteError> {
    ensure_kite_agent_registered(cli_path)?;
    let mut args = vec![
        "agent:session".to_string(),
        "create".to_string(),
        "--max-amount-per-tx".to_string(),
        max_amount_per_tx.to_string(),
        "--ttl".to_string(),
        ttl.to_string(),
    ];
    if let Some(total) = max_total_amount {
        args.extend(["--max-total-amount".to_string(), total.to_string()]);
    }
    if let Some(assets) = assets {
        args.extend(["--assets".to_string(), assets.to_string()]);
    }
    if let Some(payment_approach) = payment_approach {
        args.extend([
            "--payment-approach".to_string(),
            payment_approach.to_string(),
        ]);
    }
    if let Some(task_summary) = task_summary {
        args.extend(["--task-summary".to_string(), task_summary.to_string()]);
    }
    args.extend([
        "--output".to_string(),
        "json".to_string(),
        "--no-interactive".to_string(),
    ]);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let value = run_kpass_json_value(cli_path, &refs, &[], "session creation")?;
    Ok(KiteSessionRequestStatus {
        request_id: string_field(&value, "request_id")
            .or_else(|| string_field(&value, "session_request_id"))
            .unwrap_or_else(|| "pending".to_string()),
        status: string_field(&value, "status").unwrap_or_else(|| "pending".to_string()),
        session_id: string_field(&value, "session_id"),
        approval_url: string_field(&value, "approval_url"),
        message: string_field(&value, "hint")
            .unwrap_or_else(|| "Approve the session in Kite, then check its status.".to_string()),
    })
}

fn load_kite_session_request_status(
    cli_path: &Path,
    request_id: &str,
    wait: bool,
) -> Result<KiteSessionRequestStatus, KiteError> {
    let mut args = vec![
        "agent:session".to_string(),
        "status".to_string(),
        "--request-id".to_string(),
        request_id.to_string(),
    ];
    if wait {
        args.push("--wait".to_string());
    }
    args.extend([
        "--output".to_string(),
        "json".to_string(),
        "--no-interactive".to_string(),
    ]);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    let value = run_kpass_json_value(cli_path, &refs, &[], "session approval status")?;
    Ok(KiteSessionRequestStatus {
        request_id: string_field(&value, "request_id").unwrap_or_else(|| request_id.to_string()),
        status: string_field(&value, "status")
            .or_else(|| string_field(&value, "approval_status"))
            .unwrap_or_else(|| "unknown".to_string()),
        session_id: string_field(&value, "session_id"),
        approval_url: string_field(&value, "approval_url"),
        message: string_field(&value, "hint")
            .unwrap_or_else(|| "Kite returned the latest session approval status.".to_string()),
    })
}

fn kite_use_session_inner(
    db: &crate::history::Database,
    cli_path: &Path,
    session_id: &str,
) -> Result<KiteSessionSummary, KiteError> {
    run_kpass_json_value(
        cli_path,
        &[
            "agent:session",
            "use",
            "--session-id",
            session_id,
            "--output",
            "json",
            "--no-interactive",
        ],
        &[],
        "session selection",
    )?;
    store_string_setting(db, KITE_ACTIVE_SPENDING_SESSION_KEY, session_id)?;
    let sessions = load_kite_sessions(cli_path, Some(session_id))?;
    sessions
        .into_iter()
        .find(|session| session.id == session_id)
        .ok_or_else(|| KiteError::Unknown("Kite selected the session, but Thikra could not reload it.".to_string()))
}

fn load_kite_activity_events(
    cli_path: &Path,
    kind: Option<&str>,
    limit: usize,
) -> Result<Vec<KiteActivityEvent>, KiteError> {
    let mut owned_args = vec![
        "activity".to_string(),
        "--limit".to_string(),
        limit.to_string(),
        "--output".to_string(),
        "json".to_string(),
        "--no-interactive".to_string(),
    ];
    if let Some(kind) = kind {
        owned_args.splice(1..1, ["--kind".to_string(), kind.to_string()]);
    }
    let refs = owned_args.iter().map(String::as_str).collect::<Vec<_>>();
    let value = run_kpass_json_value(cli_path, &refs, &[], "activity feed")?;
    Ok(array_field(&value, "events")
        .iter()
        .map(|event| {
            let transaction = event
                .get("details")
                .and_then(|details| details.get("transaction"))
                .cloned()
                .unwrap_or(Value::Null);
            let shopping = transaction.get("shopping").cloned().unwrap_or(Value::Null);
            let symbol = string_field(&transaction, "asset_symbol").unwrap_or_default();
            let amount_display = string_field(&shopping, "total_amount_display").or_else(|| {
                let raw = string_field(&transaction, "amount_raw")?;
                let decimals = transaction
                    .get("decimals")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                format_decimal_amount(&raw, decimals, &symbol)
            });
            KiteActivityEvent {
                id: string_field(event, "id").unwrap_or_else(|| "activity".to_string()),
                kind: string_field(event, "kind").unwrap_or_else(|| "unknown".to_string()),
                status: string_field(event, "status").unwrap_or_else(|| "unknown".to_string()),
                title: string_field(event, "title").unwrap_or_else(|| "Kite activity".to_string()),
                occurred_at: string_field(event, "occurred_at").unwrap_or_else(|| "unknown".to_string()),
                amount_display,
                chain_name: string_field(&transaction, "chain_name"),
                tx_hash: string_field(&transaction, "tx_hash"),
                order_id: string_field(&shopping, "order_id"),
                item_titles: array_field(&shopping, "item_titles")
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
                error_message: string_field(event, "error_message"),
            }
        })
        .collect())
}

fn kite_shop_search_inner(cli_path: &Path, query: &str) -> Result<Vec<KiteShopItem>, KiteError> {
    let value = run_kpass_json_value(
        cli_path,
        &[
            "shop:search",
            "--query",
            query,
            "--output",
            "json",
            "--no-interactive",
        ],
        &[],
        "shopping search",
    )?;
    Ok(array_field(&value, "items")
        .iter()
        .map(|item| KiteShopItem {
            provider: string_field(item, "provider").unwrap_or_else(|| "unknown".to_string()),
            external_identifier: string_field(item, "external_identifier")
                .unwrap_or_else(|| "unknown".to_string()),
            title: string_field(item, "title").unwrap_or_else(|| "Untitled product".to_string()),
            price: string_field(item, "price").unwrap_or_else(|| "Unknown".to_string()),
            rating: item
                .get("rating")
                .map(|rating| match rating {
                    Value::Number(number) => number.to_string(),
                    Value::String(text) => text.clone(),
                    _ => String::new(),
                })
                .filter(|text| !text.is_empty()),
            reviews: item
                .get("reviews")
                .map(|reviews| match reviews {
                    Value::Number(number) => number.to_string(),
                    Value::String(text) => text.clone(),
                    _ => String::new(),
                })
                .filter(|text| !text.is_empty()),
            link: string_field(item, "link"),
            thumbnail: string_field(item, "thumbnail"),
        })
        .collect())
}

fn load_kite_shipping_summary(cli_path: &Path) -> Result<KiteShippingSummary, KiteError> {
    let value = run_kpass_json_value(
        cli_path,
        &["shop:shipping", "view", "--output", "json", "--no-interactive"],
        &[],
        "shipping profile",
    )?;
    Ok(KiteShippingSummary {
        name: string_field(&value, "name"),
        email: string_field(&value, "email"),
        line1: string_field(&value, "line1"),
        line2: string_field(&value, "line2"),
        city: string_field(&value, "city"),
        state: string_field(&value, "state"),
        postal_code: string_field(&value, "postal_code"),
        country: string_field(&value, "country"),
        complete: bool_field(&value, "complete").unwrap_or(false),
        missing: array_field(&value, "missing")
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
    })
}

fn load_kite_cart_summary(cli_path: &Path) -> Result<KiteCartSummary, KiteError> {
    let value = run_kpass_json_value(
        cli_path,
        &["shop:cart", "view", "--output", "json", "--no-interactive"],
        &[],
        "shopping cart",
    )?;
    let items = array_field(&value, "items")
        .iter()
        .map(|item| KiteCartItem {
            provider: string_field(item, "provider").unwrap_or_else(|| "unknown".to_string()),
            external_identifier: string_field(item, "external_identifier")
                .unwrap_or_else(|| "unknown".to_string()),
            product_locator: string_field(item, "product_locator")
                .unwrap_or_else(|| "unknown".to_string()),
            title: string_field(item, "title").unwrap_or_else(|| "Untitled item".to_string()),
            price: string_field(item, "price").unwrap_or_else(|| "Unknown".to_string()),
            quantity: integer_field(item, "quantity").unwrap_or(1),
            link: string_field(item, "link"),
            thumbnail: string_field(item, "thumbnail"),
        })
        .collect::<Vec<_>>();
    let shipping = load_kite_shipping_summary(cli_path).ok();
    Ok(KiteCartSummary {
        item_count: integer_field(&value, "item_count")
            .and_then(|count| usize::try_from(count).ok())
            .unwrap_or(items.len()),
        payment_currency: value
            .get("payment")
            .and_then(|payment| string_field(payment, "currency")),
        payment_chain: value
            .get("payment")
            .and_then(|payment| string_field(payment, "chain")),
        shipping,
        items,
    })
}

fn kite_cart_add_inner(
    cli_path: &Path,
    provider: &str,
    external_id: &str,
    quantity: i64,
) -> Result<KiteCartSummary, KiteError> {
    ensure_kite_agent_registered(cli_path)?;
    let mut args = vec![
        "shop:cart".to_string(),
        "add".to_string(),
        "--provider".to_string(),
        provider.to_string(),
        "--external-id".to_string(),
        external_id.to_string(),
    ];
    if quantity > 1 {
        args.extend(["--quantity".to_string(), quantity.to_string()]);
    }
    args.extend([
        "--output".to_string(),
        "json".to_string(),
        "--no-interactive".to_string(),
    ]);
    let refs = args.iter().map(String::as_str).collect::<Vec<_>>();
    run_kpass_json_value(cli_path, &refs, &[], "cart add")?;
    load_kite_cart_summary(cli_path)
}

fn kite_cart_remove_inner(
    cli_path: &Path,
    provider: &str,
    external_id: &str,
) -> Result<KiteCartSummary, KiteError> {
    ensure_kite_agent_registered(cli_path)?;
    run_kpass_json_value(
        cli_path,
        &[
            "shop:cart",
            "remove",
            "--provider",
            provider,
            "--external-id",
            external_id,
            "--output",
            "json",
            "--no-interactive",
        ],
        &[],
        "cart remove",
    )?;
    load_kite_cart_summary(cli_path)
}

fn load_kite_order_status(cli_path: &Path, order_id: &str) -> Result<KiteOrderSummary, KiteError> {
    let value = run_kpass_json_value(
        cli_path,
        &[
            "shop:order",
            "status",
            "--order-id",
            order_id,
            "--output",
            "json",
            "--no-interactive",
        ],
        &[],
        "order status",
    )?;
    Ok(KiteOrderSummary {
        order_id: string_field(&value, "order_id").unwrap_or_else(|| order_id.to_string()),
        phase: string_field(&value, "phase"),
        payment_status: string_field(&value, "payment_status"),
        tx_hash: string_field(&value, "tx_hash"),
        currency: string_field(&value, "currency"),
        chain: string_field(&value, "chain"),
        delivery_status: None,
        title: None,
    })
}

fn derive_order_summaries(events: &[KiteActivityEvent]) -> Vec<KiteOrderSummary> {
    events
        .iter()
        .filter(|event| event.kind == "shopping_checkout")
        .filter_map(|event| {
            event.order_id.as_ref().map(|order_id| KiteOrderSummary {
                order_id: order_id.clone(),
                phase: Some(event.status.clone()),
                payment_status: Some(event.status.clone()),
                tx_hash: event.tx_hash.clone(),
                currency: event.amount_display.clone(),
                chain: event.chain_name.clone(),
                delivery_status: None,
                title: Some(event.title.clone()),
            })
        })
        .collect()
}

fn kite_logout_inner(
    db: &crate::history::Database,
    cli_path: &Path,
) -> Result<KiteAccountSummary, KiteError> {
    run_kpass_json_value(
        cli_path,
        &["logout", "--output", "json", "--no-interactive"],
        &[],
        "logout",
    )?;
    store_string_setting(db, KITE_PENDING_LOGIN_ID_KEY, "")?;
    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
    Ok(load_kite_account_summary(&conn, Some(cli_path)))
}

fn get_kite_hub_state_inner(db: &crate::history::Database) -> Result<KiteHubState, KiteError> {
    let cli_path = detect_kite_cli();
    let conn =
        db.0.lock()
            .map_err(|e: std::sync::PoisonError<_>| KiteError::Unknown(e.to_string()))?;
    let setup = load_setup_status(&conn, cli_path.clone());
    let account = load_kite_account_summary(&conn, cli_path.as_deref());
    let selected_session_id = load_optional_setting(&conn, KITE_ACTIVE_SPENDING_SESSION_KEY);
    drop(conn);

    let mut issues = Vec::new();
    let mut wallet = None;
    let mut sessions = Vec::new();
    let mut activity = Vec::new();
    let mut cart = None;
    let mut orders = Vec::new();

    if let Some(cli_path) = cli_path.as_deref() {
        match load_kite_wallet_summary(cli_path) {
            Ok(summary) => wallet = Some(summary),
            Err(err) => issues.push(err.message().to_string()),
        }
        match load_kite_sessions(cli_path, selected_session_id.as_deref()) {
            Ok(summary) => sessions = summary,
            Err(err) => issues.push(err.message().to_string()),
        }
        match load_kite_activity_events(cli_path, None, 8) {
            Ok(events) => {
                orders = derive_order_summaries(&events);
                activity = events;
            }
            Err(err) => issues.push(err.message().to_string()),
        }
        match load_kite_cart_summary(cli_path) {
            Ok(summary) => cart = Some(summary),
            Err(err) => issues.push(err.message().to_string()),
        }
    }

    Ok(KiteHubState {
        developer: KiteDeveloperSummary {
            auth_state: setup.auth_state.clone(),
            connected: setup.connected,
            masked_mcp_url: setup.masked_mcp_url.clone(),
            last_payer_addr: setup.last_payer_addr.clone(),
            current_session_id: selected_session_id,
        },
        setup,
        account,
        wallet,
        sessions,
        activity,
        cart,
        orders,
        issues,
    })
}

fn load_setup_status(conn: &rusqlite::Connection, cli: Option<PathBuf>) -> KiteSetupStatus {
    let raw_url = crate::database::get_config(conn, KITE_MCP_URL_KEY)
        .ok()
        .flatten()
        .unwrap_or_default();
    let cli_path = cli.as_ref().map(|path| path.display().to_string());
    let cli_installed = cli.is_some();
    let auth_state = crate::database::get_config(conn, KITE_AUTH_STATE_KEY)
        .ok()
        .flatten()
        .map(|value| KiteAuthState::from_storage(&value))
        .unwrap_or_else(|| {
            if !cli_installed {
                KiteAuthState::CliMissing
            } else if raw_url.trim().is_empty() {
                KiteAuthState::MissingMcpUrl
            } else {
                KiteAuthState::Unverified
            }
        });
    let last_payer_addr = crate::database::get_config(conn, KITE_LAST_PAYER_KEY)
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty());
    let signup_email = load_optional_setting(conn, KITE_SIGNUP_EMAIL_KEY);
    let pending_signup_id = load_optional_setting(conn, KITE_PENDING_SIGNUP_ID_KEY);
    let masked_mcp_url = if raw_url.trim().is_empty() {
        None
    } else {
        Some(mask_mcp_url(&raw_url))
    };
    let connected = matches!(auth_state, KiteAuthState::Ready);
    KiteSetupStatus {
        cli_installed,
        cli_path,
        mcp_url_configured: !raw_url.trim().is_empty(),
        masked_mcp_url,
        auth_state,
        connected,
        last_payer_addr,
        signup_email,
        pending_signup_id,
        invite_only: true,
        docs_url: KITE_DOCS_URL.to_string(),
        portal_url: KITE_PORTAL_URL.to_string(),
        installer_url: KITE_INSTALLER_URL.to_string(),
    }
}

fn detect_kite_cli() -> Option<PathBuf> {
    if let Ok(output) = Command::new("where.exe").arg("kpass").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(path) = stdout.lines().map(str::trim).find(|line| !line.is_empty()) {
                return Some(PathBuf::from(path));
            }
        }
    }
    kite_cli_candidates()
        .into_iter()
        .find(|candidate| candidate.exists())
}

fn install_kite_cli_inner() -> Result<KiteCliInstallOutcome, KiteError> {
    if let Some(path) = detect_kite_cli() {
        return ensure_kite_agent_passport_bootstrap(&path);
    }

    #[cfg(target_os = "windows")]
    {
        return install_kite_cli_windows(KITE_CLI_BASE_URL);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let (program, args) = kite_installer_command();
        let output = Command::new(&program)
            .args(args.iter().map(String::as_str))
            .output()
            .map_err(|err| {
                KiteError::CliInstallFailed(format!(
                    "Thuki could not launch the Kite CLI installer using `{program}`: {err}"
                ))
            })?;

        if !output.status.success() {
            return Err(KiteError::CliInstallFailed(format!(
                "The Kite CLI installer exited with code {:?}.\n{}",
                output.status.code(),
                format_process_output(&output.stdout, &output.stderr)
            )));
        }

        let path = detect_kite_cli().ok_or_else(|| {
            KiteError::CliInstallFailed(
                "The Kite installer completed, but `kpass` was still not found on this machine. Retry the install from Settings or follow Kite's official install docs.".to_string(),
            )
        })?;
        ensure_kite_agent_passport_bootstrap(&path)
    }
}

async fn install_kite_cli_inner_async() -> Result<KiteCliInstallOutcome, KiteError> {
    run_blocking_kite_task("Kite CLI installation", install_kite_cli_inner).await
}

async fn run_blocking_kite_task<T, F>(task_name: &'static str, task: F) -> Result<T, KiteError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, KiteError> + Send + 'static,
{
    tokio::task::spawn_blocking(task).await.map_err(|err| {
        KiteError::Unknown(format!(
            "{task_name} could not finish because its background worker failed: {err}"
        ))
    })?
}

#[cfg(target_os = "windows")]
fn ensure_kite_bin_on_user_path(cli_path: &Path) -> Result<bool, KiteError> {
    let Some(bin_dir) = cli_path.parent() else {
        return Ok(false);
    };
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            "[Environment]::GetEnvironmentVariable('Path','User')",
        ])
        .output()
        .map_err(|err| {
            installer_stage_error(
                "user PATH update",
                format!("Could not read user PATH: {err}"),
            )
        })?;
    if !output.status.success() {
        return Err(installer_stage_error(
            "user PATH update",
            format!(
                "PowerShell exited with code {:?} while reading the user PATH.",
                output.status.code()
            ),
        ));
    }

    let current_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let bin_dir_str = bin_dir.display().to_string();
    let Some(updated_path) = merge_windows_path(&current_path, &bin_dir_str) else {
        return Ok(false);
    };
    let escaped = updated_path.replace('\'', "''");
    let status = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-Command",
            &format!("[Environment]::SetEnvironmentVariable('Path', '{escaped}', 'User')"),
        ])
        .status()
        .map_err(|err| {
            installer_stage_error(
                "user PATH update",
                format!("Could not update user PATH: {err}"),
            )
        })?;
    if !status.success() {
        return Err(installer_stage_error(
            "user PATH update",
            format!(
                "PowerShell exited with code {:?} while updating the user PATH.",
                status.code()
            ),
        ));
    }
    Ok(true)
}

#[cfg(not(target_os = "windows"))]
fn ensure_kite_bin_on_user_path(_cli_path: &Path) -> Result<bool, KiteError> {
    Ok(false)
}

#[cfg(target_os = "windows")]
fn run_kpass_skills_setup(cli_path: &Path) -> Result<bool, KiteError> {
    let output = Command::new(cli_path)
        .args(["skills", "setup", "--global", "--no-interactive"])
        .output()
        .map_err(|err| {
            installer_stage_error(
                "Kite Agent Passport bootstrap",
                format!("Could not launch `kpass skills setup`: {err}"),
            )
        })?;
    if !output.status.success() {
        return Err(installer_stage_error(
            "Kite Agent Passport bootstrap",
            format!(
                "`kpass skills setup --global --no-interactive` exited with code {:?}.\n{}",
                output.status.code(),
                format_process_output(&output.stdout, &output.stderr)
            ),
        ));
    }
    Ok(true)
}

#[cfg(not(target_os = "windows"))]
fn run_kpass_skills_setup(_cli_path: &Path) -> Result<bool, KiteError> {
    Ok(false)
}

fn merge_windows_path(current_path: &str, new_entry: &str) -> Option<String> {
    let normalized_new = new_entry.trim().trim_end_matches(['\\', '/']);
    if normalized_new.is_empty() {
        return None;
    }

    let exists = current_path
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| entry.trim_end_matches(['\\', '/']))
        .any(|entry| entry.eq_ignore_ascii_case(normalized_new));
    if exists {
        return None;
    }

    let trimmed_existing = current_path.trim().trim_end_matches(';');
    if trimmed_existing.is_empty() {
        Some(normalized_new.to_string())
    } else {
        Some(format!("{trimmed_existing};{normalized_new}"))
    }
}

fn kite_cli_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".kpass").join("bin").join("kpass.exe"));
        candidates.push(home.join(".kpass").join("bin").join("kpass"));
    }
    candidates
}

fn kite_codex_skill_marker_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        home.join(".agents")
            .join("skills")
            .join("kite-passport")
            .join("SKILL.md")
    })
}

fn ensure_kite_agent_passport_bootstrap(
    cli_path: &Path,
) -> Result<KiteCliInstallOutcome, KiteError> {
    let mut notes = Vec::new();
    let path_updated = ensure_kite_bin_on_user_path(cli_path).unwrap_or_else(|err| {
        notes.push(format!(
            "Thikra could not add `~/.kpass/bin` to the user PATH automatically: {}",
            err.message()
        ));
        false
    });
    let skills_bootstrapped = if kite_codex_skill_marker_path()
        .is_some_and(|marker| marker.exists())
    {
        false
    } else {
        run_kpass_skills_setup(cli_path).unwrap_or_else(|err| {
            notes.push(format!(
                "Thikra installed `kpass`, but the post-install Kite Agent Passport skills bootstrap did not finish automatically: {}",
                err.message()
            ));
            false
        })
    };

    Ok(KiteCliInstallOutcome {
        cli_path: cli_path.to_path_buf(),
        path_updated,
        skills_bootstrapped,
        notes,
    })
}

fn describe_kite_agent_passport_install(outcome: Option<&KiteCliInstallOutcome>) -> String {
    let Some(outcome) = outcome else {
        return "Kite Agent Passport skills were already present for this machine, so Thikra only needed to continue the MCP and portal setup flow.".to_string();
    };

    let mut lines = Vec::new();
    if outcome.skills_bootstrapped {
        lines.push("Thikra also bootstrapped the Kite Agent Passport skills globally, matching Kite's post-install setup flow.".to_string());
    } else {
        lines.push("Kite Agent Passport skills were already present, so Thikra skipped the extra bootstrap step.".to_string());
    }
    if outcome.path_updated {
        lines.push("`~/.kpass/bin` was added to the user PATH for future shells.".to_string());
    }
    lines.extend(outcome.notes.iter().cloned());
    lines.join("\n")
}

#[cfg(target_os = "windows")]
fn install_kite_cli_windows(base_url: &str) -> Result<KiteCliInstallOutcome, KiteError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|err| installer_stage_error("client setup", err))?;
    let install_dir = dirs::home_dir()
        .ok_or_else(|| {
            installer_stage_error(
                "install directory",
                "Could not resolve the user home directory.",
            )
        })?
        .join(".kpass");
    install_kite_cli_windows_with_client(&client, base_url, &install_dir)
}

#[cfg(target_os = "windows")]
fn install_kite_cli_windows_with_client(
    client: &reqwest::blocking::Client,
    base_url: &str,
    install_dir: &Path,
) -> Result<KiteCliInstallOutcome, KiteError> {
    let bundle_version = resolve_kite_bundle_version(client, base_url)?;
    let manifest = fetch_kite_bundle_manifest(client, base_url, &bundle_version)?;
    let platform = "windows-amd64";
    let cli_platform = manifest.cli.platforms.get(platform).ok_or_else(|| {
        installer_stage_error(
            "manifest validation",
            format!("Kite did not publish a `{platform}` CLI bundle for version {bundle_version}."),
        )
    })?;

    let temp_dir = std::env::temp_dir().join(format!("thikra-kite-install-{}", Uuid::new_v4()));
    fs::create_dir_all(&temp_dir)
        .map_err(|err| installer_stage_error("temp directory creation", err))?;

    let install_result = (|| -> Result<KiteCliInstallOutcome, KiteError> {
        let cli_zip = temp_dir.join("cli.zip");
        download_to_path(
            client,
            &format!(
                "{base_url}/bundle/{bundle_version}/{}",
                cli_platform.archive
            ),
            &cli_zip,
            "CLI archive download",
        )?;
        verify_optional_sha256(&cli_zip, cli_platform.checksum.as_deref(), "CLI archive")?;

        let cli_extract_dir = temp_dir.join("cli-extracted");
        extract_zip_archive(&cli_zip, &cli_extract_dir, "CLI archive extraction")?;

        let kpass_source = find_file_recursive(&cli_extract_dir, "kpass.exe").ok_or_else(|| {
            installer_stage_error(
                "final binary detection",
                "The downloaded CLI archive did not contain `kpass.exe`.",
            )
        })?;

        let mut ksearch_source: Option<PathBuf> = None;
        if let Some(ksearch_bundle) = &manifest.ksearch {
            if let Some(ksearch_platform) = ksearch_bundle.platforms.get(platform) {
                let ksearch_zip = temp_dir.join("ksearch.zip");
                download_to_path(
                    client,
                    &format!(
                        "{base_url}/bundle/{bundle_version}/{}",
                        ksearch_platform.archive
                    ),
                    &ksearch_zip,
                    "ksearch archive download",
                )?;
                verify_optional_sha256(
                    &ksearch_zip,
                    ksearch_platform.checksum.as_deref(),
                    "ksearch archive",
                )?;
                let ksearch_extract_dir = temp_dir.join("ksearch-extracted");
                extract_zip_archive(
                    &ksearch_zip,
                    &ksearch_extract_dir,
                    "ksearch archive extraction",
                )?;
                ksearch_source = find_file_recursive(&ksearch_extract_dir, "ksearch.exe");
            }
        }

        let skills_archive = temp_dir.join("skills.tar.gz");
        download_to_path(
            client,
            &format!(
                "{base_url}/bundle/{bundle_version}/{}",
                manifest.skills.archive
            ),
            &skills_archive,
            "skills archive download",
        )?;
        verify_optional_sha256(
            &skills_archive,
            manifest.skills.checksum.as_deref(),
            "skills archive",
        )?;

        let bin_dir = install_dir.join("bin");
        fs::create_dir_all(&bin_dir)
            .map_err(|err| installer_stage_error("install directory creation", err))?;
        let kpass_destination = bin_dir.join("kpass.exe");
        copy_file(&kpass_source, &kpass_destination, "CLI install copy")?;
        if let Some(ksearch_source) = ksearch_source {
            copy_file(
                &ksearch_source,
                &bin_dir.join("ksearch.exe"),
                "ksearch install copy",
            )?;
        }

        let skills_dir = install_dir.join("skills");
        if skills_dir.exists() {
            fs::remove_dir_all(&skills_dir)
                .map_err(|err| installer_stage_error("skills directory cleanup", err))?;
        }
        fs::create_dir_all(&skills_dir)
            .map_err(|err| installer_stage_error("skills directory creation", err))?;
        extract_tar_gz_archive(&skills_archive, &skills_dir, "skills archive extraction")?;

        write_kite_version_metadata(install_dir, &bundle_version, &manifest, base_url)?;

        if !kpass_destination.exists() {
            return Err(installer_stage_error(
                "final binary detection",
                "Kite install finished, but `kpass.exe` was still not found in `~/.kpass/bin`.",
            ));
        }

        ensure_kite_agent_passport_bootstrap(&kpass_destination)
    })();

    let _ = fs::remove_dir_all(&temp_dir);
    install_result
}

#[cfg(target_os = "windows")]
fn resolve_kite_bundle_version(
    client: &reqwest::blocking::Client,
    base_url: &str,
) -> Result<String, KiteError> {
    let response = client
        .get(format!("{base_url}/latest"))
        .send()
        .map_err(|err| installer_stage_error("version resolution", err))?;
    if !response.status().is_success() {
        return Err(installer_stage_error(
            "version resolution",
            format!(
                "Kite returned HTTP {} while resolving the latest bundle.",
                response.status().as_u16()
            ),
        ));
    }
    let body = response
        .text()
        .map_err(|err| installer_stage_error("version resolution", err))?;
    parse_bundle_version_from_body(&body)
}

#[cfg(target_os = "windows")]
fn parse_bundle_version_from_body(body: &str) -> Result<String, KiteError> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Err(installer_stage_error(
            "version resolution",
            "Kite returned an empty latest-version response.",
        ));
    }

    match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::String(value)) if !value.trim().is_empty() => Ok(value.trim().to_string()),
        Ok(Value::Number(value)) => Ok(value.to_string()),
        Ok(_) => Err(installer_stage_error(
            "version resolution",
            format!("Kite returned an unsupported latest-version payload: {trimmed}"),
        )),
        Err(_) => {
            let normalized = trimmed.trim_matches('"').trim();
            if normalized.is_empty() {
                Err(installer_stage_error(
                    "version resolution",
                    "Kite returned a blank latest-version value.",
                ))
            } else {
                Ok(normalized.to_string())
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn fetch_kite_bundle_manifest(
    client: &reqwest::blocking::Client,
    base_url: &str,
    bundle_version: &str,
) -> Result<KiteBundleManifest, KiteError> {
    let response = client
        .get(format!("{base_url}/bundle/{bundle_version}/manifest.json"))
        .send()
        .map_err(|err| installer_stage_error("manifest fetch", err))?;
    if !response.status().is_success() {
        return Err(installer_stage_error(
            "manifest fetch",
            format!(
                "Kite returned HTTP {} while fetching bundle {bundle_version}.",
                response.status().as_u16()
            ),
        ));
    }
    response
        .json::<KiteBundleManifest>()
        .map_err(|err| installer_stage_error("manifest fetch", err))
}

#[cfg(target_os = "windows")]
fn download_to_path(
    client: &reqwest::blocking::Client,
    url: &str,
    destination: &Path,
    stage: &str,
) -> Result<(), KiteError> {
    let mut response = client
        .get(url)
        .send()
        .map_err(|err| installer_stage_error(stage, err))?;
    if !response.status().is_success() {
        return Err(installer_stage_error(
            stage,
            format!(
                "Kite returned HTTP {} for {url}.",
                response.status().as_u16()
            ),
        ));
    }
    let mut file = File::create(destination).map_err(|err| installer_stage_error(stage, err))?;
    io::copy(&mut response, &mut file).map_err(|err| installer_stage_error(stage, err))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn verify_optional_sha256(
    file_path: &Path,
    expected: Option<&str>,
    label: &str,
) -> Result<(), KiteError> {
    let Some(expected) = expected else {
        return Ok(());
    };
    let normalized = expected.trim().trim_start_matches("sha256:");
    if normalized.is_empty() || normalized == "null" {
        return Ok(());
    }
    let mut file =
        File::open(file_path).map_err(|err| installer_stage_error("checksum verification", err))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| installer_stage_error("checksum verification", err))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != normalized {
        return Err(installer_stage_error(
            "checksum verification",
            format!("{label} checksum mismatch. Expected {normalized}, got {actual}."),
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn extract_zip_archive(
    archive_path: &Path,
    destination: &Path,
    stage: &str,
) -> Result<(), KiteError> {
    fs::create_dir_all(destination).map_err(|err| installer_stage_error(stage, err))?;
    let file = File::open(archive_path).map_err(|err| installer_stage_error(stage, err))?;
    let mut archive = ZipArchive::new(file).map_err(|err| installer_stage_error(stage, err))?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| installer_stage_error(stage, err))?;
        let Some(name) = entry.enclosed_name().map(|name| name.to_owned()) else {
            continue;
        };
        let out_path = destination.join(name);
        if entry.is_dir() {
            fs::create_dir_all(&out_path).map_err(|err| installer_stage_error(stage, err))?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|err| installer_stage_error(stage, err))?;
        }
        let mut out_file =
            File::create(&out_path).map_err(|err| installer_stage_error(stage, err))?;
        io::copy(&mut entry, &mut out_file).map_err(|err| installer_stage_error(stage, err))?;
        out_file
            .flush()
            .map_err(|err| installer_stage_error(stage, err))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn extract_tar_gz_archive(
    archive_path: &Path,
    destination: &Path,
    stage: &str,
) -> Result<(), KiteError> {
    let file = File::open(archive_path).map_err(|err| installer_stage_error(stage, err))?;
    let gz = GzDecoder::new(file);
    let mut archive = Archive::new(gz);
    archive
        .unpack(destination)
        .map_err(|err| installer_stage_error(stage, err))
}

#[cfg(target_os = "windows")]
fn find_file_recursive(root: &Path, file_name: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(file_name))
            {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn copy_file(source: &Path, destination: &Path, stage: &str) -> Result<(), KiteError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|err| installer_stage_error(stage, err))?;
    }
    fs::copy(source, destination).map_err(|err| installer_stage_error(stage, err))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn write_kite_version_metadata(
    install_dir: &Path,
    bundle_version: &str,
    manifest: &KiteBundleManifest,
    base_url: &str,
) -> Result<(), KiteError> {
    let installed_at_unix_s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let payload = serde_json::to_vec_pretty(&json!({
        "bundle_version": bundle_version,
        "cli_version": manifest.cli.version,
        "ksearch_version": manifest.ksearch.as_ref().map(|bundle| bundle.version.clone()),
        "skills_version": manifest.skills.version,
        "channel": "latest",
        "platform": "windows-amd64",
        "base_url": base_url,
        "installed_at_unix_s": installed_at_unix_s
    }))
    .map_err(|err| installer_stage_error("version metadata write", err))?;
    fs::write(install_dir.join("version.json"), payload)
        .map_err(|err| installer_stage_error("version metadata write", err))
}

fn installer_stage_error(stage: &str, detail: impl std::fmt::Display) -> KiteError {
    KiteError::CliInstallFailed(format!(
        "Kite CLI installation failed during {stage}: {detail}"
    ))
}

#[cfg(not(target_os = "windows"))]
fn kite_installer_command() -> (String, Vec<String>) {
    (
        "sh".to_string(),
        vec![
            "-c".to_string(),
            format!("curl -fsSL {} | bash", KITE_INSTALLER_URL),
        ],
    )
}

fn format_process_output(stdout: &[u8], stderr: &[u8]) -> String {
    let stderr_text = String::from_utf8_lossy(stderr).trim().to_string();
    let stdout_text = String::from_utf8_lossy(stdout).trim().to_string();

    if !stderr_text.is_empty() && !stdout_text.is_empty() {
        format!("stderr:\n{}\n\nstdout:\n{}", stderr_text, stdout_text)
    } else if !stderr_text.is_empty() {
        format!("stderr:\n{}", stderr_text)
    } else if !stdout_text.is_empty() {
        format!("stdout:\n{}", stdout_text)
    } else {
        "The installer did not return any output.".to_string()
    }
}

fn kite_target_url(target: &str) -> Option<&'static str> {
    match target.trim().to_ascii_lowercase().as_str() {
        "portal" | "invite" => Some(KITE_PORTAL_URL),
        "installer" | "install" => Some(KITE_INSTALLER_URL),
        "docs" | "developer-guide" => Some(KITE_DOCS_URL),
        _ => None,
    }
}

fn open_external_target(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
            .map_err(|err| err.to_string())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|err| err.to_string())
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|err| err.to_string())
    }
}

fn kite_help_text() -> String {
    [
        "Kite commands",
        "/kite setup",
        "/kite setup --email you@example.com",
        "/kite setup --code ABCD1234",
        "/kite login --email you@example.com",
        "/kite login --code ABCD1234",
        "/kite logout",
        "/kite me",
        "/kite connect",
        "/kite status",
        "/kite wallet",
        "/kite send --to 0xabc --amount 5 --asset USDC",
        "/kite faucet --token USDC",
        "/kite sessions",
        "/kite session create --max-amount 5 --ttl 1h --total 50 --assets USDC --task \"Use paid APIs\"",
        "/kite session use --session-id session_123",
        "/kite session status --request-id req_123 --wait yes",
        "/kite activity",
        "/kite shop search --query \"usb c cable\"",
        "/kite cart",
        "/kite checkout --confirmed yes",
        "/kite orders",
        "/kite orders --order-id ord_123",
        "/kite payer",
        "/kite approve --payee <addr> --amount <amount> --token <symbol> [--merchant <name>]",
        "/kite call --url <https://...> [--method GET|POST] [--body <json>] [--merchant <name>]",
        "",
        "Mode 1 note",
        "Thikra can install Kite Passport CLI and start account sign-up. Wallet and agent provisioning still happen in the Kite Portal before you paste the MCP URL into Settings > Agent > Kite Passport.",
    ]
    .join("\n")
}

fn format_verify_response(response: &KiteVerifyResponse) -> String {
    let tool_line = if response.available_tools.is_empty() {
        "(none)".to_string()
    } else {
        response.available_tools.join(", ")
    };
    format!(
        "Kite status\n{}\nAuth state: {:?}\nTools: {}",
        response.message, response.auth_state, tool_line
    )
}

fn parse_kite_command(input: &str) -> Result<KiteCommand, KiteError> {
    let tokens = tokenize_command(input)?;
    let mut filtered = tokens
        .into_iter()
        .filter(|token| token != "/kite")
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        return Ok(KiteCommand::Help);
    }

    let subcommand = filtered.remove(0).to_ascii_lowercase();
    let args = parse_flag_args(&filtered)?;
    match subcommand.as_str() {
        "help" => Ok(KiteCommand::Help),
        "setup" => Ok(KiteCommand::Setup {
            email: args.optional("email"),
            code: args.optional("code"),
        }),
        "login" => Ok(KiteCommand::Login {
            email: args.optional("email"),
            code: args.optional("code"),
        }),
        "logout" => Ok(KiteCommand::Logout),
        "me" => Ok(KiteCommand::Me),
        "connect" => Ok(KiteCommand::Connect),
        "status" => Ok(KiteCommand::Status),
        "wallet" => Ok(KiteCommand::Wallet),
        "send" => Ok(KiteCommand::Send {
            to: args.required("to")?,
            amount: args.required("amount")?,
            asset: args.required("asset")?,
        }),
        "faucet" => Ok(KiteCommand::Faucet {
            token: args.required("token")?,
        }),
        "sessions" => Ok(KiteCommand::Sessions),
        "session" => {
            let action = args.free_first().ok_or_else(|| {
                KiteError::BadInput(
                    "Use `/kite session create`, `/kite session use`, or `/kite session status`.".to_string(),
                )
            })?;
            match action.as_str() {
                "create" => Ok(KiteCommand::SessionCreate {
                    max_amount_per_tx: args.required("max-amount")?,
                    ttl: args.required("ttl")?,
                    max_total_amount: args.optional("total"),
                    assets: args.optional("assets"),
                    payment_approach: args.optional("payment-approach"),
                    task_summary: args.optional("task"),
                }),
                "use" => Ok(KiteCommand::SessionUse {
                    session_id: args.required("session-id")?,
                }),
                "status" => Ok(KiteCommand::SessionStatus {
                    request_id: args.required("request-id")?,
                    wait: args
                        .optional("wait")
                        .as_deref()
                        .map(parse_boolish)
                        .transpose()?
                        .unwrap_or(false),
                }),
                other => Err(KiteError::BadInput(format!(
                    "Unknown Kite session action `{other}`. Use create, use, or status."
                ))),
            }
        }
        "activity" => Ok(KiteCommand::Activity {
            kind: args.optional("kind"),
        }),
        "shop" => {
            let action = args.free_first().ok_or_else(|| {
                KiteError::BadInput("Use `/kite shop search --query ...`.".to_string())
            })?;
            match action.as_str() {
                "search" => Ok(KiteCommand::ShopSearch {
                    query: args.required("query")?,
                }),
                other => Err(KiteError::BadInput(format!(
                    "Unknown Kite shop action `{other}`. Use `/kite shop search --query ...`."
                ))),
            }
        }
        "cart" => Ok(KiteCommand::Cart),
        "checkout" => Ok(KiteCommand::Checkout {
            confirmed: args
                .optional("confirmed")
                .as_deref()
                .map(parse_boolish)
                .transpose()?
                .unwrap_or(false),
        }),
        "orders" => Ok(KiteCommand::Orders {
            order_id: args.optional("order-id"),
        }),
        "payer" => Ok(KiteCommand::Payer),
        "approve" => Ok(KiteCommand::Approve {
            payee_addr: args.required("payee")?,
            amount: args.required("amount")?,
            token_type: args.required("token")?,
            merchant_name: args.optional("merchant"),
        }),
        "call" => Ok(KiteCommand::Call {
            url: args.required("url")?,
            method: args.optional("method").unwrap_or_else(|| "GET".to_string()),
            body: args.optional("body"),
            merchant_name: args.optional("merchant"),
        }),
        other => Err(KiteError::BadInput(format!(
            "Unknown Kite subcommand `{other}`. Use `/kite help` to see the supported commands."
        ))),
    }
}

fn tokenize_command(input: &str) -> Result<Vec<String>, KiteError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escape = false;
    for ch in input.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        if ch == '\\' {
            escape = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            } else {
                current.push(ch);
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            quote = Some(ch);
            continue;
        }
        if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(ch);
    }
    if escape || quote.is_some() {
        return Err(KiteError::BadInput(
            "Could not parse the Kite command. Close any open quotes and try again.".to_string(),
        ));
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct FlagArgs {
    values: HashMap<String, String>,
    positionals: Vec<String>,
}

impl FlagArgs {
    fn required(&self, key: &str) -> Result<String, KiteError> {
        self.values
            .get(key)
            .cloned()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| KiteError::BadInput(format!("Missing required `--{key}` value.")))
    }

    fn optional(&self, key: &str) -> Option<String> {
        self.values
            .get(key)
            .cloned()
            .filter(|value| !value.trim().is_empty())
    }

    fn free_first(&self) -> Option<String> {
        self.positionals.first().cloned()
    }
}

fn parse_flag_args(tokens: &[String]) -> Result<FlagArgs, KiteError> {
    let mut values = HashMap::new();
    let mut positionals = Vec::new();
    let mut idx = 0;
    while idx < tokens.len() {
        if let Some(flag) = tokens[idx].strip_prefix("--") {
            let value = tokens
                .get(idx + 1)
                .ok_or_else(|| KiteError::BadInput(format!("Missing value for `--{flag}`.")))?;
            if value.starts_with("--") {
                return Err(KiteError::BadInput(format!(
                    "Missing value for `--{flag}`."
                )));
            }
            values.insert(flag.to_string(), value.clone());
            idx += 2;
        } else {
            positionals.push(tokens[idx].clone());
            idx += 1;
        }
    }
    Ok(FlagArgs { values, positionals })
}

fn parse_boolish(raw: &str) -> Result<bool, KiteError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" => Ok(true),
        "0" | "false" | "no" | "n" => Ok(false),
        _ => Err(KiteError::BadInput(format!(
            "Use `yes` or `no` for boolean Kite flags like `{raw}`."
        ))),
    }
}

fn parse_http_method(raw: &str) -> Result<Method, KiteError> {
    Method::from_bytes(raw.trim().to_ascii_uppercase().as_bytes()).map_err(|_| {
        KiteError::BadInput(format!(
            "Unsupported HTTP method `{raw}`. Use GET, POST, PUT, PATCH, or DELETE."
        ))
    })
}

fn mask_mcp_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return "(not set)".to_string();
    }
    if let Some((scheme, rest)) = trimmed.split_once("://") {
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        let masked_path = if path.len() <= 8 {
            "…".to_string()
        } else {
            format!("…{}", &path[path.len() - 8..])
        };
        return format!("{scheme}://{host}/{masked_path}");
    }
    if trimmed.len() <= 12 {
        "…".to_string()
    } else {
        format!("…{}", &trimmed[trimmed.len() - 12..])
    }
}

fn map_http_status(status: StatusCode, body: String) -> KiteError {
    match status {
        StatusCode::BAD_REQUEST => map_kite_failure(&body),
        StatusCode::UNAUTHORIZED => KiteError::Unauthorized(
            "Kite rejected the request as unauthorized. Reconnect the MCP server or restart the OAuth/session flow in the Kite Portal.".to_string(),
        ),
        StatusCode::NOT_FOUND => KiteError::SessionExpired(
            "The Kite MCP session was no longer valid. Reconnect the MCP URL or create a new session in the Kite Portal.".to_string(),
        ),
        _ => KiteError::Network(format!("Kite MCP returned HTTP {}.\n{}", status.as_u16(), body.trim())),
    }
}

fn format_jsonrpc_error(error: &JsonRpcError) -> String {
    if let Some(data) = error.data.as_ref() {
        if let Some(text) = data.as_str() {
            return format!("{}\n{}", error.message, text);
        }
        return format!("{}\n{}", error.message, data);
    }
    error.message.clone()
}

fn map_kite_failure(message: &str) -> KiteError {
    let normalized = message.to_ascii_lowercase();
    if normalized.contains("session_creation_required") {
        KiteError::SessionCreationRequired(
            "Kite needs a fresh payment session before this action can continue. Complete the session flow in the Kite Portal or reconnect the MCP URL.".to_string(),
        )
    } else if normalized.contains("sessionexpired") || normalized.contains("session expired") {
        KiteError::SessionExpired(
            "Your Kite session expired. Reconnect the MCP server and create a new session in the Kite Portal.".to_string(),
        )
    } else if normalized.contains("insufficientbudget")
        || normalized.contains("insufficient budget")
    {
        KiteError::InsufficientBudget(
            "The requested payment exceeds the active Kite session budget. Create a new session with a higher limit.".to_string(),
        )
    } else if normalized.contains("unauthorized") {
        KiteError::Unauthorized(
            "Kite authorization expired. Reconnect the MCP server and complete the OAuth/session flow again.".to_string(),
        )
    } else if normalized.contains("agent not found") {
        KiteError::AgentNotFound(
            "Kite could not find the agent behind this MCP URL. Re-copy the URL from the Kite Portal and verify the agent still exists.".to_string(),
        )
    } else if normalized.contains("oauth") || normalized.contains("auth") {
        KiteError::AuthRequired(
            "Kite requires authentication before this action can continue. Reconnect the MCP URL and complete the OAuth flow in the Kite Portal.".to_string(),
        )
    } else {
        KiteError::Unknown(message.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Cursor;

    use flate2::write::GzEncoder;
    use flate2::Compression;
    use rusqlite::Connection;
    use tar::Builder;
    use zip::write::SimpleFileOptions;

    fn open_in_memory_db() -> crate::history::Database {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE app_config (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
        crate::history::Database(std::sync::Mutex::new(conn))
    }

    #[cfg(target_os = "windows")]
    fn temp_test_dir(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("kite-{label}-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[cfg(target_os = "windows")]
    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(cursor);
        let options = SimpleFileOptions::default();
        for (name, contents) in entries {
            zip.start_file(name, options).unwrap();
            zip.write_all(contents).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    #[cfg(target_os = "windows")]
    fn build_tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = Builder::new(encoder);
        for (name, contents) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_path(name).unwrap();
            header.set_size(contents.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, *contents).unwrap();
        }
        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap()
    }

    #[cfg(target_os = "windows")]
    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn parse_help_and_setup_commands() {
        assert_eq!(parse_kite_command("/kite").unwrap(), KiteCommand::Help);
        assert_eq!(parse_kite_command("/kite help").unwrap(), KiteCommand::Help);
        assert_eq!(
            parse_kite_command("/kite setup").unwrap(),
            KiteCommand::Setup {
                email: None,
                code: None,
            }
        );
        assert_eq!(
            parse_kite_command("/kite setup --email and00sama@gmail.com").unwrap(),
            KiteCommand::Setup {
                email: Some("and00sama@gmail.com".to_string()),
                code: None,
            }
        );
        assert_eq!(
            parse_kite_command("/kite setup --code A1B2C3D4").unwrap(),
            KiteCommand::Setup {
                email: None,
                code: Some("A1B2C3D4".to_string()),
            }
        );
    }

    #[test]
    fn parse_account_wallet_and_shopping_commands() {
        assert_eq!(
            parse_kite_command("/kite login --email and00sama@gmail.com").unwrap(),
            KiteCommand::Login {
                email: Some("and00sama@gmail.com".to_string()),
                code: None,
            }
        );
        assert_eq!(parse_kite_command("/kite logout").unwrap(), KiteCommand::Logout);
        assert_eq!(parse_kite_command("/kite me").unwrap(), KiteCommand::Me);
        assert_eq!(parse_kite_command("/kite wallet").unwrap(), KiteCommand::Wallet);
        assert_eq!(
            parse_kite_command("/kite send --to 0xabc --amount 5 --asset USDC").unwrap(),
            KiteCommand::Send {
                to: "0xabc".to_string(),
                amount: "5".to_string(),
                asset: "USDC".to_string(),
            }
        );
        assert_eq!(
            parse_kite_command("/kite shop search --query 'usb c cable'").unwrap(),
            KiteCommand::ShopSearch {
                query: "usb c cable".to_string(),
            }
        );
        assert_eq!(parse_kite_command("/kite cart").unwrap(), KiteCommand::Cart);
        assert_eq!(
            parse_kite_command("/kite orders --order-id ord_123").unwrap(),
            KiteCommand::Orders {
                order_id: Some("ord_123".to_string()),
            }
        );
    }

    #[test]
    fn parse_session_commands() {
        assert_eq!(
            parse_kite_command("/kite session use --session-id session_123").unwrap(),
            KiteCommand::SessionUse {
                session_id: "session_123".to_string(),
            }
        );
        assert_eq!(
            parse_kite_command(
                "/kite session create --max-amount 5 --ttl 1h --total 25 --assets USDC --payment-approach x402 --task 'Use paid weather API'"
            )
            .unwrap(),
            KiteCommand::SessionCreate {
                max_amount_per_tx: "5".to_string(),
                ttl: "1h".to_string(),
                max_total_amount: Some("25".to_string()),
                assets: Some("USDC".to_string()),
                payment_approach: Some("x402".to_string()),
                task_summary: Some("Use paid weather API".to_string()),
            }
        );
        assert_eq!(
            parse_kite_command("/kite session status --request-id req_123 --wait yes").unwrap(),
            KiteCommand::SessionStatus {
                request_id: "req_123".to_string(),
                wait: true,
            }
        );
    }

    #[test]
    fn parse_approve_command_with_optional_merchant() {
        assert_eq!(
            parse_kite_command(
                "/kite approve --payee 0xabc --amount 100 --token USDC --merchant WeatherAPI"
            )
            .unwrap(),
            KiteCommand::Approve {
                payee_addr: "0xabc".to_string(),
                amount: "100".to_string(),
                token_type: "USDC".to_string(),
                merchant_name: Some("WeatherAPI".to_string()),
            }
        );
    }

    #[test]
    fn parse_call_command_with_body() {
        assert_eq!(
            parse_kite_command("/kite call --url https://x402.dev/api/weather --method POST --body '{\"city\":\"Riyadh\"}' --merchant Weather").unwrap(),
            KiteCommand::Call {
                url: "https://x402.dev/api/weather".to_string(),
                method: "POST".to_string(),
                body: Some("{\"city\":\"Riyadh\"}".to_string()),
                merchant_name: Some("Weather".to_string()),
            }
        );
    }

    #[test]
    fn parse_missing_flag_value_errors_cleanly() {
        let err =
            parse_kite_command("/kite approve --payee 0xabc --amount 100 --token").unwrap_err();
        assert!(matches!(err, KiteError::BadInput(_)));
    }

    #[test]
    fn tokenize_respects_quotes() {
        let tokens = tokenize_command("/kite call --body '{\"city\":\"New York\"}'").unwrap();
        assert_eq!(
            tokens,
            vec![
                "/kite".to_string(),
                "call".to_string(),
                "--body".to_string(),
                "{\"city\":\"New York\"}".to_string()
            ]
        );
    }

    #[test]
    fn mask_mcp_url_hides_secret_segments() {
        let masked = mask_mcp_url("https://mcp.prod.gokite.ai/api_key_super_secret_value/mcp");
        assert!(masked.starts_with("https://mcp.prod.gokite.ai/"));
        assert!(!masked.contains("super_secret_value"));
    }

    #[test]
    fn load_setup_status_reports_missing_cli_and_url() {
        let db = open_in_memory_db();
        let conn = db.0.lock().unwrap();
        let status = load_setup_status(&conn, None);
        assert!(!status.cli_installed);
        assert!(!status.mcp_url_configured);
        assert_eq!(status.signup_email, None);
        assert_eq!(status.pending_signup_id, None);
        assert_eq!(status.auth_state, KiteAuthState::CliMissing);
    }

    #[test]
    fn load_setup_status_reports_configured_url_and_payer() {
        let db = open_in_memory_db();
        {
            let conn = db.0.lock().unwrap();
            crate::database::set_config(
                &conn,
                KITE_MCP_URL_KEY,
                "https://neo.dev.gokite.ai/v1/mcp",
            )
            .unwrap();
            crate::database::set_config(&conn, KITE_AUTH_STATE_KEY, "ready").unwrap();
            crate::database::set_config(&conn, KITE_LAST_PAYER_KEY, "0x123").unwrap();
            crate::database::set_config(&conn, KITE_SIGNUP_EMAIL_KEY, "and00sama@gmail.com")
                .unwrap();
            crate::database::set_config(&conn, KITE_PENDING_SIGNUP_ID_KEY, "signup_123").unwrap();
        }
        let conn = db.0.lock().unwrap();
        let status = load_setup_status(
            &conn,
            Some(PathBuf::from("C:\\Users\\IAM\\.kpass\\bin\\kpass.exe")),
        );
        assert!(status.cli_installed);
        assert!(status.mcp_url_configured);
        assert!(status.connected);
        assert_eq!(status.last_payer_addr.as_deref(), Some("0x123"));
        assert_eq!(status.signup_email.as_deref(), Some("and00sama@gmail.com"));
        assert_eq!(status.pending_signup_id.as_deref(), Some("signup_123"));
    }

    #[test]
    fn signup_validation_helpers_reject_bad_inputs() {
        assert!(validate_signup_email("not-an-email").is_err());
        assert!(validate_signup_code("short").is_err());
        assert_eq!(
            validate_signup_email("  and00sama@gmail.com ").unwrap(),
            "and00sama@gmail.com"
        );
        assert_eq!(validate_signup_code("A1B2C3D4").unwrap(), "A1B2C3D4");
    }

    #[test]
    fn payment_error_mapping_covers_documented_states() {
        assert!(matches!(
            map_kite_failure("session_creation_required"),
            KiteError::SessionCreationRequired(_)
        ));
        assert!(matches!(
            map_kite_failure("SessionExpired"),
            KiteError::SessionExpired(_)
        ));
        assert!(matches!(
            map_kite_failure("InsufficientBudget"),
            KiteError::InsufficientBudget(_)
        ));
        assert!(matches!(
            map_kite_failure("Unauthorized"),
            KiteError::Unauthorized(_)
        ));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn parse_bundle_version_accepts_integer_and_string_payloads() {
        assert_eq!(parse_bundle_version_from_body("26").unwrap(), "26");
        assert_eq!(parse_bundle_version_from_body("\"27\"").unwrap(), "27");
    }

    #[tokio::test]
    async fn blocking_kite_tasks_can_drop_blocking_clients_inside_async_commands() {
        let marker = run_blocking_kite_task("test installer regression", || {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_millis(50))
                .build()
                .map_err(|err| KiteError::Unknown(err.to_string()))?;
            drop(client);
            Ok("ok".to_string())
        })
        .await
        .unwrap();

        assert_eq!(marker, "ok");
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn installer_command_targets_official_kite_installer() {
        let (program, args) = kite_installer_command();
        assert_eq!(program, "sh");
        assert!(args.iter().any(|arg| arg.contains(KITE_INSTALLER_URL)));
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn native_windows_installer_downloads_and_extracts_kite_bundle() {
        let cli_zip = build_zip(&[("nested/kpass.exe", b"kpass-binary")]);
        let ksearch_zip = build_zip(&[("nested/ksearch.exe", b"ksearch-binary")]);
        let skills_tgz = build_tar_gz(&[("skills/weather/skill.txt", b"skill body")]);
        let cli_checksum = sha256_hex(&cli_zip);
        let ksearch_checksum = sha256_hex(&ksearch_zip);
        let skills_checksum = sha256_hex(&skills_tgz);
        let manifest = serde_json::json!({
            "cli": {
                "version": "1.3.16",
                "platforms": {
                    "windows-amd64": {
                        "archive": "cli.zip",
                        "checksum": format!("sha256:{cli_checksum}")
                    }
                }
            },
            "ksearch": {
                "version": "1.0.1",
                "platforms": {
                    "windows-amd64": {
                        "archive": "ksearch.zip",
                        "checksum": format!("sha256:{ksearch_checksum}")
                    }
                }
            },
            "skills": {
                "version": "1.1.3",
                "archive": "skills.tar.gz",
                "checksum": format!("sha256:{skills_checksum}")
            }
        });

        let mut server = mockito::Server::new();
        server
            .mock("GET", "/latest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("26")
            .create();
        server
            .mock("GET", "/bundle/26/manifest.json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(manifest.to_string())
            .create();
        server
            .mock("GET", "/bundle/26/cli.zip")
            .with_status(200)
            .with_body(cli_zip)
            .create();
        server
            .mock("GET", "/bundle/26/ksearch.zip")
            .with_status(200)
            .with_body(ksearch_zip)
            .create();
        server
            .mock("GET", "/bundle/26/skills.tar.gz")
            .with_status(200)
            .with_body(skills_tgz)
            .create();

        let install_dir = temp_test_dir("kite-install-success");
        let client = reqwest::blocking::Client::new();
        let installed =
            install_kite_cli_windows_with_client(&client, &server.url(), &install_dir).unwrap();

        assert!(installed.cli_path.exists());
        assert!(install_dir.join("bin").join("kpass.exe").exists());
        assert!(install_dir.join("bin").join("ksearch.exe").exists());
        assert!(install_dir
            .join("skills")
            .join("skills")
            .join("weather")
            .join("skill.txt")
            .exists());

        let version_json = fs::read_to_string(install_dir.join("version.json")).unwrap();
        assert!(version_json.contains("\"bundle_version\": \"26\""));
        assert!(version_json.contains("\"cli_version\": \"1.3.16\""));

        let _ = fs::remove_dir_all(install_dir);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn native_windows_installer_reports_checksum_mismatch_clearly() {
        let cli_zip = build_zip(&[("nested/kpass.exe", b"kpass-binary")]);
        let skills_tgz = build_tar_gz(&[("skills/weather/skill.txt", b"skill body")]);
        let skills_checksum = sha256_hex(&skills_tgz);
        let manifest = serde_json::json!({
            "cli": {
                "version": "1.3.16",
                "platforms": {
                    "windows-amd64": {
                        "archive": "cli.zip",
                        "checksum": "sha256:deadbeef"
                    }
                }
            },
            "skills": {
                "version": "1.1.3",
                "archive": "skills.tar.gz",
                "checksum": format!("sha256:{skills_checksum}")
            }
        });

        let mut server = mockito::Server::new();
        server
            .mock("GET", "/latest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("\"26\"")
            .create();
        server
            .mock("GET", "/bundle/26/manifest.json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(manifest.to_string())
            .create();
        server
            .mock("GET", "/bundle/26/cli.zip")
            .with_status(200)
            .with_body(cli_zip)
            .create();

        let install_dir = temp_test_dir("kite-install-bad-checksum");
        let client = reqwest::blocking::Client::new();
        let err =
            install_kite_cli_windows_with_client(&client, &server.url(), &install_dir).unwrap_err();
        assert!(err.message().contains("checksum verification"));
        let _ = fs::remove_dir_all(install_dir);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn native_windows_installer_reports_missing_kpass_binary() {
        let cli_zip = build_zip(&[("nested/not-kpass.exe", b"missing")]);
        let skills_tgz = build_tar_gz(&[("skills/weather/skill.txt", b"skill body")]);
        let cli_checksum = sha256_hex(&cli_zip);
        let skills_checksum = sha256_hex(&skills_tgz);
        let manifest = serde_json::json!({
            "cli": {
                "version": "1.3.16",
                "platforms": {
                    "windows-amd64": {
                        "archive": "cli.zip",
                        "checksum": format!("sha256:{cli_checksum}")
                    }
                }
            },
            "skills": {
                "version": "1.1.3",
                "archive": "skills.tar.gz",
                "checksum": format!("sha256:{skills_checksum}")
            }
        });

        let mut server = mockito::Server::new();
        server
            .mock("GET", "/latest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("26")
            .create();
        server
            .mock("GET", "/bundle/26/manifest.json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(manifest.to_string())
            .create();
        server
            .mock("GET", "/bundle/26/cli.zip")
            .with_status(200)
            .with_body(cli_zip)
            .create();
        server
            .mock("GET", "/bundle/26/skills.tar.gz")
            .with_status(200)
            .with_body(skills_tgz)
            .create();

        let install_dir = temp_test_dir("kite-install-missing-kpass");
        let client = reqwest::blocking::Client::new();
        let err =
            install_kite_cli_windows_with_client(&client, &server.url(), &install_dir).unwrap_err();
        assert!(err.message().contains("final binary detection"));
        let _ = fs::remove_dir_all(install_dir);
    }

    #[tokio::test]
    async fn verify_connection_lists_tools_and_marks_ready() {
        let db = open_in_memory_db();
        {
            let conn = db.0.lock().unwrap();
            persist_mcp_url(&conn, "http://127.0.0.1:18080/mcp").unwrap();
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:18080")
            .await
            .unwrap();
        tokio::spawn(async move {
            for request_idx in 0..3 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0_u8; 4096];
                let bytes = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
                    .await
                    .unwrap();
                let request = String::from_utf8_lossy(&buf[..bytes]).to_string();
                let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
                let response = if request_idx == 0 {
                    assert!(body.contains("\"method\":\"initialize\""));
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: test-session\r\nContent-Length: 142\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2025-03-26\",\"capabilities\":{\"tools\":{\"listChanged\":true}},\"serverInfo\":{\"name\":\"kite\",\"version\":\"1.0.0\"}}}".to_string()
                } else if request_idx == 1 {
                    assert!(body.contains("notifications/initialized"));
                    "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n".to_string()
                } else {
                    assert!(request.contains("Mcp-Session-Id: test-session"));
                    assert!(body.contains("\"method\":\"tools/list\""));
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 160\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":3,\"result\":{\"tools\":[{\"name\":\"get_payer_addr\"},{\"name\":\"approve_payment\"}]}}".to_string()
                };
                tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                    .await
                    .unwrap();
            }
        });

        let client = Client::new();
        let response = verify_kite_connection_inner(&client, &db).await.unwrap();
        assert!(response.connected);
        assert_eq!(response.auth_state, KiteAuthState::Ready);
        assert_eq!(
            response.available_tools,
            vec!["get_payer_addr", "approve_payment"]
        );
    }

    #[tokio::test]
    async fn fetch_payer_address_reads_structured_content() {
        let db = open_in_memory_db();
        {
            let conn = db.0.lock().unwrap();
            persist_mcp_url(&conn, "http://127.0.0.1:18081/mcp").unwrap();
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:18081")
            .await
            .unwrap();
        tokio::spawn(async move {
            for request_idx in 0..4 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0_u8; 4096];
                let bytes = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
                    .await
                    .unwrap();
                let request = String::from_utf8_lossy(&buf[..bytes]).to_string();
                let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
                let response = match request_idx {
                    0 => "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: payer-session\r\nContent-Length: 142\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2025-03-26\",\"capabilities\":{\"tools\":{\"listChanged\":true}},\"serverInfo\":{\"name\":\"kite\",\"version\":\"1.0.0\"}}}".to_string(),
                    1 => "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n".to_string(),
                    2 => {
                        assert!(body.contains("\"method\":\"tools/call\""));
                        assert!(body.contains("get_payer_addr"));
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 148\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"structuredContent\":{\"payer_addr\":\"0x742d35Cc\"},\"content\":[{\"type\":\"text\",\"text\":\"payer ok\"}],\"isError\":false}}".to_string()
                    }
                    _ => panic!("unexpected request: {request}"),
                };
                tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                    .await
                    .unwrap();
            }
        });

        let payer = fetch_payer_address(&Client::new(), &db).await.unwrap();
        assert_eq!(payer, "0x742d35Cc");
    }

    #[tokio::test]
    async fn approve_payment_reads_x_payment() {
        let db = open_in_memory_db();
        {
            let conn = db.0.lock().unwrap();
            persist_mcp_url(&conn, "http://127.0.0.1:18082/mcp").unwrap();
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:18082")
            .await
            .unwrap();
        tokio::spawn(async move {
            for request_idx in 0..5 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0_u8; 4096];
                let bytes = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
                    .await
                    .unwrap();
                let request = String::from_utf8_lossy(&buf[..bytes]).to_string();
                let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
                let response = match request_idx {
                    0 => "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: approve-session\r\nContent-Length: 142\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2025-03-26\",\"capabilities\":{\"tools\":{\"listChanged\":true}},\"serverInfo\":{\"name\":\"kite\",\"version\":\"1.0.0\"}}}".to_string(),
                    1 => "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n".to_string(),
                    2 => "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 157\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"structuredContent\":{\"payer_addr\":\"0x742d35Cc\"},\"content\":[{\"type\":\"text\",\"text\":\"payer ok\"}],\"isError\":false}}".to_string(),
                    3 => {
                        assert!(body.contains("approve_payment"));
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 180\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"structuredContent\":{\"x_payment\":\"signed-header\"},\"content\":[{\"type\":\"text\",\"text\":\"signed-header\"}],\"isError\":false}}".to_string()
                    }
                    _ => panic!("unexpected request: {request}"),
                };
                tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                    .await
                    .unwrap();
            }
        });

        let payer = fetch_payer_address(&Client::new(), &db).await.unwrap();
        let auth = approve_payment(
            &Client::new(),
            &db,
            &payer,
            &PaymentRequest {
                payee_addr: "0x209693Bc".to_string(),
                amount: "100".to_string(),
                token_type: "USDC".to_string(),
                merchant_name: Some("Weather".to_string()),
            },
        )
        .await
        .unwrap();
        assert_eq!(auth.x_payment, "signed-header");
    }

    #[tokio::test]
    async fn call_x402_service_retries_after_payment_required() {
        let db = open_in_memory_db();
        let runtime = Arc::new(KiteRuntimeState::new());
        {
            let conn = db.0.lock().unwrap();
            persist_mcp_url(&conn, "http://127.0.0.1:18083/mcp").unwrap();
        }

        let mcp_listener = tokio::net::TcpListener::bind("127.0.0.1:18083")
            .await
            .unwrap();
        tokio::spawn(async move {
            for request_idx in 0..5 {
                let (mut stream, _) = mcp_listener.accept().await.unwrap();
                let mut buf = [0_u8; 4096];
                let bytes = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
                    .await
                    .unwrap();
                let request = String::from_utf8_lossy(&buf[..bytes]).to_string();
                let body = request.split("\r\n\r\n").nth(1).unwrap_or_default();
                let response = match request_idx {
                    0 => "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: x402-session\r\nContent-Length: 159\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2025-03-26\",\"capabilities\":{\"tools\":{\"listChanged\":true}},\"serverInfo\":{\"name\":\"kite\",\"version\":\"1.0.0\"}}}".to_string(),
                    1 => "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n".to_string(),
                    2 => "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 145\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"structuredContent\":{\"payer_addr\":\"0x742d35Cc\"},\"content\":[{\"type\":\"text\",\"text\":\"payer ok\"}],\"isError\":false}}".to_string(),
                    3 => {
                        assert!(body.contains("approve_payment"));
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 152\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"structuredContent\":{\"x_payment\":\"signed-header\"},\"content\":[{\"type\":\"text\",\"text\":\"signed-header\"}],\"isError\":false}}".to_string()
                    }
                    _ => panic!("unexpected request: {request}"),
                };
                tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                    .await
                    .unwrap();
            }
        });

        let service_listener = tokio::net::TcpListener::bind("127.0.0.1:18084")
            .await
            .unwrap();
        tokio::spawn(async move {
            for request_idx in 0..2 {
                let (mut stream, _) = service_listener.accept().await.unwrap();
                let mut buf = [0_u8; 4096];
                let bytes = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
                    .await
                    .unwrap();
                let request = String::from_utf8_lossy(&buf[..bytes]).to_string();
                let response = if request_idx == 0 {
                    "HTTP/1.1 402 Payment Required\r\nContent-Type: application/json\r\nContent-Length: 62\r\n\r\n{\"payee_addr\":\"0x209693Bc\",\"amount\":\"100\",\"token_type\":\"USDC\"}".to_string()
                } else {
                    assert!(request
                        .to_ascii_lowercase()
                        .contains("x-payment: signed-header"));
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 19\r\n\r\n{\"weather\":\"sunny\"}".to_string()
                };
                tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                    .await
                    .unwrap();
            }
        });

        let mut events = Vec::new();
        let runtime_for_confirm = runtime.clone();
        let result = call_x402_service(
            &Client::new(),
            &db,
            &runtime,
            "http://127.0.0.1:18084/weather",
            "POST",
            Some("{\"city\":\"Riyadh\"}"),
            Some("Weather".to_string()),
            &mut |event| {
                if let KiteEvent::AwaitingPaymentConfirmation { action_id, .. } = &event {
                    let _ = resolve_pending_payment(&runtime_for_confirm, action_id, true);
                }
                events.push(event);
            },
        )
        .await
        .unwrap();
        assert_eq!(result.status, 200);
        assert!(result.body.contains("sunny"));
        assert_eq!(result.payment.payee_addr, "0x209693Bc");
        assert!(events
            .iter()
            .any(|event| matches!(event, KiteEvent::RetryingPaidRequest)));
        assert!(events
            .iter()
            .any(|event| matches!(event, KiteEvent::AwaitingPaymentConfirmation { .. })));
    }

    #[tokio::test]
    async fn kite_agent_capability_prefers_cloud_provider_for_autopilot() {
        let chat_provider = crate::providers::SharedChatProvider::new();
        let db = open_in_memory_db();
        let agent_state = Arc::new(crate::agent::AgentState::new());
        agent_state.set_provider_config(crate::providers::ProviderConfig {
            provider: crate::providers::Provider::OpenAI,
            model: "gpt-4o".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "sk-test".to_string(),
        });
        let active_model = crate::models::ActiveModelState::new();
        let client = Client::new();
        let config = crate::config::AppConfig::default();

        let capability = kite_agent_capability_inner(
            &client,
            &chat_provider,
            &db,
            &agent_state,
            &active_model,
            &config,
        )
        .await
        .unwrap();

        assert!(capability.available);
        assert_eq!(capability.mode, "agentic");
        assert_eq!(capability.provider, "OpenAI");
    }

    #[tokio::test]
    async fn kite_agent_capability_hydrates_openrouter_from_persisted_settings() {
        let chat_provider = crate::providers::SharedChatProvider::new();
        let db = open_in_memory_db();
        {
            let conn = db.0.lock().unwrap();
            crate::database::set_config(&conn, "api_key_openrouter", "sk-or-v1-test").unwrap();
        }
        let agent_state = Arc::new(crate::agent::AgentState::new());
        let active_model = crate::models::ActiveModelState::new();
        let client = Client::new();
        let mut config = crate::config::AppConfig::default();
        config.agent.provider = "openrouter".to_string();
        config.agent.model = "google/gemini-2.5-pro".to_string();
        config.agent.base_url = "https://openrouter.ai/api/v1".to_string();

        let capability = kite_agent_capability_inner(
            &client,
            &chat_provider,
            &db,
            &agent_state,
            &active_model,
            &config,
        )
        .await
        .unwrap();

        assert!(capability.available);
        assert_eq!(capability.mode, "agentic");
        assert_eq!(capability.provider, "OpenRouter");
        assert_eq!(capability.model.as_deref(), Some("google/gemini-2.5-pro"));

        let hydrated = agent_state.get_provider_config().unwrap();
        assert!(matches!(
            hydrated.provider,
            crate::providers::Provider::OpenRouter
        ));
        assert_eq!(hydrated.model, "google/gemini-2.5-pro");

        let shared = chat_provider.0.lock().unwrap().clone().unwrap();
        assert!(matches!(
            shared.provider,
            crate::providers::Provider::OpenRouter
        ));
    }

    #[tokio::test]
    async fn kite_agent_capability_prefers_saved_openrouter_mode_over_local_config() {
        let chat_provider = crate::providers::SharedChatProvider::new();
        let db = open_in_memory_db();
        {
            let conn = db.0.lock().unwrap();
            crate::database::set_config(&conn, "provider_mode", "openrouter").unwrap();
            crate::database::set_config(&conn, "api_key_openrouter", "sk-or-v1-test").unwrap();
            crate::database::set_config(&conn, "openrouter_model", "google/gemini-2.5-pro")
                .unwrap();
        }
        let agent_state = Arc::new(crate::agent::AgentState::new());
        let active_model = crate::models::ActiveModelState::new();
        let client = Client::new();
        let config = crate::config::AppConfig::default();

        let capability = kite_agent_capability_inner(
            &client,
            &chat_provider,
            &db,
            &agent_state,
            &active_model,
            &config,
        )
        .await
        .unwrap();

        assert!(capability.available);
        assert_eq!(capability.provider, "OpenRouter");
        assert_eq!(capability.model.as_deref(), Some("google/gemini-2.5-pro"));
    }

    #[test]
    fn resolve_kite_agent_launch_model_prefers_cloud_provider_without_touching_local_model() {
        let active_model = crate::models::ActiveModelState::new();
        let provider_config = crate::providers::ProviderConfig {
            provider: crate::providers::Provider::OpenRouter,
            model: "google/gemini-2.5-flash".to_string(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            api_key: "sk-or-v1-test".to_string(),
        };

        let launch_model =
            resolve_kite_agent_launch_model(Some(&provider_config), &active_model).unwrap();

        assert_eq!(launch_model, "google/gemini-2.5-flash");
    }

    #[test]
    fn resolve_kite_agent_launch_model_requires_local_model_for_ollama() {
        let active_model = crate::models::ActiveModelState::new();
        let provider_config = crate::providers::ProviderConfig {
            provider: crate::providers::Provider::Ollama,
            model: "llama3.2".to_string(),
            base_url: "http://127.0.0.1:11434".to_string(),
            api_key: String::new(),
        };

        let error =
            resolve_kite_agent_launch_model(Some(&provider_config), &active_model).unwrap_err();

        assert!(matches!(error, KiteError::BadInput(_)));
    }

    #[test]
    fn merge_windows_path_appends_missing_entry_once() {
        let merged =
            merge_windows_path(r"C:\Windows\System32;C:\Tools", r"C:\Users\IAM\.kpass\bin")
                .unwrap();

        assert_eq!(
            merged,
            r"C:\Windows\System32;C:\Tools;C:\Users\IAM\.kpass\bin"
        );
        assert_eq!(
            merge_windows_path(&merged, r"C:\Users\IAM\.kpass\bin"),
            None
        );
    }

    #[tokio::test]
    async fn kite_agent_capability_reports_guided_help_for_text_only_local_model() {
        let chat_provider = crate::providers::SharedChatProvider::new();
        let db = open_in_memory_db();
        let agent_state = Arc::new(crate::agent::AgentState::new());
        let active_model = crate::models::ActiveModelState::new();
        *active_model.0.lock().unwrap() = Some("text-only".to_string());

        let mut server = mockito::Server::new_async().await;
        let _show = server
            .mock("POST", "/api/show")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"capabilities":["completion"]}"#)
            .create_async()
            .await;

        let client = Client::new();
        let mut config = crate::config::AppConfig::default();
        config.inference.ollama_url = server.url();

        let capability = kite_agent_capability_inner(
            &client,
            &chat_provider,
            &db,
            &agent_state,
            &active_model,
            &config,
        )
        .await
        .unwrap();

        assert!(!capability.available);
        assert_eq!(capability.mode, "advisory_fallback");
        assert!(capability
            .reason
            .contains("does not advertise vision support"));
    }

    #[tokio::test]
    async fn kite_agent_capability_reports_guided_help_when_no_local_model_is_selected() {
        let chat_provider = crate::providers::SharedChatProvider::new();
        let db = open_in_memory_db();
        let agent_state = Arc::new(crate::agent::AgentState::new());
        let active_model = crate::models::ActiveModelState::new();

        let mut server = mockito::Server::new_async().await;
        let _tags = server
            .mock("GET", "/api/tags")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"models":[{"name":"llama3.2-vision"},{"name":"gemma4:e2b"}]}"#)
            .create_async()
            .await;

        let client = Client::new();
        let mut config = crate::config::AppConfig::default();
        config.inference.ollama_url = server.url();

        let capability = kite_agent_capability_inner(
            &client,
            &chat_provider,
            &db,
            &agent_state,
            &active_model,
            &config,
        )
        .await
        .unwrap();

        assert!(!capability.available);
        assert_eq!(capability.mode, "advisory_fallback");
        assert!(capability.reason.contains("no local model is selected"));
    }

    #[tokio::test]
    async fn kite_agent_capability_reports_ollama_unreachable_as_guided_help_only() {
        let chat_provider = crate::providers::SharedChatProvider::new();
        let db = open_in_memory_db();
        let agent_state = Arc::new(crate::agent::AgentState::new());
        let active_model = crate::models::ActiveModelState::new();
        let client = Client::new();
        let mut config = crate::config::AppConfig::default();
        config.inference.ollama_url = "http://127.0.0.1:1".to_string();

        let capability = kite_agent_capability_inner(
            &client,
            &chat_provider,
            &db,
            &agent_state,
            &active_model,
            &config,
        )
        .await
        .unwrap();

        assert!(!capability.available);
        assert_eq!(capability.mode, "advisory_fallback");
        assert!(capability.reason.contains("Ollama is unreachable"));
    }

    #[tokio::test]
    async fn unauthorized_tool_error_maps_to_auth_state() {
        let db = open_in_memory_db();
        {
            let conn = db.0.lock().unwrap();
            persist_mcp_url(&conn, "http://127.0.0.1:18085/mcp").unwrap();
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:18085")
            .await
            .unwrap();
        tokio::spawn(async move {
            for request_idx in 0..4 {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buf = [0_u8; 4096];
                let _bytes = tokio::io::AsyncReadExt::read(&mut stream, &mut buf)
                    .await
                    .unwrap();
                let response = match request_idx {
                    0 => "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nMcp-Session-Id: auth-session\r\nContent-Length: 142\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"protocolVersion\":\"2025-03-26\",\"capabilities\":{\"tools\":{\"listChanged\":true}},\"serverInfo\":{\"name\":\"kite\",\"version\":\"1.0.0\"}}}".to_string(),
                    1 => "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n".to_string(),
                    2 => "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 151\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":2,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"Unauthorized\"}],\"isError\":true}}".to_string(),
                    _ => panic!("unexpected request"),
                };
                tokio::io::AsyncWriteExt::write_all(&mut stream, response.as_bytes())
                    .await
                    .unwrap();
            }
        });

        let err = fetch_payer_address(&Client::new(), &db).await.unwrap_err();
        assert!(matches!(err, KiteError::Unauthorized(_)));
    }
}
