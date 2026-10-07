// The test tool for extension.test.mjs: reads the hook contract's input on
// stdin, logs it to seen.jsonl in the working directory, and answers.
import { appendFileSync, readFileSync } from "node:fs";

const input = JSON.parse(readFileSync(0, "utf8"));
appendFileSync("seen.jsonl", `${JSON.stringify(input)}\n`);
const answer = (() => {
  switch (input.event) {
    case "session-start":
      return { answer: "context", text: "session context" };
    case "prompt-submit":
      return { answer: "context", text: `prompt context for ${input.prompt}` };
    case "pre-tool":
      return JSON.stringify(input.tool?.input).includes("rm -rf")
        ? { answer: "deny", reason: "no rm -rf" }
        : { answer: "allow" };
    case "stop":
      return input.continuing ? { answer: "allow" } : { answer: "continue", reason: "continue once" };
    default:
      return { answer: "allow" };
  }
})();
process.stdout.write(`${JSON.stringify(answer)}\n`);
