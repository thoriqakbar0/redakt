import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { TextMorph } from "torph";
import setupPrompt from "../content/setup-prompt.txt?raw";

const repository = "https://github.com/thoriqakbar0/redakt";
const installCommand =
  "curl -fsSL https://redakt.ta-0.com/install.sh -o install.sh && sh install.sh";
const sensitiveValues = ["passwords", "access tokens", "names", "addresses", "card numbers"];

function HeroCopy() {
  let rollingText: HTMLSpanElement | undefined;

  onMount(() => {
    if (!rollingText) return;

    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    if (motion.matches) return;

    const morph = new TextMorph({ element: rollingText, duration: 350 });
    morph.update(sensitiveValues[0]);
    let index = 0;
    const timer = window.setInterval(() => {
      index += 1;
      morph.update(sensitiveValues[index]);
      if (index === sensitiveValues.length - 1) window.clearInterval(timer);
    }, 1100);
    const stop = () => window.clearInterval(timer);
    motion.addEventListener("change", stop);
    onCleanup(() => {
      stop();
      motion.removeEventListener("change", stop);
      morph.destroy();
    });
  });

  return (
    <p>
      Mask{" "}
      <span class="hero-rolling" aria-hidden="true">
        <span ref={rollingText}>{sensitiveValues[0]}</span>
      </span>
      <span class="hero-types">passwords, access tokens, names, addresses, and card numbers</span>
      {" "}in logs, .env files, and prompts on your machine before sharing.
    </p>
  );
}

const examples = [
  {
    label: ".env",
    command: "redakt --irreversible < .env.example",
    input: [
      "# Database connection",
      "PASSWORD=demo-db-password",
    ].join("\n"),
    output: [
      "# Database connection",
      "PASSWORD=[SECRET_1]",
    ].join("\n"),
  },
  {
    label: "Text",
    command: "redakt --irreversible < support.log",
    input: "Email: ops@example.test\nLevel: warning\nAccess token: demo-access-token",
    output: "Email: [EMAIL_1]\nLevel: warning\nAccess token: [SECRET_1]",
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
  {
    label: "Prose",
    command: "redakt --irreversible < note.txt",
    input: "Sarah called this morning. Her email is sarah@example.test.",
    output: "[NAME_1] called this morning. Her email is [EMAIL_1].",
  },
];

const promptFlow = [
  { title: "Your prompt", text: "Draft a note to ops@example.test." },
  { title: "What you send", text: "Draft a note to [REDAKT_…]." },
  { title: "What AI sends back", text: "To: [REDAKT_…]\nThe deploy is ready." },
  { title: "Your restored reply", text: "To: ops@example.test\nThe deploy is ready." },
];

function PromptFlow() {
  return (
    <figure class="prompt-flow">
      <figcaption>From prompt to reply</figcaption>
      <ol>
        <For each={promptFlow}>
          {(step) => (
            <li>
              <span>{step.title}</span>
              <pre><code>{step.text}</code></pre>
            </li>
          )}
        </For>
      </ol>
      <p>
        Hide private details with <code>redakt --recovery session.recovery &lt; prompt.txt</code>.
        To restore the reply, run <code>redakt --reverse --recovery session.recovery &lt; reply.txt</code>.
        Keep both recovery files private. The example shows a shortened placeholder.
      </p>
    </figure>
  );
}

function Terminal() {
  const [example, setExample] = createSignal(examples[0]);
  const [selectedByReader, setSelectedByReader] = createSignal(false);
  let terminal: HTMLDivElement | undefined;
  let stopAutoplay = () => {};

  onMount(() => {
    if (!terminal) return;
    const motion = matchMedia("(prefers-reduced-motion: reduce)");
    if (motion.matches) return;

    let timer: number | undefined;
    let index = 0;

    const observer = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) return;
      observer.disconnect();
      timer = window.setInterval(() => {
        index += 1;
        setExample(examples[index]);
        if (index === examples.length - 1) stopAutoplay();
      }, 1500);
    });

    stopAutoplay = () => {
      observer.disconnect();
      window.clearInterval(timer);
    };

    observer.observe(terminal);
    motion.addEventListener("change", stopAutoplay);
    onCleanup(() => {
      stopAutoplay();
      motion.removeEventListener("change", stopAutoplay);
    });
  });

  return (
    <div class="terminal" id="example" role="region" aria-label="Example redaction" ref={terminal}>
      <div class="terminal-top">
        <div class="terminal-controls" role="group" aria-label="Redaction examples">
          <For each={examples}>
            {(option) => (
              <button
                type="button"
                aria-pressed={example() === option ? "true" : "false"}
                aria-controls="terminal-output"
                onFocus={() => stopAutoplay()}
                onClick={() => {
                  stopAutoplay();
                  setSelectedByReader(true);
                  setExample(option);
                }}
              >
                {option.label}
              </button>
            )}
          </For>
        </div>
      </div>
      <div class="terminal-body" id="terminal-output" aria-live={selectedByReader() ? "polite" : "off"}>
        <div class="terminal-stream">
          <span id="stdin-label" class="stream-label">stdin</span>
          <pre aria-labelledby="stdin-label"><code>{example().input}</code></pre>
        </div>
        <code class="command"><span aria-hidden="true">$</span> {example().command}</code>
        <div class="terminal-stream">
          <span id="stdout-label" class="stream-label">stdout</span>
          <pre aria-labelledby="stdout-label">
            <Show when={example()} keyed>
              {(sample) => (
                <code>
                  <For each={sample.output.split(/(\[[A-Z_]+_\d+\])/g)}>
                    {(part) => part.startsWith("[") ? <mark>{part}</mark> : part}
                  </For>
                </code>
              )}
            </Show>
          </pre>
        </div>
      </div>
    </div>
  );
}

