import { useState, useCallback } from 'react';
import { invoke, Channel } from '@tauri-apps/api/core';
import { notifyIfUnfocused } from '../utils/notification';
import type {
  SearchEvent,
  SearchResultPreview,
  SearchMetadata,
  SearchStage,
  SearchTraceStep,
  SearchWarning,
} from '../types/search';

/** Mirrors the Rust OllamaErrorKind enum sent over IPC. */
export type OllamaErrorKind = 'NotRunning' | 'ModelNotFound' | 'Other';

/**
 * Represents a single message in the chat thread.
 */
export interface Message {
  /** Unique identifier for stable React list keys. */
  id: string;
  role: 'user' | 'assistant';
  content: string;
  /** Selected text from the host app that was quoted with this message, if any. */
  quotedText?: string;
  /** Absolute file paths of images attached to this message, if any. */
  imagePaths?: string[];
  /** Present on assistant messages that represent an Ollama error callout. */
  errorKind?: OllamaErrorKind;
  /** Accumulated thinking/reasoning content from the model, if thinking mode was used. */
  thinkingContent?: string;
  /** Search result sources from /search pipeline. */
  searchSources?: SearchResultPreview[];
  /** Search warnings from /search pipeline. */
  searchWarnings?: SearchWarning[];
  /** Trace steps from /search pipeline. */
  searchTraces?: SearchTraceStep[];
  /** Metadata from /search pipeline. */
  searchMetadata?: SearchMetadata;
  /** True when the search sandbox was unreachable. */
  sandboxUnavailable?: boolean;
}

/**
 * The expected structure of streaming chunks emitted from the Rust backend.
 */
export type StreamChunk =
  | { type: 'Token'; data: string }
  | { type: 'ThinkingToken'; data: string }
  | { type: 'Done' }
  | { type: 'Cancelled' }
  | { type: 'Error'; data: { kind: OllamaErrorKind; message: string } };

export type KiteEvent =
  | { type: 'checking_cli' }
  | { type: 'installing_cli' }
  | { type: 'installing_agent_passport' }
  | { type: 'opening_portal'; data: { target: string } }
  | { type: 'waiting_for_mcp_config' }
  | { type: 'verifying_connection' }
  | { type: 'fetching_payer' }
  | { type: 'approving_payment' }
  | { type: 'retrying_paid_request' }
  | { type: 'entering_agent_mode'; data: { reason: string } }
  | { type: 'advisory_fallback'; data: { reason: string; guidance: string } }
  | {
      type: 'awaiting_sensitive_value';
      data: { field: string; instructions: string };
    }
  | {
      type: 'awaiting_payment_confirmation';
      data: { action_id: string; summary: string };
    }
  | { type: 'resuming_after_user_step'; data: { step: string } }
  | { type: 'token'; data: string }
  | { type: 'done' }
  | { type: 'error'; data: string };

/** Result payload delivered to callers when a `/search` pipeline turn finishes. */
export interface SearchOutcome {
  final: boolean;
}

type BasicKiteProgressEvent = Extract<
  KiteEvent,
  | { type: 'checking_cli' }
  | { type: 'installing_cli' }
  | { type: 'installing_agent_passport' }
  | { type: 'opening_portal'; data: { target: string } }
  | { type: 'waiting_for_mcp_config' }
  | { type: 'verifying_connection' }
  | { type: 'fetching_payer' }
  | { type: 'approving_payment' }
  | { type: 'retrying_paid_request' }
>;

function describeKiteEvent(event: BasicKiteProgressEvent): string {
  switch (event.type) {
    case 'checking_cli':
      return 'Checking Kite CLI…';
    case 'installing_cli':
      return 'Installing Kite CLI…';
    case 'installing_agent_passport':
      return 'Finishing Kite Agent Passport setup…';
    case 'opening_portal':
      return `Opening Kite ${event.data.target}…`;
    case 'waiting_for_mcp_config':
      return 'Waiting for Kite MCP configuration…';
    case 'verifying_connection':
      return 'Verifying Kite connection…';
    case 'fetching_payer':
      return 'Fetching Kite payer address…';
    case 'approving_payment':
      return 'Approving Kite payment…';
    case 'retrying_paid_request':
      return 'Retrying paid Kite request…';
  }
}

