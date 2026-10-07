// Qwen Code (`qwen`).
export default {
  id: "qwen",
  name: "Qwen Code",
  pkg: "@qwen-code/qwen-code",
  version: "0.25.0",
  bin: "qwen",
  // Its free sign-in ended in 2026-04: it needs a provider's key.
  login: (a) =>
    console.log(
      `${a.def.name} signs in with a provider's API key, from the environment of the run:\n\n` +
        "  OPENAI_API_KEY, OPENAI_BASE_URL and OPENAI_MODEL (any OpenAI-compatible endpoint), or\n" +
        "  ANTHROPIC_API_KEY, ANTHROPIC_BASE_URL and ANTHROPIC_MODEL, or\n" +
        "  GEMINI_API_KEY and GEMINI_MODEL\n\n" +
        "Set them when you run the checks.",
    ),
  // --yolo approves every tool, as a trusted user would; folder trust is
  // off unless turned on.
  prompt: (a, text) => [text, "--yolo", "-o", "text", ...(a.model ? ["--model", a.model] : [])],
};
