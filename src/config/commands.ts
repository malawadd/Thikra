/**
 * Registry of all slash commands supported by the ask bar.
 *
 * Each entry drives the autocomplete UI, submit-time routing, generated docs,
 * and the prompt appendix composed into the runtime system prompt.
 */

export interface CommandDocs {
  readonly summary: string;
  readonly usage: string;
  readonly examples: readonly string[];
  readonly behavior: string;
  readonly composability?: string;
  readonly limit?: string;
  readonly permission?: string;
  readonly languageFormat?: string;
  readonly defaultBehavior?: string;
}

export interface CommandPromptHelp {
  readonly summary: string;
}

export interface Command {
  /** The slash trigger, e.g. "/screen". Must start with "/". */
  readonly trigger: string;
  /** Short label shown in the suggestion row. */
  readonly label: string;
  /** One-line description shown as muted subtext in the suggestion row. */
  readonly description: string;
  /** Richer command reference used for generated docs and prompt appendix text. */
  readonly docs: CommandDocs;
  /** Shorter description embedded into the runtime prompt appendix. */
  readonly promptHelp: CommandPromptHelp;
  /** Prompt template with $INPUT / $LANG placeholders. Absent for non-template commands. */
  readonly promptTemplate?: string;
}

function defineCommand(command: Command): Command {
  return command;
}