/**
 * A custom hook that simplifies interactions with the local Ollama LLM.
 */
function describeExtendedKiteEvent(
  event: Exclude<
    KiteEvent,
    { type: 'token' } | { type: 'done' } | { type: 'error'; data: string }
  >,
): string {
  if (
    event.type === 'checking_cli' ||
    event.type === 'installing_cli' ||
    event.type === 'installing_agent_passport' ||
    event.type === 'opening_portal' ||
    event.type === 'waiting_for_mcp_config' ||
    event.type === 'verifying_connection' ||
    event.type === 'fetching_payer' ||
    event.type === 'approving_payment' ||
    event.type === 'retrying_paid_request'
  ) {
    return describeKiteEvent(event);
  }

  switch (event.type) {
    case 'entering_agent_mode':
      return 'Kite agentic mode is taking over...';
    case 'advisory_fallback':
      return 'Kite is switching to AI troubleshooting guidance...';
    case 'awaiting_sensitive_value':
      return 'Waiting for a manual Kite setup value...';
    case 'awaiting_payment_confirmation':
      return 'Waiting for payment confirmation...';
    case 'resuming_after_user_step':
      return 'Resuming the Kite flow...';
  }
}

/**
 * A custom hook that simplifies interactions with the local Ollama LLM.
 * It manages message history, streaming state, and sets up Rust IPC channels.
 *
 * @param onTurnComplete Optional callback invoked after a complete user/assistant
 *   turn (i.e., when the `Done` chunk is received). Receives the user message
 *   and the finalized assistant message. Not called on `Cancelled` or `Error`.
 *   Used by the caller to persist completed turns to SQLite.
 * @returns An object containing the message history, a submit callback function, and operational states.
 */
