import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { TextMorph } from "torph";
import setupPrompt from "../content/setup-prompt.txt?raw";

const repository = "https://github.com/thoriqakbar0/redakt";
const installCommand =
  "curl -fsSL https://redakt.ta-0.com/install.sh -o install.sh && sh install.sh";
const sensitiveValues = ["passwords", "access tokens", "names", "addresses", "credit cards"];

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
      <span class="hero-types">passwords, access tokens, names, addresses, and credit cards</span>
      {" "}before sharing with AI. Redakt runs on your machine, even offline.
    </p>
  );
}

const examples = [
  {
    label: ".env",
    command: "redakt < .env.example",
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
    command: "redakt < support.log",
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
    command: "redakt < note.txt",
    input: "Sarah called this morning. Her email is sarah@example.test.",
    output: "[NAME_1] called this morning. Her email is [EMAIL_1].",
  },
];

const capabilities = [
  {
    title: "Find sensitive text",
    description: "Rules look for card numbers and labels such as PASSWORD. They run offline.",
    link: "See how detection works",
    href: "/about",
  },
  {
    title: "Replace matches with labels",
    description: "An email becomes [EMAIL_1] and keeps that label throughout a document.",
    link: "See what redakt detects",
    href: repository + "#coverage-and-measurements",
  },
  {
    title: "Review a separate copy",
    description: "Check for missed secrets. In JSONL mode, you also get each match’s position.",
    link: "Read the JSONL guide",
    href: repository + "#process-multiple-documents",
  },
];

const promptFlow = [
  { title: "Original prompt", text: "Draft a note to ops@example.test." },
  { title: "Send the redacted prompt", text: "Draft a note to [EMAIL_1]." },
  { title: "AI response", text: "To: [EMAIL_1]\nThe deploy is ready." },
  { title: "Fill in manually", text: "To: ops@example.test\nThe deploy is ready." },
];

function PromptFlow() {
  const [inView, setInView] = createSignal(false);
  let flow: HTMLElement | undefined;

  onMount(() => {
    if (!flow) return;
    const observer = new IntersectionObserver(([entry]) => {
      if (!entry.isIntersecting) return;
      setInView(true);
      observer.disconnect();
    });
    observer.observe(flow);
    onCleanup(() => observer.disconnect());
  });

  return (
    <figure class="prompt-flow" ref={flow}>
      <figcaption>From prompt to reply</figcaption>
      <ol classList={{ "flow-in-view": inView() }}>
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
        Keep the original value local. If the AI keeps <code>[EMAIL_1]</code> in its reply,
        replace it by hand on your machine.
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
        <p>Open your project in Codex or Claude Code and paste this prompt to set up local redaction.</p>
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
        <p>Install a verified prebuilt binary on macOS or Linux.</p>
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
              <h1 id="hero-title"><span class="hero-strike">redakt</span> before you prompt.</h1>
              <HeroCopy />
              <div class="hero-actions">
                <a class="button primary" href="#install">Install redakt</a>
                <a class="button text-button" href="#setup-prompt">Set up Codex or Claude Code</a>
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
          <section class="workflow" id="workflow" aria-labelledby="workflow-title">
            <div class="workflow-intro">
              <h2 id="workflow-title">How it works</h2>
              <p>
                Redakt replaces detected secrets with labels like <code>[EMAIL_1]</code>{" "}
                before you send text to AI. Its model has about 2 million nonzero weights
                and runs offline on your CPU. It trained on 100,000 synthetic English documents
                from{" "}
                <a class="inline-link" href="https://huggingface.co/datasets/nvidia/Nemotron-PII">
                  NVIDIA’s Nemotron-PII dataset
                </a>.
              </p>
            </div>
            <p class="workflow-command">
              <code>
                <span aria-hidden="true">$</span>
                {" (umask 077; set -C; redakt < .env > .env.redacted)"}
              </code>
            </p>
            <PromptFlow />
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
