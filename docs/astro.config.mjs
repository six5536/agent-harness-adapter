// The docs site (PLAN-016): Astro Starlight, deployed to GitHub Pages.
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import starlightLinksValidator from "starlight-links-validator";
import starlightTypeDoc, { typeDocSidebarGroup } from "starlight-typedoc";

const repo = "https://github.com/six5536/agent-harness-adapter";

export default defineConfig({
  site: "https://six5536.github.io",
  base: "/agent-harness-adapter",
  integrations: [
    starlight({
      title: "agent-harness-adapter",
      description:
        "One agent setup. Every coding agent. Write instructions, skills and hooks once; every coding agent uses them.",
      social: [{ icon: "github", label: "GitHub", href: repo }],
      customCss: ["./src/styles/custom.css"],
      components: { Hero: "./src/components/Hero.astro" },
      plugins: [
        // pdoc writes the Python API reference into public/ at build time.
        starlightLinksValidator({ exclude: ["/agent-harness-adapter/api/python/"] }),
        starlightTypeDoc({
          entryPoints: ["../packages/agent-harness-adapter-node/index.d.ts"],
          tsconfig: "./tsconfig.typedoc.json",
          output: "api/node",
          sidebar: { label: "Node", collapsed: true },
          typeDoc: { treatWarningsAsErrors: true, readme: "none" },
        }),
      ],
      sidebar: [
        { label: "Getting started", slug: "getting-started" },
        { label: "How it works", slug: "how-it-works" },
        { label: "Coming from Claude Code", slug: "claude-code" },
        { label: "Troubleshooting", slug: "troubleshooting" },
        {
          label: "Write once",
          items: [
            "write-once/instructions",
            "write-once/skills",
            "write-once/hooks",
            "write-once/mcp-and-subagents",
            "write-once/commands-and-permissions",
            "write-once/scopes",
          ],
        },
        {
          label: "For tool authors",
          items: ["use/command", "use/rust", "use/node", "use/python", "use/example"],
        },
        {
          label: "Reference",
          items: [
            "reference/config",
            "reference/hooks-file",
            "reference/hook-contract",
            "reference/harnesses",
            "reference/schemas",
            "reference/changelog",
          ],
        },
        {
          label: "API reference",
          items: [
            { label: "Rust (docs.rs) ↗", link: "https://docs.rs/agent-harness-adapter-core", attrs: { target: "_blank" } },
            typeDocSidebarGroup,
            { label: "Python (pdoc) ↗", link: "/agent-harness-adapter/api/python/", attrs: { target: "_blank" } },
          ],
        },
      ],
    }),
  ],
});