export function useOllama(
  onTurnComplete?: (userMsg: Message, assistantMsg: Message) => void,
) {
  const [messages, setMessages] = useState<Message[]>([]);
  const [isGenerating, setIsGenerating] = useState(false);
  const [searchStage, setSearchStage] = useState<SearchStage>(null);

  /**
   * Submits a message to the Ollama backend and initiates the streaming response.
   * The backend manages conversation history — only the new user message is sent.
   *
   * Streams tokens directly into the messages array. An empty assistant placeholder
   * is added immediately, then updated in-place on each token until generation finishes.
   *
   * @param displayContent The user's query as it should appear in the chat bubble.
   * @param quotedText Optional selected text quoted alongside this message.
   * @param imagePaths Optional array of absolute file paths for attached images.
   * @param think When true, enables Ollama's thinking/reasoning mode.
   * @param promptOverride When provided, sent to the backend as the actual message
   *   instead of displayContent. The chat bubble still shows displayContent.
   *   Used by utility slash commands to send a composed prompt template while
   *   displaying the user's original input.
   */
  const ask = useCallback(
    async (
      displayContent: string,
      quotedText?: string,
      imagePaths?: string[],
      think?: boolean,
      promptOverride?: string,
    ) => {
      if (
        (!displayContent.trim() && (!imagePaths || imagePaths.length === 0)) ||
        isGenerating
      )
        return;

      const userMsg: Message = {
        id: crypto.randomUUID(),
        role: 'user',
        content: displayContent,
        quotedText,
        imagePaths:
          imagePaths && imagePaths.length > 0 ? imagePaths : undefined,
      };

      const assistantId = crypto.randomUUID();
      const assistantMsg: Message = {
        id: assistantId,
        role: 'assistant',
        content: '',
      };

      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      setIsGenerating(true);

      const channel = new Channel<StreamChunk>();
      let currentContent = '';
      let currentThinkingContent = '';

      channel.onmessage = (chunk) => {
        if (chunk.type === 'ThinkingToken') {
          currentThinkingContent += chunk.data;
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId
                ? { ...m, thinkingContent: currentThinkingContent }
                : m,
            ),
          );
        } else if (chunk.type === 'Token') {
          currentContent += chunk.data;
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId ? { ...m, content: currentContent } : m,
            ),
          );
        } else if (chunk.type === 'Done') {
          setIsGenerating(false);
          // Show a desktop toast when the user has switched away from the app.
          void notifyIfUnfocused('windowsMate - Thuki', 'Response ready');
          // Notify the caller that a complete turn has finished so it can
          // persist both messages to SQLite if the conversation is saved.
          onTurnComplete?.(userMsg, {
            ...assistantMsg,
            content: currentContent,
            thinkingContent: currentThinkingContent || undefined,
          });
        } else if (chunk.type === 'Cancelled') {
          // Remove the empty assistant placeholder if nothing was generated.
          if (!currentContent && !currentThinkingContent) {
            setMessages((prev) => prev.filter((m) => m.id !== assistantId));
          }
          setIsGenerating(false);
        } else {
          // Replace the streaming placeholder with an error message.
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId
                ? {
                    ...m,
                    content: chunk.data.message,
                    errorKind: chunk.data.kind,
                  }
                : m,
            ),
          );
          setIsGenerating(false);
        }
      };

      try {
        await invoke('ask_ollama', {
          message: promptOverride ?? displayContent,
          quotedText: quotedText ?? null,
          imagePaths: imagePaths && imagePaths.length > 0 ? imagePaths : null,
          think: think ?? false,
          onEvent: channel,
        });
      } catch {
        setMessages((prev) => [
          ...prev,
          {
            id: crypto.randomUUID(),
            role: 'assistant',
            content: 'Something went wrong\nCould not reach Ollama.',
            errorKind: 'Other' as const,
          },
        ]);
        setIsGenerating(false);
      }
    },
    [isGenerating, onTurnComplete],
  );

  /** Runs the agentic search pipeline for the `/search` command. */
  const askSearch = useCallback(
    async (
      query: string,
      displayContent?: string,
      quotedText?: string,
    ): Promise<SearchOutcome> => {
      const trimmed = query.trim();
      if (!trimmed) return { final: true };
      if (isGenerating) return { final: true };

      const userMsg: Message = {
        id: crypto.randomUUID(),
        role: 'user',
        content: displayContent ?? trimmed,
        quotedText,
      };
      const assistantId = crypto.randomUUID();
      const assistantMsg: Message = {
        id: assistantId,
        role: 'assistant',
        content: '',
      };

      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      setIsGenerating(true);
      setSearchStage(null);

      const channel = new Channel<SearchEvent>();
      let currentContent = '';
      let pendingSources: SearchResultPreview[] | undefined;
      let warnings: SearchWarning[] = [];
      let pendingTraces: SearchTraceStep[] = [];
      let pendingMetadata: SearchMetadata | undefined;
      let awaitingClarification = false;
      let errored = false;
      let cancelled = false;

      const updateAssistant = (patch: Partial<Message>) => {
        setMessages((prev) =>
          prev.map((message) =>
            message.id === assistantId ? { ...message, ...patch } : message,
          ),
        );
      };

      return new Promise<SearchOutcome>((resolve) => {
        let inGapRound = false;

        const finish = (final: boolean) => {
          setIsGenerating(false);
          setSearchStage(null);

          if (!errored && !cancelled && currentContent) {
            updateAssistant({
              searchSources: pendingSources,
              searchWarnings: warnings.length > 0 ? warnings : undefined,
              searchTraces: pendingTraces,
              searchMetadata: pendingMetadata,
            });
            onTurnComplete?.(userMsg, {
              ...assistantMsg,
              content: currentContent,
              searchSources: pendingSources,
              searchWarnings: warnings.length > 0 ? warnings : undefined,
              searchTraces: pendingTraces,
              searchMetadata: pendingMetadata,
            });
          }

          resolve({ final });
        };

        channel.onmessage = (event: SearchEvent) => {
          switch (event.type) {
            case 'Trace': {
              const existingIdx = pendingTraces.findIndex(
                (s) => s.id === event.step.id,
              );
              if (existingIdx === -1) {
                pendingTraces = [...pendingTraces, event.step];
              } else {
                pendingTraces = pendingTraces.map((s) =>
                  s.id === event.step.id ? event.step : s,
                );
              }
              awaitingClarification ||= event.step.kind === 'clarify';
              updateAssistant({ searchTraces: pendingTraces });
              break;
            }
            case 'AnalyzingQuery': {
              setSearchStage({ kind: 'analyzing_query' });
              break;
            }
            case 'Searching': {
              setSearchStage(
                inGapRound ? { kind: 'searching', gap: true } : { kind: 'searching' },
              );
              break;
            }
            case 'FetchingUrl':
            case 'ReadingSources': {
              setSearchStage(
                inGapRound
                  ? { kind: 'reading_sources', gap: true }
                  : { kind: 'reading_sources' },
              );
              break;
            }
            case 'RefiningSearch': {
              inGapRound = true;
              setSearchStage({
                kind: 'refining_search',
                attempt: event.attempt,
                total: event.total,
              });
              break;
            }
            case 'Composing': {
              setSearchStage(
                inGapRound ? { kind: 'composing', gap: true } : { kind: 'composing' },
              );
              break;
            }
            case 'Sources': {
              pendingSources = event.results;
              break;
            }
            case 'Token': {
              currentContent += event.content;
              setSearchStage(null);
              updateAssistant({ content: currentContent });
              break;
            }
            case 'Warning': {
              warnings = [...warnings, event.warning];
              break;
            }
            case 'Done': {
              pendingMetadata = event.metadata ?? pendingMetadata;
              finish(!awaitingClarification && !!currentContent);
              break;
            }
            case 'Cancelled': {
              cancelled = true;
              if (!currentContent) {
                setMessages((prev) =>
                  prev.filter((message) => message.id !== assistantId),
                );
              }
              setIsGenerating(false);
              setSearchStage(null);
              resolve({ final: true });
              break;
            }
            case 'Error': {
              errored = true;
              updateAssistant({
                content: event.message,
                errorKind: 'Other',
              });
              finish(true);
              break;
            }
            case 'SandboxUnavailable': {
              errored = true;
              updateAssistant({ sandboxUnavailable: true });
              finish(true);
              break;
            }
            case 'IterationComplete': {
              // Finalize running trace steps for this iteration.
              const finalized = pendingTraces.map((s) =>
                s.status === 'running' ? { ...s, status: 'completed' as const } : s,
              );
              pendingTraces = finalized;
              updateAssistant({ searchTraces: finalized });
              break;
            }
          }
        };

        invoke('search_pipeline', {
          message: trimmed,
          onEvent: channel,
        }).catch(() => {
          if (errored || cancelled) return;
          errored = true;
          updateAssistant({
            content: 'Something went wrong\nCould not start search.',
            errorKind: 'Other' as const,
          });
          finish(true);
        });
      });
    },
    [isGenerating, onTurnComplete],
  );

  /** Runs the native Kite backend flow for `/kite` commands. */
  const askKite = useCallback(
    async (
      input: string,
      displayContent?: string,
      quotedText?: string,
    ) => {
      const trimmed = input.trim();
      if (!trimmed || isGenerating) return;

      const userMsg: Message = {
        id: crypto.randomUUID(),
        role: 'user',
        content: displayContent ?? trimmed,
        quotedText,
      };
      const assistantId = crypto.randomUUID();
      const assistantMsg: Message = {
        id: assistantId,
        role: 'assistant',
        content: '',
      };

      setMessages((prev) => [...prev, userMsg, assistantMsg]);
      setIsGenerating(true);
      setSearchStage(null);

      const channel = new Channel<KiteEvent>();
      let currentContent = '';
      let finished = false;
      let contentMode: 'empty' | 'progress' | 'final' = 'empty';

      const updateAssistant = (patch: Partial<Message>) => {
        setMessages((prev) =>
          prev.map((message) =>
            message.id === assistantId ? { ...message, ...patch } : message,
          ),
        );
      };

      const replaceAssistantContent = (
        content: string,
        mode: 'progress' | 'final',
      ) => {
        currentContent = content;
        contentMode = mode;
        updateAssistant({ content });
      };

      channel.onmessage = (event) => {
        switch (event.type) {
          case 'token':
            if (contentMode !== 'final') {
              replaceAssistantContent(event.data, 'final');
            } else if (event.data !== currentContent) {
              currentContent += event.data;
              updateAssistant({ content: currentContent });
            }
            break;
          case 'advisory_fallback':
            replaceAssistantContent(event.data.guidance, 'final');
            break;
          case 'awaiting_sensitive_value':
            if (contentMode !== 'final') {
              replaceAssistantContent(event.data.instructions, 'final');
            }
            break;
          case 'awaiting_payment_confirmation': {
            const approved = window.confirm(event.data.summary);
            const command = approved
              ? 'confirm_kite_payment_action'
              : 'reject_kite_payment_action';
            void invoke(command, { actionId: event.data.action_id });
            if (contentMode !== 'final') {
              replaceAssistantContent(
                approved
                  ? 'Payment confirmation sent. Resuming Kite flow...'
                  : 'Payment was cancelled before Kite signed anything.',
                'final',
              );
            }
            break;
          }
          case 'done':
            finished = true;
            setIsGenerating(false);
            void notifyIfUnfocused('windowsMate - Thuki', 'Kite response ready');
            onTurnComplete?.(userMsg, {
              ...assistantMsg,
              content: currentContent,
            });
            break;
          case 'error':
            finished = true;
            replaceAssistantContent(event.data, 'final');
            updateAssistant({ errorKind: 'Other' });
            setIsGenerating(false);
            break;
          default:
            if (contentMode !== 'final') {
              replaceAssistantContent(
                describeExtendedKiteEvent(event),
                'progress',
              );
            }
            break;
        }
      };

      try {
        await invoke('run_kite_command', {
          input: trimmed,
          onEvent: channel,
        });
        if (!finished) {
          setIsGenerating(false);
        }
      } catch {
        updateAssistant({
          content: 'Something went wrong\nCould not start the Kite flow.',
          errorKind: 'Other',
        });
        setIsGenerating(false);
      }
    },
    [isGenerating, onTurnComplete],
  );

  /** Cancels the currently active generation by signalling the Rust backend. */
  const cancel = useCallback(async () => {
    if (!isGenerating) return;
    await invoke('cancel_generation');
  }, [isGenerating]);

  /** Resets all conversation state to prepare for a fresh session. */
  const reset = useCallback(() => {
    setMessages([]);
    setIsGenerating(false);
    void invoke('reset_conversation');
  }, []);

  /**
   * Replaces the current message list with a previously loaded set of messages.
   *
   * Called after `load_conversation` returns from the backend (which already
   * synced the Rust `ConversationHistory`). Does NOT call `reset_conversation`
   * to avoid conflicting with the epoch bump performed by `load_conversation`.
   *
   * @param msgs The complete message array to load into React state.
   */
  const loadMessages = useCallback((msgs: Message[]) => {
    setMessages(msgs);
    setIsGenerating(false);
  }, []);

  /**
   * Appends one or more pre-built messages directly into the chat without
   * going through the Ollama streaming pipeline. Used by agent mode to
   * inject the user task and the agent result into the conversation.
   */
  const injectMessages = useCallback((msgs: Message[]) => {
    setMessages((prev) => [...prev, ...msgs]);
  }, []);

  return {
    messages,
    ask,
    askSearch,
    askKite,
    cancel,
    isGenerating,
    searchStage,
    reset,
    loadMessages,
    injectMessages,
  };
}
