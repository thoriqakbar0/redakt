import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { TextMorph } from "torph";
import setupPrompt from "../content/setup-prompt.txt?raw";

const repository = "https://github.com/thoriqakbar0/redakt";
const installCommand =
  "curl -fsSL https://redakt.ta-0.com/install.sh -o install.sh && sh install.sh";
const windowsInstallCommand = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\\install.ps1";
const cargoInstallCommand = "cargo install redakt --locked";
const sensitiveValues = ["passwords", "access tokens", "names", "addresses", "card numbers"];
const assistantLogos = ["openai", "claude", "cursor", "gemini"];

function HeroCopy(props: { playing: boolean }) {
  let rollingText: HTMLSpanElement | undefined;

  onMount(() => {
    if (!rollingText) return;

    const morph = new TextMorph({ element: rollingText, duration: 350 });
    morph.update(sensitiveValues[0]);
    let index = 0;
    createEffect(() => {
      if (!props.playing) return;
      const timer = window.setInterval(() => {
        index = (index + 1) % sensitiveValues.length;
        morph.update(sensitiveValues[index]);
      }, 1100);
      onCleanup(() => window.clearInterval(timer));
    });
    onCleanup(() => morph.destroy());
  });

  return (
    <p>
      Mask{" "}
      <span class="hero-rolling" aria-hidden="true">
        <span ref={rollingText}>{sensitiveValues[0]}</span>
      </span>
      <span class="hero-types">passwords, access tokens, names, addresses, and card numbers</span>
      {" "}in logs, .env files, and prompts before you share them.
      Redakt runs on your machine.
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
      <figcaption>Restore private details in an AI reply</figcaption>
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
        Use <code>redakt --recovery session.recovery &lt; prompt.txt</code> to mask the prompt.
        Review the output before sending it. Save the reply as <code>reply.txt</code> and run{" "}
        <code>redakt --reverse --recovery session.recovery &lt; reply.txt</code> locally.
        Keep the recovery file and its separate code private. The example shortens the placeholder.
      </p>
    </figure>
  );
}