export const COMMANDS: readonly Command[] = [
  defineCommand({
    trigger: '/screen',
    label: '/screen',
    description: 'Capture your screen and include it as context',
    docs: {
      summary:
        'Captures a fresh screenshot and sends it with your message as visual context.',
      usage: '/screen [question or instruction]',
      examples: [
        '`/screen what error is this?`',
        '`/think /screen explain this chart`',
      ],
      behavior:
        'Captures the current screen, keeps the original typed message in the chat bubble, and sends the screenshot to a vision-capable model.',
      composability:
        'Can be combined with `/think`; takes precedence over follow-up search routing.',
      permission:
        'Requires screen capture permission.',
    },
    promptHelp: {
      summary:
        'Capture the current screen and include it as image context for the reply.',
    },
  }),
  defineCommand({
    trigger: '/do',
    label: '/do',
    description: 'Agent mode: autonomously control your desktop',
    docs: {
      summary:
        'Starts desktop agent mode and asks the agent to carry out the typed task.',
      usage: '/do <task>',
      examples: [
        '`/do open Settings and turn on Bluetooth`',
        '`/do draft an email to the product team`',
      ],
      behavior:
        'Routes the task into agent mode instead of normal chat generation and shows the task in the conversation.',
      composability:
        'Standalone command; it bypasses prompt-template utility routing.',
    },
    promptHelp: {
      summary:
        'Start desktop agent mode and execute the user task autonomously.',
    },
  }),
  defineCommand({
    trigger: '/think',
    label: '/think',
    description: 'Think deeply before answering',
    docs: {
      summary:
        'Enables extended reasoning for the current request when the active model supports it.',
      usage: '/think <question or task>',
      examples: [
        '`/think compare these options`',
        '`/think /tldr summarize this thread`',
      ],
      behavior:
        'Leaves the visible user message unchanged while enabling the backend thinking mode for that turn.',
      composability:
        'Can be combined with `/screen` and utility text commands.',
    },
    promptHelp: {
      summary: 'Enable extended reasoning for this turn when supported.',
    },
  }),
  defineCommand({
    trigger: '/search',
    label: '/search',
    description: 'Agentic web search: iterative reasoning & cited synthesis',
    docs: {
      summary:
        'Runs the dedicated search pipeline with iterative reasoning, source reading, and cited synthesis.',
      usage: '/search <question>',
      examples: [
        '`/search latest Rust async runtime benchmarks`',
        '`/search explain this selected paragraph`',
      ],
      behavior:
        'Bypasses the normal chat model path and routes directly to the search pipeline, which may ask clarifying follow-ups before finishing.',
      composability:
        'Can be combined with `/think`; follow-up clarifications continue through search until the search turn fully completes.',
      limit:
        'Needs a non-empty question after command stripping unless selected text is available.',
    },
    promptHelp: {
      summary:
        'Use the agentic search pipeline with iterative web search and cited synthesis.',
    },
  }),
  defineCommand({
    trigger: '/kite',
    label: '/kite',
    description: 'Kite Passport setup, status, payments, and x402 calls',
    docs: {
      summary:
        'Routes the request into Thuki’s Kite Passport integration for setup, connection status, payment approval, and x402 service calls.',
      usage:
        '/kite <setup|connect|status|payer|approve|call> [flags]',
      examples: [
        '`/kite setup`',
        '`/kite status`',
        '`/kite approve --payee 0xabc --amount 100 --token USDC`',
        '`/kite call --url https://example.com/paid --method POST --body \'{"city":"Riyadh"}\'`',
      ],
      behavior:
        'Bypasses the normal chat model path and calls the native Kite backend flow directly. Users still create their Passport account and agent in Kite’s Portal, then paste the MCP URL into Settings.',
      composability:
        'Standalone backend command; it is not a prompt-template command.',
      limit:
        'Mode 1 only. Invite-only/testnet onboarding still happens in Kite’s Portal.',
    },
    promptHelp: {
      summary:
        'Run a Kite Passport backend action such as setup, status, payer lookup, payment approval, or an x402 call.',
    },
  }),
  defineCommand({
    trigger: '/translate',
    label: '/translate',
    description: 'Translate text to another language',
    docs: {
      summary:
        'Translates the input text into the requested target language.',
      usage: '/translate <language> <text>',
      examples: [
        '`/translate Japanese hello world`',
        '`/translate vi this announcement`',
      ],
      behavior:
        'Uses selected text first when present, otherwise typed text after the language token. If both are present, the typed text is treated as an extra instruction.',
      languageFormat:
        'Accepts language names, ISO codes, abbreviations, and informal shorthand.',
      defaultBehavior:
        'If no target language is provided, translate to English for non-English input or to Vietnamese for English input.',
    },
    promptHelp: {
      summary:
        'Translate the input into the requested language; selected text takes priority over typed text.',
    },
    promptTemplate:
      'You are a translation assistant. Translate the following text to the specified target language. The user may specify the target language by its full name (e.g., "Vietnamese"), ISO code (e.g., "vi", "vie"), abbreviation, or informal shorthand. Interpret the language identifier flexibly and use your best judgment. If no target language is specified: translate to English if the text is non-English, or to Vietnamese if it is already in English. Output only the translation with no commentary or explanation.\n\nTarget language: $LANG\n\nText: $INPUT',
  }),
  defineCommand({
    trigger: '/rewrite',
    label: '/rewrite',
    description: 'Rewrite text for clarity and flow',
    docs: {
      summary:
        'Rewrites text so it reads more naturally while preserving intent.',
      usage: '/rewrite <text>',
      examples: [
        '`/rewrite make this update clearer`',
        '`/rewrite` with selected text',
      ],
      behavior:
        'Uses selected text as the main source when present and outputs only the rewritten text.',
    },
    promptHelp: {
      summary:
        'Rewrite the input for clarity and natural flow while preserving meaning.',
    },
    promptTemplate:
      'Please help rewrite the text below so it reads naturally and smoothly. Make it clear, easy to understand, and easy to follow. No icons, no em dashes. Please output only the rewritten text.\n\nText: $INPUT',
  }),
  defineCommand({
    trigger: '/tldr',
    label: '/tldr',
    description: 'Summarize text in 1-3 sentences',
    docs: {
      summary: 'Produces a short, direct TL;DR of the input text.',
      usage: '/tldr <text>',
      examples: ['`/tldr this meeting transcript`', '`/tldr` with selected text'],
      behavior:
        'Outputs only the summary and strips away non-essential background details.',
    },
    promptHelp: {
      summary: 'Summarize the input into a concise TL;DR.',
    },
    promptTemplate:
      "Summarize the following text into a TL;DR. Capture the core message in 1-3 short, direct sentences. Focus on what matters most: the main point, the key decision, or the critical takeaway. Skip background details, qualifications, and anything that isn't essential to understanding the gist. Output only the summary.\n\nText: $INPUT",
  }),
  defineCommand({
    trigger: '/refine',
    label: '/refine',
    description: 'Fix grammar, spelling, and punctuation',
    docs: {
      summary:
        'Lightly edits text to fix grammar, spelling, punctuation, and awkward phrasing.',
      usage: '/refine <text>',
      examples: ['`/refine she dont goes there`', '`/refine` with selected text'],
      behavior:
        'Preserves tone and meaning, avoids restructuring the content, and outputs only the refined text.',
    },
    promptHelp: {
      summary:
        'Fix grammar, spelling, punctuation, and light phrasing issues without changing intent.',
    },
    promptTemplate:
      'Refine the following text by correcting grammar, spelling, punctuation, and awkward phrasing. Keep the original tone, voice, and meaning intact. Do not restructure paragraphs, add new ideas, or remove content. If a sentence is grammatically correct but stylistically rough, smooth it lightly without changing the intent. Output only the refined text.\n\nText: $INPUT',
  }),
  defineCommand({
    trigger: '/bullets',
    label: '/bullets',
    description: 'Extract key points as a bullet list',
    docs: {
      summary:
        'Pulls the key points out of the input and returns them as concise bullet items.',
      usage: '/bullets <text>',
      examples: ['`/bullets this article`', '`/bullets` with selected text'],
      behavior:
        'Outputs only markdown bullets that begin with `- ` and omits other formatting.',
    },
    promptHelp: {
      summary:
        'Extract the main points from the input as a markdown bullet list.',
    },
    promptTemplate:
      'Extract the key points from the following text as a bulleted list. Each item must begin with "- " (a hyphen followed by a space). Do not use numbered lists, plain paragraphs, headers, or any other formatting. Output only the bulleted list, nothing else.\n\nExample output format:\n- First key point\n- Second key point\n- Third key point\n\nEach bullet should be a concise, self-contained statement. Order by importance or logical sequence. Leave out filler and repetition.\n\nText: $INPUT',
  }),
  defineCommand({
    trigger: '/todos',
    label: '/todos',
    description: 'Extract to-do items as a checkbox list',
    docs: {
      summary:
        'Summarizes the input and extracts action items into a markdown checkbox list.',
      usage: '/todos <text>',
      examples: ['`/todos this planning thread`', '`/todos` with selected text'],
      behavior:
        'Returns a short context paragraph followed by a markdown checklist where every task starts with `- [ ] `.',
    },
    promptHelp: {
      summary:
        'Summarize the input, then extract tasks into a markdown checkbox list.',
    },
    promptTemplate:
      'Read the following text and respond in two parts:\n\n**Part 1: Summary.** Write a short paragraph (3-5 sentences) explaining what this text is about. Cover: what the situation or topic is, who is involved, what the current state is, and why it matters or what is at stake. This should give someone who has not read the original text a clear picture of the context.\n\n**Part 2: To-dos.** List every task, action item, commitment, and follow-up from the text as a markdown checkbox list. Every single item MUST begin with "- [ ] " (hyphen, space, open bracket, space, close bracket, space). Do not use numbered lists, plain bullets, headers, or any other format for the list items.\n\nSeparate the two parts with a blank line. Do not add any headings or labels like "Summary:" or "To-dos:"; just write the paragraph, then the list.\n\nExample output format:\nThis is a paragraph explaining what the text is about, who is involved, and what the situation is. It gives enough context to understand why the tasks matter. It is clear and direct.\n\n- [ ] First task to complete\n- [ ] Second task to complete\n- [ ] Third task to complete\n\nFor each to-do item, include who is responsible (if mentioned), what needs to be done, and any deadline or timeframe (if mentioned). Order by urgency or sequence when possible.\n\nText: $INPUT',
  }),
] as const;

