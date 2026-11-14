// Records that the install of the shared TypeScript tools was made from their
// current package.json and lock file.

import { toolsDir, writeStamp } from "./lib/tools-install.mts";

writeStamp(toolsDir);
