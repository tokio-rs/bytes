import { defineConfig, markdown } from "sourcey";

const canonicalUrl = new URL(
  process.env.READTHEDOCS_CANONICAL_URL ??
    "https://bytes-field-guide.readthedocs.io/en/latest/",
);

export default defineConfig({
  name: "Bytes Field Guide",
  siteUrl: canonicalUrl.origin,
  baseUrl: canonicalUrl.pathname,
  repo: "https://github.com/tzwkb/bytes",
  editBranch: "main",
  editBasePath: "docs/sourcey",
  navigation: {
    tabs: [
      {
        tab: "Guide",
        slug: "",
        source: markdown({
          groups: [
            { group: "Start", pages: ["index", "installation"] },
            { group: "Buffer Types", pages: ["bytes", "bytes-mut"] },
            { group: "Traits", pages: ["buf", "buf-mut"] },
            { group: "Workflows", pages: ["adapters", "patterns"] },
          ],
        }),
      },
    ],
  },
});
