import { createSignal, For } from "solid-js";
import setupPrompt from "../content/setup-prompt.txt?raw";

const repository = "https://github.com/thoriqakbar0/redakt";
const installCommand = "cargo install --git https://github.com/thoriqakbar0/redakt --locked";

const promptInstructions = [
  "Feed the source file directly to stdin and write stdout to a separate local file.",
  "Restrict access to the output to my account and choose a fresh path if it already exists.",
  "Keep file contents out of chat, tool output, and external services.",
  "Report only the output path so I can review it locally.",
  "If the command fails, summarize the error without quoting file contents.",
].join(" ");

const prompts = [
  {
    title: "Redact a log file",
    task: "Use the local redakt CLI to redact support.log into support.redacted.log.",
  },
  {
    title: "Redact a JSONL file",
    task: [
      "Use the local redakt CLI with --jsonl to redact events.jsonl",
      "into events.redacted.jsonl.",
      "Each input line should contain an id and text.",
    ].join(" "),
  },
];

const examples = [
  {
    label: "Text",
    command: "redakt < support.log",
    input: "Email: ops@example.test\nLevel: warning\nAPI_KEY=synthetic_demo_1234",
    output: "Email: [EMAIL_1]\nLevel: warning\nAPI_KEY=[SECRET_1]",
  },
  {
    label: "JSONL",
    command: "redakt --jsonl < events.jsonl",
    input: '{"id":"event-1","text":"Email: ops@example.test"}',
    output: [
      '{"id":"event-1","redacted":"Email: [EMAIL_1]",',
      '"spans":[{"start":7,"end":23,"label":"email"}]}',
    ].join(""),
  },
];

const capabilities = [
  {
    title: "Keep raw logs on your machine",
    description: "The detection rules are built into the binary, so you can redact files offline.",
    link: "How detection works",
    href: repository + "/blob/main/docs/ternary.md",
  },
  {
    title: "Follow the same value across a log",
    description: "A repeated email address becomes the same label each time, such as [EMAIL_1].",
    link: "See what the CLI detects",
    href: repository + "#coverage-and-measurements",
  },
  {
    title: "Process records in batches",
    description: "Use JSONL to stream documents and get the start and end of each match.",
    link: "Read the JSONL guide",
    href: repository + "#process-multiple-documents",
  },
];

function Terminal() {
  const [example, setExample] = createSignal(examples[0]);

  return (
    <div class="terminal" id="example" role="region" aria-label="Example redaction">
      <div class="terminal-top">
        <div class="terminal-controls" role="group" aria-label="Sample output format">
          <For each={examples}>
            {(option) => (
              <button
                type="button"
                aria-pressed={example() === option ? "true" : "false"}
                aria-controls="terminal-output"
                onClick={() => setExample(option)}
              >
                {option.label}
              </button>
            )}
          </For>
        </div>
        <span class="terminal-caption">Synthetic example</span>
      </div>
      <div class="terminal-body" id="terminal-output" aria-live="polite">
        <div class="terminal-stream">
          <span id="stdin-label" class="stream-label">stdin</span>
          <pre aria-labelledby="stdin-label"><code>{example().input}</code></pre>
        </div>
        <code class="command"><span aria-hidden="true">$</span> {example().command}</code>
        <div class="terminal-stream">
          <span id="stdout-label" class="stream-label">stdout</span>
          <pre aria-labelledby="stdout-label"><code>
            <For each={example().output.split(/(\[[A-Z]+_\d+\])/g)}>
              {(part) => part.startsWith("[") ? <mark>{part}</mark> : part}
            </For>
          </code></pre>
        </div>
      </div>
    </div>
  );
}

function createClipboard(text: string, item: string) {
  const [feedback, setFeedback] = createSignal("");

  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      setFeedback(`${item} copied.`);
    } catch {
      setFeedback("Select the text to copy it manually.");
    }
  }

  return { copy, feedback };
}

function PromptCard(props: { title: string; text: string; label: string; id?: string }) {
  const { copy, feedback } = createClipboard(props.text, "Prompt");

  return (
    <article class="prompt" id={props.id}>
      <h3>{props.title}</h3>
      <p>{props.text}</p>
      <div class="prompt-action">
        <button type="button" class="button ghost" onClick={copy}>{props.label}</button>
        <span class="copy-feedback" role="status">{feedback()}</span>
      </div>
    </article>
  );
}