function createClipboard(text: string, item: string) {
  const [feedback, setFeedback] = createSignal("");
  const [copyFailed, setCopyFailed] = createSignal(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      setCopyFailed(false);
      setFeedback(`${item} copied.`);
    } catch {
      setCopyFailed(true);
      setFeedback("Select the text to copy it manually.");
    }
  }

  return { copy, feedback, copyFailed };
}

function Prompts() {
  const prompt = setupPrompt.trim();
  const { copy, feedback, copyFailed } = createClipboard(prompt, "Prompt");

  return (
    <section class="prompts" id="prompts" aria-labelledby="prompts-title">
      <div class="workflow-intro">
        <h2 id="prompts-title">Run redakt from Codex or Claude Code</h2>
        <p>
          Paste this prompt into your assistant to set up redakt for sensitive files.
        </p>
      </div>
      <div class="prompt-action" id="setup-prompt">
        <button type="button" class="button primary" onClick={copy}>Copy prompt</button>
        <span class="copy-feedback" role="status">{feedback()}</span>
      </div>
      <details class="prompt-fallback" open={copyFailed()}>
        <summary>Read the setup prompt</summary>
        <p>{prompt}</p>
      </details>
    </section>
  );
}

function Install() {
  const { copy, feedback } = createClipboard(installCommand, "Command");

  return (
    <section class="install" id="install" aria-labelledby="install-title">
      <div>
        <h2 id="install-title">Install redakt</h2>
        <p>
          Run this in your terminal on macOS or Linux with glibc. The installer
          checks the download before it installs redakt.
        </p>
        <a class="inline-link" href={repository + "/releases"}>Browse GitHub releases</a>
      </div>
      <div class="install-shell">
        <div class="install-command">
          <code><span aria-hidden="true">$</span> {installCommand}</code>
          <button type="button" class="button ghost" onClick={copy}>
            {feedback() === "Command copied." ? "Copied" : "Copy"}
          </button>
        </div>
        <p class="copy-feedback" role="status">{feedback()}</p>
        <details class="install-options">
          <summary>Windows and Cargo</summary>
          <p>
            On 64 bit x86 Windows, download <a href={repository + "/blob/main/install.ps1"}>
              install.ps1
            </a> and run this from its folder in PowerShell.
          </p>
          <pre><code>powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\install.ps1</code></pre>
          <p>With Rust and Cargo installed, run:</p>
          <pre><code>cargo install redakt --locked</code></pre>
        </details>
      </div>
    </section>
  );
}

const questions = [
  {
    question: "Does redakt send my text anywhere?",
    answer: "Redakt handles your text on your machine. Update checks send no file contents.",
  },
  {
    question: "What can redakt detect?",
    answer:
      "Redakt looks for email addresses, card numbers, and labeled secrets. " +
      "It can also find names and addresses in English text.",
  },
  {
    question: "Should I review the output before sharing?",
    answer: "Yes. Check for private details redakt missed before you share the copy.",
  },
  {
    question: "What if I only need a copy to share?",
    answer: "Use redakt --irreversible to get numbered labels without saving recovery files.",
  },
  {
    question: "Can redakt restore values if the AI edits a placeholder?",
    answer: "Redakt needs the AI to leave the placeholder unchanged to put the original detail back.",
  },
];

