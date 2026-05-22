/**
 * Agent tab - provider, model, base URL, and API key for agent mode.
 *
 * OpenRouter uses a dedicated "Connect" registration flow:
 * the user enters their key and clicks Connect, which validates it
 * against the OpenRouter API. On success the provider/model/base_url
 * are all switched to OpenRouter automatically. All other providers
 * keep the original form-based UX.
 */

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

import { Section, TextField, Dropdown } from '../components';
import { SaveField } from '../components/SaveField';
import { configHelp } from '../configHelpers';
import type { RawAppConfig } from '../types';

type AgentProvider = 'ollama' | 'openai' | 'anthropic';

// OpenRouter is intentionally excluded from the generic dropdown —
// it has its own dedicated registration card below.
const PROVIDERS: AgentProvider[] = ['ollama', 'openai', 'anthropic'];
const PROVIDER_LABELS: Record<AgentProvider, string> = {
  ollama: 'Ollama (Local)',
  openai: 'OpenAI',
  anthropic: 'Anthropic',
};

const PROVIDER_MODEL_SUGGESTIONS: Record<AgentProvider, readonly string[]> = {
  ollama: ['llama3.2-vision', 'llama3.2', 'mistral', 'gemma3', 'qwen2.5-vl'],
  openai: ['gpt-4o', 'gpt-4o-mini', 'gpt-4-turbo', 'o3-mini', 'o1'],
  anthropic: [
    'claude-sonnet-4-20250514',
    'claude-3-5-sonnet-20241022',
    'claude-3-5-haiku-20241022',
    'claude-3-haiku-20240307',
  ],
};

const OPENROUTER_DEFAULT_MODEL = 'google/gemini-2.5-flash';
const OPENROUTER_BASE_URL = 'https://openrouter.ai/api/v1';

const OPENROUTER_MODELS = [
  'openai/gpt-4o',
  'openai/gpt-4o-mini',
  'openai/o3-mini',
  'anthropic/claude-sonnet-4',
  'anthropic/claude-3-5-haiku',
  'google/gemini-2.5-pro',
  'google/gemini-2.5-flash',
  'meta-llama/llama-4-scout',
  'meta-llama/llama-4-maverick',
  'mistralai/mistral-large',
  'deepseek/deepseek-r1',
  'x-ai/grok-3',
  'google/gemma-4-31b-it:free',
  'google/gemma-4-26b-a4b-it:free',
  'inclusionai/ring-2.6-1t:free',
  'arcee-ai/trinity-large-thinking:free',
  'baidu/cobuddy:free',
  'poolside/laguna-xs.2:free',
  'minimax/minimax-m2.5:free',
  'liquid/lfm-2.5-1.2b-thinking:free',
  'openai/gpt-oss-120b:free',
  'qwen/qwen3-coder:free',
] as const;

interface AgentTabProps {
  config: RawAppConfig;
  resyncToken: number;
  onSaved: (next: RawAppConfig) => void;
}

interface KiteSetupStatus {
  cliInstalled: boolean;
  cliPath: string | null;
  mcpUrlConfigured: boolean;
  maskedMcpUrl: string | null;
  authState: string;
  connected: boolean;
  lastPayerAddr: string | null;
  signupEmail: string | null;
  pendingSignupId: string | null;
  inviteOnly: boolean;
  docsUrl: string;
  portalUrl: string;
  installerUrl: string;
}

interface KiteVerifyResponse {
  connected: boolean;
  authState: string;
  availableTools: string[];
  message: string;
}

interface KiteAgentCapability {
  available: boolean;
  mode: string;
  provider: string;
  model: string | null;
  reason: string;
}

interface KiteAccountSummary {
  loggedIn: boolean;
  email: string | null;
  userId: string | null;
  signupEmail: string | null;
  pendingSignupId: string | null;
  pendingLoginId: string | null;
  currentAgentIdentity: string | null;
  authState: string;
}

interface KiteWalletAsset {
  symbol: string;
  balance: string;
  native: boolean;
}

interface KiteWalletSummary {
  walletAddress: string;
  walletType: string;
  chainId: number;
  assets: KiteWalletAsset[];
  canUseFaucet: boolean;
}

interface KiteSessionSummary {
  id: string;
  status: string;
  agentType: string;
  expiresAt: string | null;
  taskSummary: string | null;
  assets: string[];
  maxAmountPerTx: string | null;
  maxTotalAmount: string | null;
  spentTotal: string | null;
  reservedTotal: string | null;
  selected: boolean;
}

interface KiteActivityEvent {
  id: string;
  kind: string;
  status: string;
  title: string;
  occurredAt: string;
  amountDisplay: string | null;
  chainName: string | null;
  txHash: string | null;
  orderId: string | null;
  itemTitles: string[];
  errorMessage: string | null;
}

interface KiteShopItem {
  provider: string;
  externalIdentifier: string;
  title: string;
  price: string;
  rating: string | null;
  reviews: string | null;
  link: string | null;
  thumbnail: string | null;
}

interface KiteCartItem {
  provider: string;
  externalIdentifier: string;
  productLocator: string;
  title: string;
  price: string;
  quantity: number;
}

interface KiteShippingSummary {
  name: string | null;
  email: string | null;
  line1: string | null;
  line2: string | null;
  city: string | null;
  state: string | null;
  postalCode: string | null;
  country: string | null;
  complete: boolean;
  missing: string[];
}

interface KiteCartSummary {
  items: KiteCartItem[];
  itemCount: number;
  paymentCurrency: string | null;
  paymentChain: string | null;
  shipping: KiteShippingSummary | null;
}

interface KiteOrderSummary {
  orderId: string;
  phase: string | null;
  paymentStatus: string | null;
  txHash: string | null;
  currency: string | null;
  chain: string | null;
  deliveryStatus: string | null;
  title: string | null;
}

interface KiteDeveloperSummary {
  authState: string;
  connected: boolean;
  maskedMcpUrl: string | null;
  lastPayerAddr: string | null;
  currentSessionId: string | null;
}

interface KiteHubState {
  setup: KiteSetupStatus;
  account: KiteAccountSummary;
  wallet: KiteWalletSummary | null;
  sessions: KiteSessionSummary[];
  activity: KiteActivityEvent[];
  cart: KiteCartSummary | null;
  orders: KiteOrderSummary[];
  developer: KiteDeveloperSummary;
  issues: string[];
}

