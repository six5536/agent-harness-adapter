// The checks' tool: answers each hook from the hook contract on stdin and
// logs what it saw. Its words come from the run's words file, so a reply
// that holds one proves the agent got that text.
//
//   node tool.mjs <words.json> <hooks.jsonl>
import { appendFileSync, readFileSync } from "node:fs";

const [wordsFile, logFile] = process.argv.slice(2);
const words = JSON.parse(readFileSync(wordsFile, "utf8"));
const input = JSON.parse(readFileSync(0, "utf8"));

const shell = JSON.stringify(input.tool?.input ?? "");
const answer = (() => {
  switch (input.event) {
    case "session-start":
      return { answer: "context", text: `The session word is ${words.session}.` };
    case "pre-tool":
      return shell.includes("aha-denied-marker")
        ? { answer: "deny", reason: `aha-check refuses this command (${words.deny})` }
        : { answer: "allow" };
    case "post-tool":
      return { answer: "context", text: `The after-tool word is ${words.post}.` };
    case "stop":
      return input.continuing
        ? { answer: "allow" }
        : { answer: "continue", reason: `Before you stop, reply with the stop word ${words.stop} and nothing else.` };
    default:
      return { answer: "allow" };
  }
})();
appendFileSync(logFile, `${JSON.stringify({ in: input, out: answer })}\n`);
process.stdout.write(`${JSON.stringify(answer)}\n`);