function Faq() {
  return (
    <section class="faq" aria-labelledby="faq-title">
      <h2 id="faq-title">Common questions</h2>
      <div class="faq-questions">
        <For each={questions}>
          {(item) => (
            <details>
              <summary>{item.question}</summary>
              <p>{item.answer}</p>
            </details>
          )}
        </For>
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
            <a href="#local-model">Local model</a>
            <a href={repository}>GitHub</a>
          </nav>
        </div>
      </header>
      <main id="main">
        <div class="section-width">
          <section class="hero" aria-labelledby="hero-title">
            <div class="hero-copy">
              <h1 id="hero-title"><span class="hero-strike">redakt</span> before you prompt.</h1>
              <HeroCopy />
              <div class="hero-actions">
                <a class="button primary" href="#install">Install redakt</a>
                <a class="inline-link" href="#local-model">How the local model works</a>
              </div>
            </div>
            <Terminal />
            <iframe
              class="hero-figure"
              src="/redaction-stamp.html"
              title="Redaction stamp illustration"
              aria-hidden="true"
              tabindex={-1}
            />
          </section>
          <section class="model" id="local-model" aria-labelledby="model-title">
            <div>
              <h2 id="model-title">The model runs on your machine.</h2>
              <p>
                Redakt comes with a small model that runs on your CPU, even offline.
                Rules catch patterns like card numbers. The model reads nearby words
                to help spot names and addresses in English sentences.
              </p>
              <p>
                It learned from 100,000 synthetic English documents.
                Always check the copy for details it may have missed.
              </p>
              <a class="inline-link" href={repository + "/tree/main/experiments/pii"}>
                Training and results
              </a>
            </div>
            <div class="model-example">
              <span>Input</span>
              <p><mark>Sarah</mark> called this morning.</p>
              <span>Output with <code>--irreversible</code></span>
              <p><code>[NAME_1]</code> called this morning.</p>
            </div>
          </section>
          <section class="workflow" id="workflow" aria-labelledby="workflow-title">
            <div class="workflow-intro">
              <h2 id="workflow-title">Redact before you share.</h2>
              <p>
                This command saves a copy of your .env file with private details hidden.
                Open the copy and check it before sharing.
              </p>
            </div>
            <p class="workflow-command">
              <code>
                <span aria-hidden="true">$</span>
                {' copy=$(mktemp .env.redacted.XXXXXX) && ' +
                 'redakt --irreversible < .env > "$copy" && printf \'%s\\n\' "$copy"'}
              </code>
            </p>
            <PromptFlow />
          </section>
          <Prompts />
          <Faq />
          <Install />
          <section class="site-info" aria-label="Project information">
            <div id="privacy">
              <h2>Privacy</h2>
              <p>
                Your files stay on your machine. Keep the encrypted recovery file and its
                separate code private. Review redacted copies before sharing them.
              </p>
              <p>
                When the CLI checks GitHub for an update, it asks for release information
                only. Set <code>REDAKT_UPDATE_NOTIFY=0</code> to turn off automatic checks.
              </p>
            </div>
            <div id="support">
              <h2>Support</h2>
              <p>
                Ask a question or report a bug in <a href={repository + "/issues"}>GitHub issues</a>.
                For bugs, include your redakt version, operating system, and a made-up example.
              </p>
              <p>
                Follow the <a href={repository + "/blob/main/SECURITY.md"}>security policy</a>{" "}
                to report a vulnerability privately.
              </p>
            </div>
          </section>
        </div>
      </main>
      <footer class="site-footer">
        <div class="section-width footer-content">
          <a href="#top">redakt</a>
          <nav aria-label="Project">
            <a href="#local-model">Local model</a>
            <a href="#support">Support</a>
            <a href="#privacy">Privacy</a>
            <a href={repository + "/blob/main/LICENSE"}>Apache 2.0</a>
            <a href={repository + "/blob/main/SECURITY.md"}>Security</a>
            <a href={repository}>GitHub</a>
          </nav>
        </div>
      </footer>
    </>
  );
}