function Prompts() {
  return (
    <section class="prompts" id="prompts" aria-labelledby="prompts-title">
      <div class="workflow-intro">
        <h2 id="prompts-title">Prompts for coding assistants</h2>
        <p>Paste the setup prompt into Codex or Claude Code from your project root.</p>
      </div>
      <div class="prompt-grid">
        <PromptCard
          id="setup-prompt"
          title="Set up redakt in your project"
          text={setupPrompt.trim()}
          label="Copy setup prompt"
        />
        <For each={prompts}>
          {(prompt) => (
            <PromptCard
              title={prompt.title}
              text={`${prompt.task} ${promptInstructions}`}
              label="Copy prompt"
            />
          )}
        </For>
      </div>
    </section>
  );
}

function Install() {
  const { copy, feedback } = createClipboard(installCommand, "Command");

  return (
    <section class="install" id="install" aria-labelledby="install-title">
      <div>
        <h2 id="install-title">Install the CLI</h2>
        <p>Install from source with Cargo.</p>
        <a class="inline-link" href={repository + "/releases"}>View releases</a>
      </div>
      <div class="install-shell">
        <div class="install-command">
          <code><span aria-hidden="true">$</span> {installCommand}</code>
          <button type="button" class="button ghost" onClick={copy}>
            {feedback() === "Command copied." ? "Copied" : "Copy"}
          </button>
        </div>
        <p class="copy-feedback" role="status">{feedback()}</p>
      </div>
    </section>
  );
}

export default function Home() {
  return (
    <>
      <a class="skip-link" href="#main">Skip to content</a>
      <header class="site-header" id="top">
        <div class="section-width header-content">
          <a class="wordmark" href="/" aria-label="redakt home">
            <span class="brand-mark" aria-hidden="true"><i /><i /><i /></span>
            redakt
          </a>
          <nav aria-label="Primary">
            <a href="#workflow">How it works</a>
            <a href={repository + "#try-it"}>Docs</a>
            <a href={repository}>GitHub</a>
          </nav>
        </div>
      </header>
      <main id="main">
        <div class="section-width">
          <section class="hero" aria-labelledby="hero-title">
            <div class="hero-copy">
              <h1 id="hero-title">Redact your LLM calls.</h1>
              <p>
                Replace detected email addresses, API keys, and other sensitive values on
                your machine before sending logs or documents to a language model.
              </p>
              <div class="hero-actions">
                <a class="button primary" href="#install">Install CLI</a>
                <a class="button text-button" href="#setup-prompt">Set up with an assistant</a>
              </div>
            </div>
            <Terminal />
          </section>
          <section class="workflow" id="workflow" aria-labelledby="workflow-title">
            <div class="workflow-intro">
              <h2 id="workflow-title">Save a redacted copy</h2>
              <p>Send a log through the CLI, save the output, and review it before sharing.</p>
            </div>
            <p class="workflow-command">
              <code>
                <span aria-hidden="true">$</span>
                {" redakt < support.log > support.redacted.log"}
              </code>
            </p>
            <div class="capabilities">
              <For each={capabilities}>
                {(item) => (
                  <article>
                    <h3>{item.title}</h3>
                    <p>{item.description}</p>
                    <a class="inline-link" href={item.href}>{item.link}</a>
                  </article>
                )}
              </For>
            </div>
          </section>
          <Install />
          <Prompts />
        </div>
      </main>
      <footer class="site-footer">
        <div class="section-width footer-content">
          <a href="#top">redakt</a>
          <nav aria-label="Project">
            <a href="/about">About</a>
            <a href="/contact">Contact</a>
            <a href="/privacy">Privacy</a>
            <a href={repository + "/blob/main/LICENSE"}>Apache 2.0</a>
            <a href={repository + "/blob/main/SECURITY.md"}>Security</a>
            <a href={repository}>GitHub</a>
          </nav>
        </div>
      </footer>
    </>
  );
}
