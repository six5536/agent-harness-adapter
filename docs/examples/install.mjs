// region: install
import aha from "@six5536/agent-harness-adapter-node";

const mytool = {
  version: 1,
  name: "mytool",
  instructions: "Run `mytool check` before you finish.\n",
  skills: [{ name: "mytool", description: "Check the project with mytool.", body: "Run `mytool check`; fix what it reports.\n" }],
  hooks: [{ event: "pre-tool", tools: "shell", command: "mytool hook {harness} {event}" }],
  allow_commands: ["mytool"],
};

const result = aha.install(mytool, ["claude", "codex", "gemini"]);
for (const h of result.harnesses) {
  console.log(h.harness, h.parts.map((p) => `${p.state} ${p.path}`));
}
// endregion: install
