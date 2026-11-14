import base from "./.config/oxc/oxfmt.base.json" with { type: "json" };
import { projectIgnorePatterns } from "./.config/oxc/ignore.mts";

export default { ...base, ignorePatterns: [...base.ignorePatterns, ...projectIgnorePatterns()] };