/**
 * Sentinel image-path value used as a loading placeholder while the
 * /screen capture is in flight. ChatBubble detects this value and
 * renders a branded screen-capture loading tile instead of a broken image.
 */
export const SCREEN_CAPTURE_PLACEHOLDER = 'blob:screen-capture-loading';

/**
 * Builds a fully composed prompt from a utility command's template.
 *
 * Input resolution (selected text primary, typed text fallback):
 * 1. Selected text present, no typed text: selected text is $INPUT.
 * 2. No selected text, typed text present: typed text is $INPUT.
 * 3. Both present: selected text is $INPUT, typed text appended as instruction.
 *
 * For /translate, the first word of strippedMessage is treated as the target
 * language identifier. The model interprets it flexibly (full name, ISO code,
 * abbreviation). If the language word is the only typed content and there is
 * no selected text, returns null (no input to translate).
 *
 * Returns null if the command has no template, is unknown, or input is empty.
 */
export function buildPrompt(
  trigger: string,
  strippedMessage: string,
  selectedText?: string,
): string | null {
  const cmd = COMMANDS.find((command) => command.trigger === trigger);
  if (!cmd?.promptTemplate) return null;

  const typed = strippedMessage.trim();
  const selected = selectedText?.trim() ?? '';

  let lang = '';
  let typedRemainder = typed;

  if (trigger === '/translate' && typed) {
    const spaceIdx = typed.indexOf(' ');
    if (spaceIdx === -1) {
      lang = typed;
      typedRemainder = '';
    } else {
      lang = typed.slice(0, spaceIdx);
      typedRemainder = typed.slice(spaceIdx + 1).trim();
    }
  }

  let input: string;
  if (selected && typedRemainder) {
    input = `${selected}\n\n[Additional instruction]: ${typedRemainder}`;
  } else if (selected) {
    input = selected;
  } else if (typedRemainder) {
    input = typedRemainder;
  } else {
    return null;
  }

  return cmd.promptTemplate.replace(/\$LANG|\$INPUT/g, (match) =>
    match === '$LANG' ? lang : input,
  );
}