function describeKiteAuthState(state: string): string {
  switch (state) {
    case 'ready':
      return 'Connected';
    case 'unverified':
      return 'Saved but not verified';
    case 'missing_mcp_url':
      return 'MCP URL required';
    case 'cli_missing':
      return 'Kite CLI not found';
    case 'auth_required':
      return 'Auth required';
    case 'session_creation_required':
      return 'Session creation required';
    case 'session_expired':
      return 'Session expired';
    case 'insufficient_budget':
      return 'Insufficient budget';
    case 'unauthorized':
      return 'Unauthorized';
    case 'agent_not_found':
      return 'Agent not found';
    case 'invalid_payment_response':
      return 'Invalid payment response';
    case 'network_error':
      return 'Network error';
    default:
      return 'Unknown';
  }
}

export function AgentTab({ config, resyncToken, onSaved }: AgentTabProps) {
  const [apiKey, setApiKey] = useState('');

  // OpenRouter registration state
  const [orKey, setOrKey] = useState('');
  const [orConnecting, setOrConnecting] = useState(false);
  const [orError, setOrError] = useState<string | null>(null);
  // Pre-seed label so connected state shows immediately (avoids registration form flash while SQLite loads).
  const [orLabel, setOrLabel] = useState<string | null>(
    config.agent.provider === 'openrouter' ? 'OpenRouter' : null,
  );
  const [orModel, setOrModel] = useState(OPENROUTER_DEFAULT_MODEL);
  const [kiteMcpUrl, setKiteMcpUrl] = useState('');
  const [kiteSignupEmail, setKiteSignupEmail] = useState('');
  const [kiteStatus, setKiteStatus] = useState<KiteSetupStatus | null>(null);
  const [kiteCapability, setKiteCapability] =
    useState<KiteAgentCapability | null>(null);
  const [kiteHub, setKiteHub] = useState<KiteHubState | null>(null);
  const [kiteSendTo, setKiteSendTo] = useState('');
  const [kiteSendAmount, setKiteSendAmount] = useState('');
  const [kiteSendAsset, setKiteSendAsset] = useState('USDC');
  const [kiteFaucetToken] = useState('USDC');
  const [kiteShopQuery, setKiteShopQuery] = useState('');
  const [kiteShopResults, setKiteShopResults] = useState<KiteShopItem[]>([]);
  const [kiteMessage, setKiteMessage] = useState<string | null>(null);
  const [kiteLoading, setKiteLoading] = useState(false);

  const isOpenRouter = config.agent.provider === 'openrouter';
  const provider = isOpenRouter ? 'openai' : (config.agent.provider as AgentProvider);

  // Load API key and OpenRouter state from SQLite
  useEffect(() => {
    async function loadKeys() {
      try {
        const settings = await invoke<Record<string, string>>('get_settings');
        const prov = config.agent.provider;
        if (prov !== 'ollama' && prov !== 'openrouter' && settings[`api_key_${prov}`]) {
          setApiKey(settings[`api_key_${prov}`]);
        }
        if (settings['api_key_openrouter']) {
          setOrKey(settings['api_key_openrouter']);
          setOrLabel(settings['openrouter_label'] ?? 'OpenRouter');
        }
        if (settings['openrouter_model']) {
          setOrModel(settings['openrouter_model']);
        }
      } catch {
        // not set yet
      }
    }
    void loadKeys();
  }, [config.agent.provider]);

  async function refreshKiteHubState() {
    try {
      const [settings, status, hub, capability] = await Promise.all([
        invoke<Record<string, string>>('get_settings'),
        invoke<KiteSetupStatus>('get_kite_setup_status'),
        invoke<KiteHubState>('get_kite_hub_state'),
        invoke<KiteAgentCapability>('get_kite_agent_capability'),
      ]);
      setKiteMcpUrl(settings['kite_mcp_url'] ?? '');
      setKiteSignupEmail(settings['kite_signup_email'] ?? status.signupEmail ?? '');
      setKiteStatus(status);
      setKiteHub(hub);
      setKiteCapability(capability);
    } catch {
      setKiteStatus(null);
      setKiteHub(null);
      setKiteCapability(null);
    }
  }

  useEffect(() => {
    async function loadKiteState() {
      await refreshKiteHubState();
    }

    void loadKiteState();
  }, [resyncToken, config.agent.provider, config.agent.model]);

  async function saveApiKey(key: string) {
    try {
      if (provider !== 'ollama') {
        await invoke('set_setting', { key: `api_key_${provider}`, value: key });
        await invoke('set_agent_provider', {
          provider,
          model: config.agent.model,
          baseUrl: config.agent.base_url,
          apiKey: key,
        });
      }
    } catch {
      // ignore
    }
  }

  async function connectOpenRouter() {
    setOrConnecting(true);
    setOrError(null);
    try {
      const label = await invoke<string>('validate_openrouter_key', { apiKey: orKey });
      // Validation passed — persist and activate
      await invoke('set_setting', { key: 'api_key_openrouter', value: orKey });
      await invoke('set_setting', { key: 'openrouter_label', value: label });
      await invoke('set_setting', { key: 'openrouter_model', value: orModel });
      await invoke('set_setting', { key: 'provider_mode', value: 'openrouter' });
      // Grant screenshot consent — user chose an online provider and accepts cloud data sharing.
      await invoke('set_setting', { key: 'agent_screenshot_consent', value: 'true' });
      // Switch TOML config to openrouter
      await invoke('set_config_field', { section: 'agent', key: 'provider', value: 'openrouter' });
      await invoke('set_config_field', { section: 'agent', key: 'model', value: orModel });
      await invoke('set_config_field', { section: 'agent', key: 'base_url', value: OPENROUTER_BASE_URL });
      // Sync in-memory agent state
      await invoke('set_agent_provider', {
        provider: 'openrouter',
        model: orModel,
        baseUrl: OPENROUTER_BASE_URL,
        apiKey: orKey,
      });
      // Refresh parent config
      const next = await invoke<RawAppConfig>('get_config');
      onSaved(next);
      setOrLabel(label);
    } catch (e) {
      setOrError(String(e));
    } finally {
      setOrConnecting(false);
    }
  }

  async function disconnectOpenRouter() {
    try {
      // Clear SQLite persistence so disconnect survives app restart.
      await invoke('set_setting', { key: 'provider_mode', value: 'local' });
      // Switch back to Ollama
      await invoke('set_config_field', { section: 'agent', key: 'provider', value: 'ollama' });
      await invoke('set_config_field', { section: 'agent', key: 'model', value: 'llama3.2' });
      await invoke('set_config_field', { section: 'agent', key: 'base_url', value: 'http://127.0.0.1:11434' });
      await invoke('set_agent_provider', {
        provider: 'ollama',
        model: 'llama3.2',
        baseUrl: 'http://127.0.0.1:11434',
        apiKey: '',
      });
      const next = await invoke<RawAppConfig>('get_config');
      onSaved(next);
      setOrLabel(null);
    } catch {
      // ignore
    }
  }

  async function saveKiteMcpUrl() {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      await invoke('set_kite_mcp_url', { url: kiteMcpUrl });
      const status = await invoke<KiteSetupStatus>('get_kite_setup_status');
      setKiteStatus(status);
      setKiteMessage('Kite MCP URL saved. Verify the connection when you are ready.');
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function saveKiteSignupEmail() {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      await invoke('set_setting', {
        key: 'kite_signup_email',
        value: kiteSignupEmail.trim(),
      });
      const status = await invoke<KiteSetupStatus>('get_kite_setup_status');
      setKiteStatus(status);
      setKiteMessage(
        'Kite signup email saved. `/kite setup` can now start the Passport sign-up flow.',
      );
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function verifyKite() {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      const response = await invoke<KiteVerifyResponse>('verify_kite_connection');
      const status = await invoke<KiteSetupStatus>('get_kite_setup_status');
      setKiteStatus(status);
      const tools =
        response.availableTools.length > 0
          ? ` Tools: ${response.availableTools.join(', ')}`
          : '';
      setKiteMessage(`${response.message}${tools}`);
    } catch (error) {
      setKiteMessage(String(error));
      try {
        const status = await invoke<KiteSetupStatus>('get_kite_setup_status');
        setKiteStatus(status);
      } catch {
        // Ignore follow-up status failures.
      }
    } finally {
      setKiteLoading(false);
    }
  }

  async function disconnectKite() {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      await invoke('disconnect_kite');
      const status = await invoke<KiteSetupStatus>('get_kite_setup_status');
      setKiteMcpUrl('');
      setKiteStatus(status);
      setKiteMessage('Kite Passport has been disconnected from this device.');
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function openKiteTarget(target: 'portal' | 'installer' | 'docs') {
    try {
      await invoke('open_kite_setup_target', { target });
    } catch (error) {
      setKiteMessage(String(error));
    }
  }

  async function installKiteCli() {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      const cliPath = await invoke<string>('install_kite_cli');
      const status = await invoke<KiteSetupStatus>('get_kite_setup_status');
      setKiteStatus(status);
      setKiteMessage(
        `Kite CLI installed from Thikra at ${cliPath}. If Kite's hosted PowerShell bootstrap is broken, this native Windows installer path keeps setup moving.`,
      );
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function startKiteAiSetup() {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      const trimmedEmail = kiteSignupEmail.trim();
      if (trimmedEmail.length > 0) {
        await invoke('set_setting', {
          key: 'kite_signup_email',
          value: trimmedEmail,
        });
      }
      const message = await invoke<string>('start_kite_agent_mode', {
        input:
          trimmedEmail.length > 0
            ? `/kite setup --email ${trimmedEmail}`
            : '/kite setup',
      });
      setKiteMessage(message);
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function logoutKiteAccount() {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      const account = await invoke<KiteAccountSummary>('kite_logout');
      await refreshKiteHubState();
      setKiteMessage(
        account.loggedIn
          ? `Still logged in as ${account.email ?? 'unknown user'}.`
          : 'Kite Passport logged out on this device.',
      );
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function sendKiteWalletTransfer() {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      const transfer = await invoke<{
        amount: string;
        asset: string;
        recipientAddress: string;
        transactionHash: string;
      }>('kite_wallet_send', {
        to: kiteSendTo.trim(),
        amount: kiteSendAmount.trim(),
        asset: kiteSendAsset.trim(),
      });
      await refreshKiteHubState();
      setKiteMessage(
        `Sent ${transfer.amount} ${transfer.asset} to ${transfer.recipientAddress}. Tx: ${transfer.transactionHash}`,
      );
      setKiteSendTo('');
      setKiteSendAmount('');
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function requestKiteFaucet() {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      const drop = await invoke<{
        amount: string;
        asset: string;
        transactionHash: string;
      }>('kite_faucet_drop', {
        token: kiteFaucetToken.trim(),
      });
      await refreshKiteHubState();
      setKiteMessage(
        `Dropped ${drop.amount} ${drop.asset} to your testnet wallet. Tx: ${drop.transactionHash}`,
      );
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function selectKiteSession(sessionId: string) {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      await invoke('kite_use_session', { sessionId });
      await refreshKiteHubState();
      setKiteMessage(`Kite session ${sessionId} is now selected for Thikra.`);
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function searchKiteShop() {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      const results = await invoke<KiteShopItem[]>('kite_shop_search', {
        query: kiteShopQuery.trim(),
      });
      setKiteShopResults(results);
      setKiteMessage(
        results.length > 0
          ? `Found ${results.length} Kite shopping result(s).`
          : 'No Kite shopping results matched that query.',
      );
    } catch (error) {
      setKiteShopResults([]);
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function addKiteCartItem(item: KiteShopItem) {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      await invoke('kite_cart_add', {
        provider: item.provider,
        externalId: item.externalIdentifier,
        quantity: 1,
      });
      await refreshKiteHubState();
      setKiteMessage(`Added ${item.title} to your Kite cart.`);
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  async function removeKiteCartItem(item: KiteCartItem) {
    setKiteLoading(true);
    setKiteMessage(null);
    try {
      await invoke('kite_cart_remove', {
        provider: item.provider,
        externalId: item.externalIdentifier,
      });
      await refreshKiteHubState();
      setKiteMessage(`Removed ${item.title} from your Kite cart.`);
    } catch (error) {
      setKiteMessage(String(error));
    } finally {
      setKiteLoading(false);
    }
  }

  return (
    <>
      {/* ── OpenRouter registration card ── */}
      <Section heading="OpenRouter">
        {isOpenRouter && orLabel ? (
          // Connected state
          <div className="flex flex-col gap-3">
            <div
              className="flex items-center gap-2 rounded-lg px-3 py-2"
              style={{ background: 'rgba(80, 200, 120, 0.08)', border: '1px solid rgba(80, 200, 120, 0.2)' }}
            >
              <span style={{ color: '#50c878', fontSize: 15 }}>✓</span>
              <span className="text-xs font-medium" style={{ color: '#50c878' }}>
                Connected — {orLabel}
              </span>
            </div>
            <div className="flex flex-col gap-1">
              <label className="text-xs font-medium" style={{ color: 'var(--color-text-secondary)' }}>
                Model
              </label>
              <select
                value={orModel}
                onChange={async (e) => {
                  const m = e.target.value;
                  setOrModel(m);
                  await invoke('set_setting', { key: 'openrouter_model', value: m });
                  await invoke('set_config_field', { section: 'agent', key: 'model', value: m });
                  await invoke('set_agent_provider', {
                    provider: 'openrouter',
                    model: m,
                    baseUrl: OPENROUTER_BASE_URL,
                    apiKey: orKey,
                  });
                  const next = await invoke<RawAppConfig>('get_config');
                  onSaved(next);
                }}
                className="w-full bg-transparent border-b border-white/20 text-sm focus:outline-none focus:border-primary"
                style={{ color: 'var(--color-text-primary)', padding: '4px 0' }}
              >
                {OPENROUTER_MODELS.map((m) => (
                  <option key={m} value={m} style={{ background: '#1a1512' }}>
                    {m}
                  </option>
                ))}
              </select>
            </div>
            <button
              type="button"
              onClick={() => void disconnectOpenRouter()}
              className="text-xs self-start"
              style={{ color: 'var(--color-text-secondary)', background: 'none', border: 'none', cursor: 'pointer', padding: 0 }}
            >
              Disconnect
            </button>
          </div>
        ) : (
          // Registration form
          <div className="flex flex-col gap-3">
            <p className="text-xs" style={{ color: 'var(--color-text-secondary)', lineHeight: 1.5 }}>
              Connect OpenRouter to use any model (GPT-4o, Gemini, Claude, Llama…) with a single API key.
            </p>
            <div className="flex flex-col gap-1">
              <label className="text-xs font-medium" style={{ color: 'var(--color-text-secondary)' }}>
                API Key
              </label>
              <input
                type="password"
                value={orKey}
                onChange={(e) => { setOrKey(e.target.value); setOrError(null); }}
                placeholder="sk-or-v1-..."
                className="w-full bg-transparent border-b border-white/20 text-sm focus:outline-none focus:border-primary"
                style={{ color: 'var(--color-text-primary)', padding: '4px 0' }}
              />
            </div>
            <div className="flex flex-col gap-1">
              <label className="text-xs font-medium" style={{ color: 'var(--color-text-secondary)' }}>
                Model
              </label>
              <select
                value={orModel}
                onChange={(e) => setOrModel(e.target.value)}
                className="w-full bg-transparent border-b border-white/20 text-sm focus:outline-none focus:border-primary"
                style={{ color: 'var(--color-text-primary)', padding: '4px 0' }}
              >
                {OPENROUTER_MODELS.map((m) => (
                  <option key={m} value={m} style={{ background: '#1a1512' }}>
                    {m}
                  </option>
                ))}
              </select>
            </div>
            {orError ? (
              <p className="text-xs" style={{ color: '#ff8a80' }}>{orError}</p>
            ) : null}
            <button
              type="button"
              onClick={() => void connectOpenRouter()}
              disabled={orConnecting || orKey.trim().length === 0}
              className="self-start text-xs font-medium rounded-lg px-3 py-1.5 transition-opacity"
              style={{
                background: 'var(--color-primary)',
                color: '#fff',
                border: 'none',
                cursor: orConnecting || orKey.trim().length === 0 ? 'not-allowed' : 'pointer',
                opacity: orConnecting || orKey.trim().length === 0 ? 0.5 : 1,
              }}
            >
              {orConnecting ? 'Connecting…' : 'Connect'}
            </button>
            <p className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
              Get a key at{' '}
              <span style={{ color: 'var(--color-text-primary)' }}>openrouter.ai/keys</span>
            </p>
          </div>
        )}
      </Section>

      {/* ── Generic provider (Ollama / OpenAI / Anthropic) — hidden when OpenRouter is active ── */}
      {!isOpenRouter ? (
        <>
          <Section heading="Provider">
            <SaveField
              section="agent"
              fieldKey="provider"
              label="Provider"
              helper={configHelp('agent', 'provider')}
              initialValue={config.agent.provider}
              resyncToken={resyncToken}
              onSaved={onSaved}
              render={(value, setValue) => (
                <Dropdown
                  value={value as AgentProvider}
                  options={PROVIDERS}
                  onChange={(next) => setValue(next)}
                  ariaLabel="Agent provider"
                />
              )}
            />
          </Section>

          <Section heading="Model">
            <SaveField
              section="agent"
              fieldKey="model"
              label="Agent model"
              helper={configHelp('agent', 'model')}
              initialValue={config.agent.model}
              resyncToken={resyncToken}
              onSaved={onSaved}
              render={(value, setValue, errored) => (
                <TextField
                  value={value}
                  onChange={setValue}
                  placeholder="e.g. llama3.2, gpt-4o, claude-sonnet-4-20250514"
                  errored={errored}
                  ariaLabel="Agent model"
                  suggestions={PROVIDER_MODEL_SUGGESTIONS[provider] ?? []}
                />
              )}
            />
          </Section>

          <Section heading="Connection">
            <SaveField
              section="agent"
              fieldKey="base_url"
              label="Base URL"
              helper={configHelp('agent', 'base_url')}
              initialValue={config.agent.base_url}
              resyncToken={resyncToken}
              onSaved={onSaved}
              render={(value, setValue, errored) => (
                <TextField
                  value={value}
                  onChange={setValue}
                  placeholder={
                    provider === 'openai'
                      ? 'https://api.openai.com/v1'
                      : provider === 'anthropic'
                        ? 'https://api.anthropic.com'
                        : 'http://127.0.0.1:11434'
                  }
                  errored={errored}
                  ariaLabel="Agent base URL"
                />
              )}
            />
          </Section>

          {provider !== 'ollama' ? (
            <Section heading="API Key">
              <div className="flex flex-col gap-2">
                <label className="text-xs font-medium" style={{ color: 'var(--color-text-secondary)' }}>
                  API Key ({PROVIDER_LABELS[provider]})
                </label>
                <input
                  type="password"
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  onBlur={() => void saveApiKey(apiKey)}
                  placeholder={provider === 'openai' ? 'sk-...' : 'sk-ant-...'}
                  className="w-full bg-transparent border-b border-white/20 text-sm focus:outline-none focus:border-primary"
                  style={{ color: 'var(--color-text-primary)', padding: '4px 0' }}
                />
                <span className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                  Stored securely in local database, not in config.toml.
                </span>
              </div>
            </Section>
          ) : null}
        </>
      ) : null}

      <Section heading="Kite Passport">
            <div className="flex flex-col gap-3">
              <p
                className="text-xs"
                style={{ color: 'var(--color-text-secondary)', lineHeight: 1.5 }}
              >
                Thikra can install Kite Passport CLI and start Passport sign-up
                with your email. Wallet creation and agent provisioning still
                happen in the Kite Portal during the current invite-only
                testnet. Thikra stores the raw MCP URL locally and uses it to
                run `/kite` commands.
              </p>

              <div className="flex flex-col gap-1">
                <label
                  className="text-xs font-medium"
                  style={{ color: 'var(--color-text-secondary)' }}
                >
                  Signup email
                </label>
                <input
                  type="email"
                  value={kiteSignupEmail}
                  onChange={(e) => setKiteSignupEmail(e.target.value)}
                  placeholder="you@example.com"
                  className="w-full bg-transparent border-b border-white/20 text-sm focus:outline-none focus:border-primary"
                  style={{
                    color: 'var(--color-text-primary)',
                    padding: '4px 0',
                  }}
                />
                <span
                  className="text-xs"
                  style={{ color: 'var(--color-text-secondary)' }}
                >
                  `/kite setup` uses this email to start Kite Passport signup,
                  then pauses for the 8-character code from your inbox.
                </span>
              </div>

              <div className="flex flex-col gap-1">
                <label
                  className="text-xs font-medium"
                  style={{ color: 'var(--color-text-secondary)' }}
                >
                  Portal MCP URL
                </label>
                <input
                  type="password"
                  value={kiteMcpUrl}
                  onChange={(e) => setKiteMcpUrl(e.target.value)}
                  placeholder="Paste the MCP URL from Kite Portal"
                  className="w-full bg-transparent border-b border-white/20 text-sm focus:outline-none focus:border-primary"
                  style={{
                    color: 'var(--color-text-primary)',
                    padding: '4px 0',
                  }}
                />
                <span
                  className="text-xs"
                  style={{ color: 'var(--color-text-secondary)' }}
                >
                  The raw MCP URL may contain agent-specific auth material, so
                  it is kept out of `config.toml`.
                </span>
              </div>

              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => void saveKiteSignupEmail()}
                  disabled={kiteLoading || kiteSignupEmail.trim().length === 0}
                  className="text-xs font-medium rounded-lg px-3 py-1.5 transition-opacity"
                  style={{
                    background: 'rgba(255,255,255,0.08)',
                    color: 'var(--color-text-primary)',
                    border: '1px solid rgba(255,255,255,0.12)',
                    cursor:
                      kiteLoading || kiteSignupEmail.trim().length === 0
                        ? 'not-allowed'
                        : 'pointer',
                    opacity:
                      kiteLoading || kiteSignupEmail.trim().length === 0
                        ? 0.5
                        : 1,
                  }}
                >
                  Save email
                </button>
                <button
                  type="button"
                  onClick={() => void saveKiteMcpUrl()}
                  disabled={kiteLoading || kiteMcpUrl.trim().length === 0}
                  className="text-xs font-medium rounded-lg px-3 py-1.5 transition-opacity"
                  style={{
                    background: 'var(--color-primary)',
                    color: '#fff',
                    border: 'none',
                    cursor:
                      kiteLoading || kiteMcpUrl.trim().length === 0
                        ? 'not-allowed'
                        : 'pointer',
                    opacity:
                      kiteLoading || kiteMcpUrl.trim().length === 0 ? 0.5 : 1,
                  }}
                >
                  Save MCP URL
                </button>
                <button
                  type="button"
                  onClick={() => void verifyKite()}
                  disabled={kiteLoading || kiteMcpUrl.trim().length === 0}
                  className="text-xs font-medium rounded-lg px-3 py-1.5 transition-opacity"
                  style={{
                    background: 'rgba(255,255,255,0.08)',
                    color: 'var(--color-text-primary)',
                    border: '1px solid rgba(255,255,255,0.12)',
                    cursor:
                      kiteLoading || kiteMcpUrl.trim().length === 0
                        ? 'not-allowed'
                        : 'pointer',
                    opacity:
                      kiteLoading || kiteMcpUrl.trim().length === 0 ? 0.5 : 1,
                  }}
                >
                  Verify
                </button>
                <button
                  type="button"
                  onClick={() => void disconnectKite()}
                  disabled={kiteLoading}
                  className="text-xs font-medium rounded-lg px-3 py-1.5 transition-opacity"
                  style={{
                    background: 'transparent',
                    color: 'var(--color-text-secondary)',
                    border: '1px solid rgba(255,255,255,0.12)',
                    cursor: kiteLoading ? 'not-allowed' : 'pointer',
                    opacity: kiteLoading ? 0.5 : 1,
                  }}
                >
                  Disconnect
                </button>
                <button
                  type="button"
                  onClick={() => void startKiteAiSetup()}
                  disabled={kiteLoading}
                  className="text-xs font-medium rounded-lg px-3 py-1.5 transition-opacity"
                  style={{
                    background: 'rgba(120, 180, 255, 0.14)',
                    color: 'var(--color-text-primary)',
                    border: '1px solid rgba(120, 180, 255, 0.22)',
                    cursor: kiteLoading ? 'not-allowed' : 'pointer',
                    opacity: kiteLoading ? 0.5 : 1,
                  }}
                >
                  Use AI to finish setup
                </button>
              </div>

              <div className="flex flex-wrap gap-2">
                <button
                  type="button"
                  onClick={() => void openKiteTarget('portal')}
                  className="text-xs"
                  style={{
                    color: 'var(--color-text-primary)',
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    padding: 0,
                  }}
                >
                  Open Kite Portal
                </button>
                <button
                  type="button"
                  onClick={() => void installKiteCli()}
                  disabled={kiteLoading}
                  className="text-xs"
                  style={{
                    color: 'var(--color-text-primary)',
                    background: 'none',
                    border: 'none',
                    cursor: kiteLoading ? 'not-allowed' : 'pointer',
                    opacity: kiteLoading ? 0.5 : 1,
                    padding: 0,
                  }}
                >
                  Install Kite CLI
                </button>
                <button
                  type="button"
                  onClick={() => void openKiteTarget('docs')}
                  className="text-xs"
                  style={{
                    color: 'var(--color-text-primary)',
                    background: 'none',
                    border: 'none',
                    cursor: 'pointer',
                    padding: 0,
                  }}
                >
                  Open Kite Docs
                </button>
              </div>

              <div className="flex flex-col gap-1 text-xs">
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  Hub:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    Native account, wallet, sessions, activity, and shopping
                    surfaces are now mapped directly to Kite Passport.
                  </span>
                </span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  CLI:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    {kiteStatus?.cliInstalled
                      ? kiteStatus.cliPath ?? 'Installed'
                      : 'Not found'}
                  </span>
                </span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  Connection:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    {kiteStatus?.connected ? 'Ready' : 'Not connected'}
                  </span>
                </span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  AI mode:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    {kiteCapability?.available
                      ? `Autopilot available via ${kiteCapability.provider}${
                          kiteCapability.model
                            ? ` (${kiteCapability.model})`
                            : ''
                        }`
                      : 'Guided help only'}
                  </span>
                </span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  AI detail:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    {kiteCapability?.reason ?? 'Checking Kite AI capability…'}
                  </span>
                </span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  Auth/session:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    {describeKiteAuthState(kiteStatus?.authState ?? 'unknown_error')}
                  </span>
                </span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  Saved MCP URL:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    {kiteStatus?.maskedMcpUrl ?? 'Not set'}
                  </span>
                </span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  Last payer:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    {kiteStatus?.lastPayerAddr ?? 'Unknown'}
                  </span>
                </span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  Signup email:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    {(kiteStatus?.signupEmail ?? kiteSignupEmail) || 'Not set'}
                  </span>
                </span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  Pending signup:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    {kiteStatus?.pendingSignupId ?? 'None'}
                  </span>
                </span>
                <span style={{ color: 'var(--color-text-secondary)' }}>
                  Windows note:{' '}
                  <span style={{ color: 'var(--color-text-primary)' }}>
                    Install Kite CLI from Thikra. If Kite&apos;s official
                    PowerShell bootstrap is broken, Thikra uses its own Windows
                    installer path.
                  </span>
                </span>
              </div>

              {kiteHub ? (
                <div className="flex flex-col gap-3">
                  <div
                    className="rounded-xl border p-3"
                    style={{
                      borderColor: 'rgba(255,255,255,0.12)',
                      background: 'rgba(255,255,255,0.04)',
                    }}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-xs font-medium" style={{ color: 'var(--color-text-primary)' }}>
                        Account
                      </span>
                      <button
                        type="button"
                        onClick={() => void logoutKiteAccount()}
                        disabled={kiteLoading || !kiteHub.account.loggedIn}
                        className="text-xs"
                        style={{
                          color: 'var(--color-text-secondary)',
                          background: 'none',
                          border: 'none',
                          cursor:
                            kiteLoading || !kiteHub.account.loggedIn
                              ? 'not-allowed'
                              : 'pointer',
                          opacity:
                            kiteLoading || !kiteHub.account.loggedIn ? 0.5 : 1,
                          padding: 0,
                        }}
                      >
                        Logout
                      </button>
                    </div>
                    <div className="mt-2 flex flex-col gap-1 text-xs">
                      <span style={{ color: 'var(--color-text-secondary)' }}>
                        Status:{' '}
                        <span style={{ color: 'var(--color-text-primary)' }}>
                          {kiteHub.account.loggedIn ? 'Logged in' : 'Awaiting login'}
                        </span>
                      </span>
                      <span style={{ color: 'var(--color-text-secondary)' }}>
                        Email:{' '}
                        <span style={{ color: 'var(--color-text-primary)' }}>
                          {kiteHub.account.email ?? kiteHub.account.signupEmail ?? 'Not set'}
                        </span>
                      </span>
                      <span style={{ color: 'var(--color-text-secondary)' }}>
                        User ID:{' '}
                        <span style={{ color: 'var(--color-text-primary)' }}>
                          {kiteHub.account.userId ?? 'Unknown'}
                        </span>
                      </span>
                      <span style={{ color: 'var(--color-text-secondary)' }}>
                        Agent:{' '}
                        <span style={{ color: 'var(--color-text-primary)' }}>
                          {kiteHub.account.currentAgentIdentity ?? 'Not registered'}
                        </span>
                      </span>
                    </div>
                  </div>

                  <div
                    className="rounded-xl border p-3"
                    style={{
                      borderColor: 'rgba(255,255,255,0.12)',
                      background: 'rgba(255,255,255,0.04)',
                    }}
                  >
                    <span className="text-xs font-medium" style={{ color: 'var(--color-text-primary)' }}>
                      Wallet
                    </span>
                    <div className="mt-2 flex flex-col gap-1 text-xs">
                      <span style={{ color: 'var(--color-text-secondary)' }}>
                        Address:{' '}
                        <span style={{ color: 'var(--color-text-primary)' }}>
                          {kiteHub.wallet?.walletAddress ?? 'Unavailable'}
                        </span>
                      </span>
                      <span style={{ color: 'var(--color-text-secondary)' }}>
                        Chain:{' '}
                        <span style={{ color: 'var(--color-text-primary)' }}>
                          {kiteHub.wallet?.chainId ?? 'Unknown'}
                        </span>
                      </span>
                      {kiteHub.wallet?.assets.map((asset) => (
                        <span key={asset.symbol} style={{ color: 'var(--color-text-secondary)' }}>
                          {asset.symbol}:{' '}
                          <span style={{ color: 'var(--color-text-primary)' }}>
                            {asset.balance}
                            {asset.native ? ' (native)' : ''}
                          </span>
                        </span>
                      ))}
                    </div>
                    <div className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-3">
                      <input
                        type="text"
                        value={kiteSendTo}
                        onChange={(e) => setKiteSendTo(e.target.value)}
                        placeholder="0x recipient"
                        className="w-full bg-transparent border-b border-white/20 text-sm focus:outline-none focus:border-primary"
                        style={{ color: 'var(--color-text-primary)', padding: '4px 0' }}
                      />
                      <input
                        type="text"
                        value={kiteSendAmount}
                        onChange={(e) => setKiteSendAmount(e.target.value)}
                        placeholder="Amount"
                        className="w-full bg-transparent border-b border-white/20 text-sm focus:outline-none focus:border-primary"
                        style={{ color: 'var(--color-text-primary)', padding: '4px 0' }}
                      />
                      <input
                        type="text"
                        value={kiteSendAsset}
                        onChange={(e) => setKiteSendAsset(e.target.value)}
                        placeholder="Asset"
                        className="w-full bg-transparent border-b border-white/20 text-sm focus:outline-none focus:border-primary"
                        style={{ color: 'var(--color-text-primary)', padding: '4px 0' }}
                      />
                    </div>
                    <div className="mt-2 flex flex-wrap gap-2">
                      <button
                        type="button"
                        onClick={() => void sendKiteWalletTransfer()}
                        disabled={
                          kiteLoading ||
                          kiteSendTo.trim().length === 0 ||
                          kiteSendAmount.trim().length === 0 ||
                          kiteSendAsset.trim().length === 0
                        }
                        className="text-xs font-medium rounded-lg px-3 py-1.5 transition-opacity"
                        style={{
                          background: 'rgba(255,255,255,0.08)',
                          color: 'var(--color-text-primary)',
                          border: '1px solid rgba(255,255,255,0.12)',
                          cursor:
                            kiteLoading ||
                            kiteSendTo.trim().length === 0 ||
                            kiteSendAmount.trim().length === 0 ||
                            kiteSendAsset.trim().length === 0
                              ? 'not-allowed'
                              : 'pointer',
                          opacity:
                            kiteLoading ||
                            kiteSendTo.trim().length === 0 ||
                            kiteSendAmount.trim().length === 0 ||
                            kiteSendAsset.trim().length === 0
                              ? 0.5
                              : 1,
                        }}
                      >
                        Send token
                      </button>
                      <button
                        type="button"
                        onClick={() => void requestKiteFaucet()}
                        disabled={kiteLoading || !kiteHub.wallet?.canUseFaucet}
                        className="text-xs font-medium rounded-lg px-3 py-1.5 transition-opacity"
                        style={{
                          background: 'rgba(120, 180, 255, 0.14)',
                          color: 'var(--color-text-primary)',
                          border: '1px solid rgba(120, 180, 255, 0.22)',
                          cursor:
                            kiteLoading || !kiteHub.wallet?.canUseFaucet
                              ? 'not-allowed'
                              : 'pointer',
                          opacity: kiteLoading || !kiteHub.wallet?.canUseFaucet ? 0.5 : 1,
                        }}
                      >
                        Faucet {kiteFaucetToken}
                      </button>
                    </div>
                  </div>

                  <div
                    className="rounded-xl border p-3"
                    style={{
                      borderColor: 'rgba(255,255,255,0.12)',
                      background: 'rgba(255,255,255,0.04)',
                    }}
                  >
                    <span className="text-xs font-medium" style={{ color: 'var(--color-text-primary)' }}>
                      Sessions
                    </span>
                    <div className="mt-2 flex flex-col gap-2">
                      {kiteHub.sessions.length === 0 ? (
                        <span className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                          No sessions yet. Use `/kite session create ...` to request one.
                        </span>
                      ) : (
                        kiteHub.sessions.map((session) => (
                          <div
                            key={session.id}
                            className="rounded-lg border p-2"
                            style={{ borderColor: 'rgba(255,255,255,0.08)' }}
                          >
                            <div className="flex items-center justify-between gap-2">
                              <span className="text-xs font-medium" style={{ color: 'var(--color-text-primary)' }}>
                                {session.id}
                              </span>
                              <button
                                type="button"
                                onClick={() => void selectKiteSession(session.id)}
                                disabled={kiteLoading || session.selected}
                                className="text-xs"
                                style={{
                                  color: session.selected
                                    ? 'var(--color-text-secondary)'
                                    : 'var(--color-text-primary)',
                                  background: 'none',
                                  border: 'none',
                                  cursor:
                                    kiteLoading || session.selected
                                      ? 'not-allowed'
                                      : 'pointer',
                                  padding: 0,
                                }}
                              >
                                {session.selected ? 'Selected' : 'Use session'}
                              </button>
                            </div>
                            <div className="mt-1 flex flex-col gap-1 text-xs">
                              <span style={{ color: 'var(--color-text-secondary)' }}>
                                Status:{' '}
                                <span style={{ color: 'var(--color-text-primary)' }}>
                                  {session.status}
                                </span>
                              </span>
                              {session.taskSummary ? (
                                <span style={{ color: 'var(--color-text-secondary)' }}>
                                  Task:{' '}
                                  <span style={{ color: 'var(--color-text-primary)' }}>
                                    {session.taskSummary}
                                  </span>
                                </span>
                              ) : null}
                            </div>
                          </div>
                        ))
                      )}
                    </div>
                  </div>

                  <div
                    className="rounded-xl border p-3"
                    style={{
                      borderColor: 'rgba(255,255,255,0.12)',
                      background: 'rgba(255,255,255,0.04)',
                    }}
                  >
                    <span className="text-xs font-medium" style={{ color: 'var(--color-text-primary)' }}>
                      Activity
                    </span>
                    <div className="mt-2 flex flex-col gap-2">
                      {kiteHub.activity.length === 0 ? (
                        <span className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                          No activity yet.
                        </span>
                      ) : (
                        kiteHub.activity.slice(0, 5).map((event) => (
                          <div key={event.id} className="text-xs">
                            <span style={{ color: 'var(--color-text-primary)' }}>
                              {event.title}
                            </span>
                            <span style={{ color: 'var(--color-text-secondary)' }}>
                              {' '}
                              {event.amountDisplay ? `• ${event.amountDisplay}` : ''} • {event.occurredAt}
                            </span>
                          </div>
                        ))
                      )}
                    </div>
                  </div>

                  <div
                    className="rounded-xl border p-3"
                    style={{
                      borderColor: 'rgba(255,255,255,0.12)',
                      background: 'rgba(255,255,255,0.04)',
                    }}
                  >
                    <span className="text-xs font-medium" style={{ color: 'var(--color-text-primary)' }}>
                      Shopping
                    </span>
                    <div className="mt-2 flex flex-col gap-2">
                      <input
                        type="text"
                        value={kiteShopQuery}
                        onChange={(e) => setKiteShopQuery(e.target.value)}
                        placeholder="Search products with Kite"
                        className="w-full bg-transparent border-b border-white/20 text-sm focus:outline-none focus:border-primary"
                        style={{ color: 'var(--color-text-primary)', padding: '4px 0' }}
                      />
                      <div className="flex gap-2">
                        <button
                          type="button"
                          onClick={() => void searchKiteShop()}
                          disabled={kiteLoading || kiteShopQuery.trim().length === 0}
                          className="text-xs font-medium rounded-lg px-3 py-1.5 transition-opacity"
                          style={{
                            background: 'rgba(255,255,255,0.08)',
                            color: 'var(--color-text-primary)',
                            border: '1px solid rgba(255,255,255,0.12)',
                            cursor:
                              kiteLoading || kiteShopQuery.trim().length === 0
                                ? 'not-allowed'
                                : 'pointer',
                            opacity:
                              kiteLoading || kiteShopQuery.trim().length === 0 ? 0.5 : 1,
                          }}
                        >
                          Search
                        </button>
                      </div>
                      {kiteShopResults.length > 0 ? (
                        <div className="flex flex-col gap-2">
                          {kiteShopResults.slice(0, 4).map((item) => (
                            <div
                              key={`${item.provider}:${item.externalIdentifier}`}
                              className="rounded-lg border p-2"
                              style={{ borderColor: 'rgba(255,255,255,0.08)' }}
                            >
                              <div className="flex items-start justify-between gap-2">
                                <div className="flex flex-col gap-1">
                                  <span className="text-xs font-medium" style={{ color: 'var(--color-text-primary)' }}>
                                    {item.title}
                                  </span>
                                  <span className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                                    {item.price}
                                    {item.rating ? ` • ${item.rating}★` : ''}
                                  </span>
                                </div>
                                <button
                                  type="button"
                                  onClick={() => void addKiteCartItem(item)}
                                  disabled={kiteLoading}
                                  className="text-xs"
                                  style={{
                                    color: 'var(--color-text-primary)',
                                    background: 'none',
                                    border: 'none',
                                    cursor: kiteLoading ? 'not-allowed' : 'pointer',
                                    padding: 0,
                                  }}
                                >
                                  Add to cart
                                </button>
                              </div>
                            </div>
                          ))}
                        </div>
                      ) : null}
                      <div className="flex flex-col gap-2">
                        <span className="text-xs font-medium" style={{ color: 'var(--color-text-primary)' }}>
                          Cart
                        </span>
                        {kiteHub.cart?.items.length ? (
                          kiteHub.cart.items.map((item) => (
                            <div
                              key={item.productLocator}
                              className="flex items-center justify-between gap-2 text-xs"
                            >
                              <span style={{ color: 'var(--color-text-secondary)' }}>
                                <span style={{ color: 'var(--color-text-primary)' }}>
                                  {item.title}
                                </span>{' '}
                                • {item.price} ×{item.quantity}
                              </span>
                              <button
                                type="button"
                                onClick={() => void removeKiteCartItem(item)}
                                disabled={kiteLoading}
                                className="text-xs"
                                style={{
                                  color: 'var(--color-text-secondary)',
                                  background: 'none',
                                  border: 'none',
                                  cursor: kiteLoading ? 'not-allowed' : 'pointer',
                                  padding: 0,
                                }}
                              >
                                Remove
                              </button>
                            </div>
                          ))
                        ) : (
                          <span className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                            Cart is empty.
                          </span>
                        )}
                        {kiteHub.orders.length > 0 ? (
                          <div className="flex flex-col gap-1">
                            <span className="text-xs font-medium" style={{ color: 'var(--color-text-primary)' }}>
                              Recent orders
                            </span>
                            {kiteHub.orders.slice(0, 3).map((order) => (
                              <span key={order.orderId} className="text-xs" style={{ color: 'var(--color-text-secondary)' }}>
                                <span style={{ color: 'var(--color-text-primary)' }}>
                                  {order.orderId}
                                </span>{' '}
                                • {order.phase ?? order.paymentStatus ?? 'pending'}
                              </span>
                            ))}
                          </div>
                        ) : null}
                      </div>
                    </div>
                  </div>
                </div>
              ) : null}

              {kiteHub?.issues.length ? (
                <div className="flex flex-col gap-1 text-xs">
                  {kiteHub.issues.slice(0, 4).map((issue) => (
                    <span key={issue} style={{ color: '#ffb366' }}>
                      {issue}
                    </span>
                  ))}
                </div>
              ) : null}

              {kiteMessage ? (
                <p
                  className="text-xs"
                  style={{ color: 'var(--color-text-secondary)' }}
                >
                  {kiteMessage}
                </p>
              ) : null}
            </div>
      </Section>
    </>
  );
}
