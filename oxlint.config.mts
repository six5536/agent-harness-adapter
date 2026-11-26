import base from "./.config/oxc/oxlint.base.json" with { type: "json" };
import { projectIgnorePatterns } from "./.config/oxc/ignore.mts";

export default {
  extends: [base],
  ignorePatterns: [".sokf/**", ".pakf/**", "packs/*/files/**", ...projectIgnorePatterns()],
};