function Terminal(props: { playing: boolean }) {
  const [example, setExample] = createSignal(examples[0]);
  const [selectedByReader, setSelectedByReader] = createSignal(false);
  const [autoplay, setAutoplay] = createSignal(true);
  const [visible, setVisible] = createSignal(false);
  let terminal: HTMLDivElement | undefined;

  onMount(() => {
    if (!terminal) return;
    let index = 0;
    const observer = new IntersectionObserver(([entry]) => {
      setVisible(entry.isIntersecting);
    });
    observer.observe(terminal);
    onCleanup(() => observer.disconnect());

    createEffect(() => {
      if (!props.playing || !autoplay() || !visible()) return;
      const timer = window.setInterval(() => {
        index = (index + 1) % examples.length;
        setExample(examples[index]);
      }, 1500);
      onCleanup(() => window.clearInterval(timer));
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
                onFocus={() => setAutoplay(false)}
                onClick={() => {
                  setAutoplay(false);
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
        <h2 id="prompts-title">Ask your coding assistant to redact files first</h2>
        <p>
          Paste the setup prompt into Codex, Claude Code, Cursor, or Gemini CLI
          to add a project rule for handling sensitive files. Review the redacted
          copy before sharing it.
        </p>
      </div>
      <div class="prompt-action" id="setup-prompt">
        <button type="button" class="button primary" onClick={copy}>
          Copy prompt
          <span class="prompt-logos" aria-hidden="true">
            <For each={assistantLogos}>
              {(logo) => <img src={`/assistant-logos/${logo}.svg`} alt="" width="16" height="16" />}
            </For>
          </span>
        </button>
        <span class="copy-feedback" role="status">{feedback()}</span>
      </div>
      <details class="prompt-fallback" open={copyFailed()}>
        <summary>Read the setup prompt</summary>
        <p>{prompt}</p>
      </details>
    </section>
  );
}

function InstallCommand(props: { command: string; platform: string; shellPrompt?: boolean }) {
  const { copy, feedback } = createClipboard(props.command, "Command");

  return (
    <>
      <div class="install-command">
        <code>{props.shellPrompt && <span aria-hidden="true">$ </span>}{props.command}</code>
        <button
          type="button"
          class="button ghost"
          aria-label={`Copy ${props.platform} command`}
          onClick={copy}
        >
          {feedback() === "Command copied." ? "Copied" : "Copy"}
        </button>
      </div>
      <p class="copy-feedback" role="status">{feedback()}</p>
    </>
  );
}

function Install() {
  return (
    <section class="install" id="install" aria-labelledby="install-title">
      <div>
        <h2 id="install-title">Install redakt</h2>
        <p>
          Run this in your terminal on macOS or glibc-based Linux. The installer
          verifies the download's SHA-256 checksum.
        </p>
        <a class="inline-link" href={repository + "/releases"}>Browse GitHub releases</a>
      </div>
      <div class="install-shell">
        <InstallCommand command={installCommand} platform="macOS and Linux" shellPrompt />
        <details class="install-options">
          <summary>Windows and Cargo</summary>
          <p>
            On x86-64 Windows, download <a href={repository + "/blob/main/install.ps1"}>
              install.ps1
            </a> and run this from its folder in PowerShell.
          </p>
          <InstallCommand command={windowsInstallCommand} platform="Windows" />
          <p>With Rust and Cargo installed, run:</p>
          <InstallCommand command={cargoInstallCommand} platform="Cargo" />
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
    answer: "Yes. Check for private details redakt missed and ordinary text it masked by mistake.",
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
  const [allowsMotion, setAllowsMotion] = createSignal(false);
  const [paused, setPaused] = createSignal(false);
  const playing = () => allowsMotion() && !paused();
  let stamp: HTMLIFrameElement | undefined;

  const syncStamp = () => {
    stamp?.contentWindow?.postMessage({ redaktAnimations: playing() }, window.location.origin);
  };

  onMount(() => {
    const motion = matchMedia("(prefers-reduced-motion: no-preference)");
    const syncMotion = () => setAllowsMotion(motion.matches);
    syncMotion();
    motion.addEventListener("change", syncMotion);
    onCleanup(() => motion.removeEventListener("change", syncMotion));
    createEffect(syncStamp);
  });

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
          <section class="hero" aria-labelledby="hero-title" data-playing={playing()}>
            <div class="hero-copy">
              <h1 id="hero-title"><span class="hero-strike">redakt</span> before you share.</h1>
              <HeroCopy playing={playing()} />
              <div class="hero-actions">
                <a class="button primary" href="#install">Install redakt</a>
                <a class="inline-link" href="#workflow">See how it works</a>
                <Show when={allowsMotion()}>
                  <button
                    type="button"
                    class="button ghost"
                    onClick={() => setPaused(!paused())}
                  >
                    {paused() ? "Resume animations" : "Pause animations"}
                  </button>
                </Show>
              </div>
            </div>
            <Terminal playing={playing()} />
            <iframe
              ref={stamp}
              onLoad={syncStamp}
              class="hero-figure"
              src="/redaction-stamp.html"
              title="Redaction stamp illustration"
              aria-hidden="true"
              tabindex={-1}
            />
          </section>
          <section class="model" id="local-model" aria-labelledby="model-title">
            <div>
              <h2 id="model-title">Detect sensitive text offline</h2>
              <p>
                Redakt runs on your CPU. Rules look for patterns such as card numbers.
                A trained detector uses nearby words to spot names and addresses in English sentences.
              </p>
              <p>
                The detector trained on 100,000 synthetic English documents.
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
              <h2 id="workflow-title">Make a redacted copy</h2>
              <p>
                This command leaves your <code>.env</code> intact and prints the path
                to a private copy. It replaces detected values with labels such as{" "}
                <code>[SECRET_1]</code> and saves no recovery files. Review the copy before sharing it.
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
                Redakt processes your text locally. For reversible redaction, keep the
                encrypted recovery file and its separate code private.
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
